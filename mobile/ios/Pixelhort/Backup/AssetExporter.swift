import Foundation
import Photos
import UniformTypeIdentifiers

/// An original resource of a `PHAsset` exported to a temporary file.
struct ExportedResource: Sendable {
    let fileURL: URL
    let fileName: String
    let mimeType: String
    let checksum: String
    let isVideo: Bool
}

enum AssetExportError: LocalizedError {
    case noResource
    case cancelled

    var errorDescription: String? {
        switch self {
        case .noResource: return NSLocalizedString("error.noResource", comment: "")
        case .cancelled: return NSLocalizedString("error.cancelled", comment: "")
        }
    }
}

/// Exports original asset data via `PHAssetResourceManager` (downloading from iCloud if needed).
enum AssetExporter {
    static var workDirectory: URL {
        FileManager.default.temporaryDirectory.appendingPathComponent("backup-export", isDirectory: true)
    }

    /// The original photo/video resource (unedited original, as Immich/Google Photos do).
    static func primaryResource(for asset: PHAsset) -> PHAssetResource? {
        let resources = PHAssetResource.assetResources(for: asset)
        let preferred: [PHAssetResourceType] = asset.mediaType == .video
            ? [.video, .fullSizeVideo]
            : [.photo, .fullSizePhoto, .alternatePhoto]
        for type in preferred {
            if let match = resources.first(where: { $0.type == type }) {
                return match
            }
        }
        return resources.first
    }

    /// The video half of a Live Photo, if any.
    static func pairedVideoResource(for asset: PHAsset) -> PHAssetResource? {
        guard asset.mediaSubtypes.contains(.photoLive) else { return nil }
        let resources = PHAssetResource.assetResources(for: asset)
        return resources.first(where: { $0.type == .pairedVideo })
            ?? resources.first(where: { $0.type == .fullSizePairedVideo })
    }

    static func export(_ resource: PHAssetResource) async throws -> ExportedResource {
        let directory = workDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let fileName = resource.originalFilename.isEmpty ? "asset" : resource.originalFilename
        let fileURL = directory.appendingPathComponent(fileName)

        let options = PHAssetResourceRequestOptions()
        options.isNetworkAccessAllowed = true

        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            PHAssetResourceManager.default().writeData(for: resource, toFile: fileURL, options: options) { error in
                if let error {
                    continuation.resume(throwing: error)
                } else {
                    continuation.resume()
                }
            }
        }
        try Task.checkCancellation()

        let checksum = try FileHasher.sha1Hex(of: fileURL)
        let type = UTType(resource.uniformTypeIdentifier)
        let mimeType = type?.preferredMIMEType
            ?? UTType(filenameExtension: fileURL.pathExtension)?.preferredMIMEType
            ?? "application/octet-stream"
        let isVideo = type?.conforms(to: .movie) ?? false
        return ExportedResource(fileURL: fileURL, fileName: fileName, mimeType: mimeType,
                                checksum: checksum, isVideo: isVideo)
    }

    static func cleanup(_ resource: ExportedResource?) {
        guard let resource else { return }
        try? FileManager.default.removeItem(at: resource.fileURL.deletingLastPathComponent())
    }
}
