import Foundation

/// User configurable backup options (persisted in `UserDefaults`).
struct BackupSettings: Codable, Equatable, Sendable {
    var isEnabled: Bool = false
    /// When true, the whole library ("Recents") is backed up and `selectedAlbumIds` is ignored.
    var backupAllPhotos: Bool = true
    /// `PHAssetCollection.localIdentifier`s chosen by the user.
    var selectedAlbumIds: [String] = []
    var wifiOnly: Bool = true
    var includeVideos: Bool = true
    var backgroundBackup: Bool = true

    private static let key = "backupSettings.v1"

    static func load(from defaults: UserDefaults = .standard) -> BackupSettings {
        guard let data = defaults.data(forKey: key),
              let settings = try? JSONDecoder().decode(BackupSettings.self, from: data) else {
            return BackupSettings()
        }
        return settings
    }

    func save(to defaults: UserDefaults = .standard) {
        if let data = try? JSONEncoder().encode(self) {
            defaults.set(data, forKey: Self.key)
        }
    }
}

/// Context attached to every background upload task (`URLSessionTask.taskDescription`)
/// so completions can be attributed even after the app was relaunched by the system.
struct UploadTaskContext: Codable, Sendable {
    enum Role: String, Codable, Sendable {
        case primary
        case livePhotoVideo
    }

    let localIdentifier: String
    let role: Role
    let checksum: String
    let bodyFilePath: String
    let fileName: String
    let modificationDate: Date?

    func encoded() -> String? {
        guard let data = try? JSONCoding.makeEncoder().encode(self) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    static func decode(_ string: String?) -> UploadTaskContext? {
        guard let string, let data = string.data(using: .utf8) else { return nil }
        return try? JSONCoding.makeDecoder().decode(UploadTaskContext.self, from: data)
    }
}
