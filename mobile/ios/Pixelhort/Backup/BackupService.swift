import Foundation
import Observation
import Photos

/// Automatic backup of the local photo library to the Pixelhort server.
///
/// Flow per asset: export original resource(s) → SHA-1 → `POST /api/assets/bulk-upload-check`
/// → multipart upload via background `URLSession`. Live Photos upload the paired video first
/// (hidden) and then the photo with `livePhotoVideoId`.
@MainActor
@Observable
final class BackupService {
    static let shared = BackupService()

    enum Status: Equatable {
        case idle
        case scanning
        case uploading
        case waitingForWiFi
        case noPermission
        case disabled
        case failed(String)
    }

    enum Trigger: String {
        case foreground
        case libraryChange
        case manual
        case backgroundRefresh
        case backgroundProcessing
    }

    struct RecentUpload: Identifiable, Equatable {
        let id = UUID()
        let fileName: String
        let date: Date
        let duplicate: Bool
    }

    // Observable state for the progress screen.
    private(set) var status: Status = .idle
    private(set) var totalAssets = 0
    private(set) var backedUpAssets = 0
    private(set) var remainingAssets = 0
    private(set) var currentFileName: String?
    private(set) var currentProgress: Double = 0
    private(set) var lastRunAt: Date?
    private(set) var lastError: String?
    private(set) var recentUploads: [RecentUpload] = []
    private(set) var failedCount = 0

    /// Tracked storage; `settings` wraps it so changes are persisted and applied.
    private var storedSettings: BackupSettings

    var settings: BackupSettings {
        get { storedSettings }
        set {
            let oldValue = storedSettings
            guard newValue != oldValue else { return }
            storedSettings = newValue
            newValue.save()
            if newValue.isEnabled {
                if status == .disabled { status = .idle }
                start(.manual)
            } else {
                stop()
                status = .disabled
            }
        }
    }

    var isRunning: Bool { runTask != nil }

    private var runTask: Task<Void, Never>?
    private var runID = UUID()
    private var debounceTask: Task<Void, Never>?
    private let store = UploadStateStore.shared
    private let uploader = BackupUploader.shared
    private var libraryObserver: PhotoLibraryObserver?

    init() {
        let loaded = BackupSettings.load()
        storedSettings = loaded
        status = loaded.isEnabled ? .idle : .disabled
        uploader.onProgress = { fileName, fraction in
            Task { @MainActor in
                BackupService.shared.updateProgress(fileName: fileName, fraction: fraction)
            }
        }
        NetworkMonitor.shared.onChange = { isConnected, isExpensive in
            Task { @MainActor in
                BackupService.shared.networkChanged(isConnected: isConnected, isExpensive: isExpensive)
            }
        }
    }

    private func networkChanged(isConnected: Bool, isExpensive: Bool) {
        guard status == .waitingForWiFi, isConnected, !isExpensive || !settings.wifiOnly else { return }
        status = .idle
        start(.foreground)
    }

    private func updateProgress(fileName: String, fraction: Double) {
        currentFileName = fileName
        currentProgress = fraction
    }

    // MARK: Permission

    var authorizationStatus: PHAuthorizationStatus {
        PHPhotoLibrary.authorizationStatus(for: .readWrite)
    }

    func requestAuthorization() async -> Bool {
        let status = await PHPhotoLibrary.requestAuthorization(for: .readWrite)
        let granted = status == .authorized || status == .limited
        if granted { startObservingLibrary() }
        return granted
    }

    func startObservingLibrary() {
        guard libraryObserver == nil,
              authorizationStatus == .authorized || authorizationStatus == .limited else { return }
        let observer = PhotoLibraryObserver {
            Task { @MainActor in
                BackupService.shared.libraryDidChange()
            }
        }
        PHPhotoLibrary.shared().register(observer)
        libraryObserver = observer
    }

    private func libraryDidChange() {
        debounceTask?.cancel()
        debounceTask = Task {
            try? await Task.sleep(nanoseconds: 5_000_000_000)
            guard !Task.isCancelled else { return }
            start(.libraryChange)
        }
    }

    // MARK: Control

    /// Starts a backup run unless one is already running.
    func start(_ trigger: Trigger) {
        guard runTask == nil else { return }
        let waitForUploads = trigger != .backgroundRefresh
        let id = UUID()
        runID = id
        runTask = Task {
            await run(trigger: trigger, waitForUploads: waitForUploads)
            if runID == id {
                runTask = nil
            }
        }
    }

    /// Runs a backup pass and returns when it finished (used by BGTasks).
    func runToCompletion(_ trigger: Trigger) async {
        if let runTask {
            await runTask.value
            return
        }
        start(trigger)
        await runTask?.value
    }

    func stop() {
        runTask?.cancel()
        runTask = nil
        debounceTask?.cancel()
        if status == .scanning || status == .uploading {
            status = .idle
        }
        currentFileName = nil
        currentProgress = 0
    }

    /// Cancels running uploads and forgets local state (used after changing albums or on "reset").
    func resetState() async {
        stop()
        uploader.cancelAll()
        await store.removeAll()
        recentUploads = []
        await refreshCounts()
    }

    // MARK: Collecting assets

    private func fetchOptions() -> PHFetchOptions {
        let options = PHFetchOptions()
        options.sortDescriptors = [NSSortDescriptor(key: "creationDate", ascending: false)]
        options.includeHiddenAssets = false
        if settings.includeVideos {
            options.predicate = NSPredicate(format: "mediaType == %d OR mediaType == %d",
                                            PHAssetMediaType.image.rawValue, PHAssetMediaType.video.rawValue)
        } else {
            options.predicate = NSPredicate(format: "mediaType == %d", PHAssetMediaType.image.rawValue)
        }
        return options
    }

    /// All assets selected for backup, newest first, de-duplicated by `localIdentifier`.
    func selectedAssets() -> [PHAsset] {
        var result: [PHAsset] = []
        var seen = Set<String>()
        func collect(_ fetch: PHFetchResult<PHAsset>) {
            fetch.enumerateObjects { asset, _, _ in
                if seen.insert(asset.localIdentifier).inserted {
                    result.append(asset)
                }
            }
        }
        if settings.backupAllPhotos {
            collect(PHAsset.fetchAssets(with: fetchOptions()))
        } else if !settings.selectedAlbumIds.isEmpty {
            let collections = PHAssetCollection.fetchAssetCollections(
                withLocalIdentifiers: settings.selectedAlbumIds, options: nil)
            collections.enumerateObjects { collection, _, _ in
                collect(PHAsset.fetchAssets(in: collection, options: self.fetchOptions()))
            }
        }
        return result.sorted { ($0.creationDate ?? .distantPast) > ($1.creationDate ?? .distantPast) }
    }

    func refreshCounts() async {
        guard authorizationStatus == .authorized || authorizationStatus == .limited else { return }
        let assets = selectedAssets()
        var done = 0
        for asset in assets {
            if await store.isBackedUp(asset.localIdentifier, modificationDate: asset.modificationDate) {
                done += 1
            }
        }
        totalAssets = assets.count
        backedUpAssets = done
        remainingAssets = max(0, assets.count - done)
        failedCount = await store.statistics().failed
    }

    // MARK: Run

    private func run(trigger: Trigger, waitForUploads: Bool) async {
        guard settings.isEnabled else {
            status = .disabled
            return
        }
        guard authorizationStatus == .authorized || authorizationStatus == .limited else {
            status = .noPermission
            return
        }
        guard let api = AppSession.shared.api else { return }
        startObservingLibrary()
        if waitForUploads, settings.wifiOnly, NetworkMonitor.shared.isExpensive {
            // Uploads would only wait for Wi-Fi; resume when the network changes.
            status = .waitingForWiFi
            return
        }

        status = .scanning
        lastError = nil
        let deviceId = AppSession.shared.deviceId
        let assets = selectedAssets()
        totalAssets = assets.count

        // Uploads still running in the background session must not be started twice.
        let active = await uploader.activeLocalIdentifiers()
        await store.resetStaleQueued(keeping: active)

        var pending: [PHAsset] = []
        for asset in assets {
            if active.contains(asset.localIdentifier) { continue }
            if await store.isBackedUp(asset.localIdentifier, modificationDate: asset.modificationDate) { continue }
            pending.append(asset)
        }

        // Fast path after reinstall: ask the server which device asset ids it already knows.
        if !pending.isEmpty {
            var known = Set<String>()
            let ids = pending.map(\.localIdentifier)
            var index = 0
            while index < ids.count {
                let chunk = Array(ids[index..<min(index + 500, ids.count)])
                index += 500
                if let existing = try? await api.existingAssets(deviceId: deviceId, deviceAssetIds: chunk) {
                    known.formUnion(existing)
                }
            }
            if !known.isEmpty {
                for asset in pending where known.contains(asset.localIdentifier) {
                    let record = await store.record(for: asset.localIdentifier)
                    // Only trust the server for assets we never saw modified locally.
                    if record == nil {
                        await store.markUploaded(asset.localIdentifier, checksum: nil, remoteAssetId: nil,
                                                 modificationDate: asset.modificationDate)
                    }
                }
                var stillPending: [PHAsset] = []
                for asset in pending {
                    let backedUp = await store.isBackedUp(asset.localIdentifier, modificationDate: asset.modificationDate)
                    if !backedUp {
                        stillPending.append(asset)
                    }
                }
                pending = stillPending
            }
        }

        backedUpAssets = max(0, assets.count - pending.count - active.count)
        remainingAssets = pending.count + active.count
        lastRunAt = Date()

        if pending.isEmpty {
            status = .idle
            failedCount = await store.statistics().failed
            await store.flush()
            return
        }

        status = .uploading
        // In short background windows only enqueue a limited number of uploads.
        let limit = waitForUploads ? pending.count : min(pending.count, 25)
        for asset in pending.prefix(limit) {
            if Task.isCancelled { break }
            do {
                let finished = try await backup(asset: asset, api: api, deviceId: deviceId, waitForUploads: waitForUploads)
                if finished {
                    backedUpAssets += 1
                }
                remainingAssets = max(0, remainingAssets - 1)
            } catch is CancellationError {
                break
            } catch {
                lastError = error.localizedDescription
                await store.markFailed(asset.localIdentifier, error: error.localizedDescription)
            }
        }

        currentFileName = nil
        currentProgress = 0
        failedCount = await store.statistics().failed
        await store.flush()
        if Task.isCancelled {
            status = .idle
        } else if let lastError, failedCount > 0 {
            status = .failed(lastError)
        } else {
            status = .idle
        }
    }

    /// Backs up one asset. Returns true when the asset is on the server afterwards.
    private func backup(asset: PHAsset, api: APIClient, deviceId: String, waitForUploads: Bool) async throws -> Bool {
        let localId = asset.localIdentifier
        let allowsCellular = !settings.wifiOnly
        let record = await store.record(for: localId)

        // 1. Live Photo: upload the paired video first (always, independent of "include videos").
        var livePhotoVideoId = record?.livePhotoVideoRemoteId
        if livePhotoVideoId == nil, let videoResource = AssetExporter.pairedVideoResource(for: asset) {
            let exported = try await AssetExporter.export(videoResource)
            defer { AssetExporter.cleanup(exported) }
            currentFileName = exported.fileName
            if let existingId = try await duplicateId(for: exported, api: api) {
                livePhotoVideoId = existingId
                await store.setLivePhotoVideo(localId, remoteId: existingId, checksum: exported.checksum)
            } else {
                let response = try await upload(exported, asset: asset, api: api, deviceId: deviceId,
                                                deviceAssetId: localId + "#live-video",
                                                role: .livePhotoVideo, livePhotoVideoId: nil,
                                                visibility: .hidden, allowsCellular: allowsCellular,
                                                wait: waitForUploads)
                guard let response else {
                    // Enqueued only; the photo follows on the next run once the video id is known.
                    await store.markQueued(localId, checksum: nil, modificationDate: asset.modificationDate)
                    return false
                }
                livePhotoVideoId = response.id
                await store.setLivePhotoVideo(localId, remoteId: response.id, checksum: exported.checksum)
            }
        }

        // 2. The primary photo / video resource.
        guard let primary = AssetExporter.primaryResource(for: asset) else {
            throw AssetExportError.noResource
        }
        let exported = try await AssetExporter.export(primary)
        defer { AssetExporter.cleanup(exported) }
        currentFileName = exported.fileName
        currentProgress = 0

        if let existingId = try await duplicateId(for: exported, api: api) {
            await store.markUploaded(localId, checksum: exported.checksum, remoteAssetId: existingId,
                                     modificationDate: asset.modificationDate)
            appendRecent(exported.fileName, duplicate: true)
            return true
        }

        let response = try await upload(exported, asset: asset, api: api, deviceId: deviceId,
                                        deviceAssetId: localId, role: .primary,
                                        livePhotoVideoId: livePhotoVideoId, visibility: nil,
                                        allowsCellular: allowsCellular, wait: waitForUploads)
        guard let response else {
            await store.markQueued(localId, checksum: exported.checksum, modificationDate: asset.modificationDate)
            return false
        }
        // The delegate already persisted the result; keep the in-memory list for the UI.
        appendRecent(exported.fileName, duplicate: response.isDuplicate)
        return true
    }

    /// Returns the server asset id if the checksum is already known (`reject`/`duplicate`).
    private func duplicateId(for resource: ExportedResource, api: APIClient) async throws -> String? {
        let results = try await api.bulkUploadCheck([BulkUploadCheckItem(id: "1", checksum: resource.checksum)])
        guard let result = results.first, !result.isAccepted else { return nil }
        return result.assetId ?? ""
    }

    private func upload(_ resource: ExportedResource, asset: PHAsset, api: APIClient, deviceId: String,
                        deviceAssetId: String, role: UploadTaskContext.Role, livePhotoVideoId: String?,
                        visibility: AssetVisibility?, allowsCellular: Bool, wait: Bool) async throws -> UploadResponse? {
        let created = asset.creationDate ?? Date()
        let modified = asset.modificationDate ?? created
        var fields: [(name: String, value: String)] = [
            ("deviceAssetId", deviceAssetId),
            ("deviceId", deviceId),
            ("fileCreatedAt", JSONCoding.formatDate(created)),
            ("fileModifiedAt", JSONCoding.formatDate(modified)),
            ("isFavorite", (asset.isFavorite && role == .primary) ? "true" : "false"),
        ]
        if let livePhotoVideoId, !livePhotoVideoId.isEmpty {
            fields.append(("livePhotoVideoId", livePhotoVideoId))
        }
        if let visibility {
            fields.append(("visibility", visibility.rawValue))
        }
        let bodyPath = BackupUploader.bodyDirectory
            .appendingPathComponent(UUID().uuidString + ".multipart").path
        let context = UploadTaskContext(localIdentifier: asset.localIdentifier, role: role,
                                        checksum: resource.checksum, bodyFilePath: bodyPath,
                                        fileName: resource.fileName, modificationDate: asset.modificationDate)
        return try await uploader.upload(api: api, resource: resource, fields: fields, context: context,
                                         allowsCellular: allowsCellular, waitForCompletion: wait)
    }

    private func appendRecent(_ fileName: String, duplicate: Bool) {
        recentUploads.insert(RecentUpload(fileName: fileName, date: Date(), duplicate: duplicate), at: 0)
        if recentUploads.count > 30 {
            recentUploads.removeLast(recentUploads.count - 30)
        }
    }
}

/// Bridges `PHPhotoLibraryChangeObserver` (called on a background queue) to the backup service.
final class PhotoLibraryObserver: NSObject, PHPhotoLibraryChangeObserver, @unchecked Sendable {
    private let onChange: @Sendable () -> Void

    init(onChange: @escaping @Sendable () -> Void) {
        self.onChange = onChange
    }

    func photoLibraryDidChange(_ changeInstance: PHChange) {
        onChange()
    }
}
