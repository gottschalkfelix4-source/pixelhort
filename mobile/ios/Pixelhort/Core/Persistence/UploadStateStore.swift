import Foundation

/// Persisted per-asset backup state, keyed by `PHAsset.localIdentifier`.
struct UploadRecord: Codable, Sendable, Hashable {
    enum State: String, Codable, Sendable {
        /// Handed to the background URLSession, completion pending.
        case queued
        /// Uploaded (or found on the server as duplicate).
        case uploaded
        /// Last attempt failed; will be retried on the next run.
        case failed
    }

    var localIdentifier: String
    var state: State
    var checksum: String?
    var remoteAssetId: String?
    /// Server id of the uploaded Live Photo video part (uploaded first, then referenced).
    var livePhotoVideoRemoteId: String?
    var livePhotoVideoChecksum: String?
    /// `PHAsset.modificationDate` at upload time; a changed date triggers a re-check.
    var modificationDate: Date?
    var lastError: String?
    var attempts: Int
    var updatedAt: Date

    init(localIdentifier: String, state: State, checksum: String? = nil, remoteAssetId: String? = nil,
         modificationDate: Date? = nil) {
        self.localIdentifier = localIdentifier
        self.state = state
        self.checksum = checksum
        self.remoteAssetId = remoteAssetId
        self.modificationDate = modificationDate
        self.attempts = 0
        self.updatedAt = Date()
    }
}

struct UploadStatistics: Sendable, Equatable {
    var uploaded: Int = 0
    var failed: Int = 0
    var queued: Int = 0
}

/// Small JSON-file backed store. Writes are coalesced to keep IO low during large backups.
actor UploadStateStore {
    static let shared = UploadStateStore()

    private var records: [String: UploadRecord] = [:]
    private let fileURL: URL
    private var loaded = false
    private var saveTask: Task<Void, Never>?

    init(fileURL: URL? = nil) {
        if let fileURL {
            self.fileURL = fileURL
        } else {
            let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
                ?? FileManager.default.temporaryDirectory
            self.fileURL = base.appendingPathComponent("Pixelhort", isDirectory: true)
                .appendingPathComponent("upload-state.json")
        }
    }

    private func loadIfNeeded() {
        guard !loaded else { return }
        loaded = true
        guard let data = try? Data(contentsOf: fileURL) else { return }
        if let decoded = try? JSONCoding.makeDecoder().decode([String: UploadRecord].self, from: data) {
            records = decoded
        }
    }

    // MARK: Queries

    func record(for localIdentifier: String) -> UploadRecord? {
        loadIfNeeded()
        return records[localIdentifier]
    }

    /// True when the asset was uploaded and has not been modified since.
    func isBackedUp(_ localIdentifier: String, modificationDate: Date?) -> Bool {
        loadIfNeeded()
        guard let record = records[localIdentifier], record.state == .uploaded else { return false }
        guard let modificationDate, let stored = record.modificationDate else { return true }
        return abs(modificationDate.timeIntervalSince(stored)) < 1
    }

    func statistics() -> UploadStatistics {
        loadIfNeeded()
        var stats = UploadStatistics()
        for record in records.values {
            switch record.state {
            case .uploaded: stats.uploaded += 1
            case .failed: stats.failed += 1
            case .queued: stats.queued += 1
            }
        }
        return stats
    }

    func failedRecords() -> [UploadRecord] {
        loadIfNeeded()
        return records.values.filter { $0.state == .failed }.sorted { $0.updatedAt > $1.updatedAt }
    }

    // MARK: Mutations

    func markQueued(_ localIdentifier: String, checksum: String?, modificationDate: Date?) {
        mutate(localIdentifier) { record in
            record.state = .queued
            record.checksum = checksum ?? record.checksum
            record.modificationDate = modificationDate
        }
    }

    func markUploaded(_ localIdentifier: String, checksum: String?, remoteAssetId: String?, modificationDate: Date?) {
        mutate(localIdentifier) { record in
            record.state = .uploaded
            record.checksum = checksum ?? record.checksum
            record.remoteAssetId = remoteAssetId ?? record.remoteAssetId
            if let modificationDate { record.modificationDate = modificationDate }
            record.lastError = nil
        }
    }

    func markFailed(_ localIdentifier: String, error: String) {
        mutate(localIdentifier) { record in
            record.state = .failed
            record.lastError = error
            record.attempts += 1
        }
    }

    func setLivePhotoVideo(_ localIdentifier: String, remoteId: String, checksum: String?) {
        mutate(localIdentifier) { record in
            record.livePhotoVideoRemoteId = remoteId
            record.livePhotoVideoChecksum = checksum
        }
    }

    /// Resets `queued` records that no longer have a running URLSession task.
    func resetStaleQueued(keeping activeIdentifiers: Set<String>) {
        loadIfNeeded()
        var changed = false
        for (key, record) in records where record.state == .queued && !activeIdentifiers.contains(key) {
            var updated = record
            updated.state = .failed
            updated.lastError = "interrupted"
            records[key] = updated
            changed = true
        }
        if changed { scheduleSave() }
    }

    func removeAll() {
        records = [:]
        loaded = true
        scheduleSave()
    }

    private func mutate(_ localIdentifier: String, _ change: (inout UploadRecord) -> Void) {
        loadIfNeeded()
        var record = records[localIdentifier] ?? UploadRecord(localIdentifier: localIdentifier, state: .failed)
        change(&record)
        record.updatedAt = Date()
        records[localIdentifier] = record
        scheduleSave()
    }

    // MARK: Persistence

    private func scheduleSave() {
        guard saveTask == nil else { return }
        saveTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 1_000_000_000)
            await self?.flush()
        }
    }

    /// Writes pending changes immediately (call before the app is suspended).
    func flush() {
        saveTask?.cancel()
        saveTask = nil
        guard loaded else { return }
        do {
            try FileManager.default.createDirectory(at: fileURL.deletingLastPathComponent(),
                                                    withIntermediateDirectories: true)
            let data = try JSONCoding.makeEncoder().encode(records)
            try data.write(to: fileURL, options: .atomic)
        } catch {
            // Non-fatal: state is rebuilt from the server via /api/assets/exist on the next run.
        }
    }
}
