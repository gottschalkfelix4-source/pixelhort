import Foundation
import Photos

/// Saves downloaded server assets into the iOS photo library.
enum PhotoLibrarySaver {
    enum SaveError: LocalizedError {
        case notAuthorized

        var errorDescription: String? {
            NSLocalizedString("error.photoLibraryDenied", comment: "")
        }
    }

    static func requestAddAuthorization() async -> Bool {
        let status = await PHPhotoLibrary.requestAuthorization(for: .addOnly)
        return status == .authorized || status == .limited
    }

    /// Saves an image or video file. When `pairedVideoURL` is set the two files are stored as a Live Photo.
    static func save(fileURL: URL, isVideo: Bool, pairedVideoURL: URL? = nil) async throws {
        guard await requestAddAuthorization() else { throw SaveError.notAuthorized }
        try await PHPhotoLibrary.shared().performChanges {
            let request = PHAssetCreationRequest.forAsset()
            let options = PHAssetResourceCreationOptions()
            options.shouldMoveFile = false
            if isVideo {
                request.addResource(with: .video, fileURL: fileURL, options: options)
            } else {
                request.addResource(with: .photo, fileURL: fileURL, options: options)
                if let pairedVideoURL {
                    let videoOptions = PHAssetResourceCreationOptions()
                    videoOptions.shouldMoveFile = false
                    request.addResource(with: .pairedVideo, fileURL: pairedVideoURL, options: videoOptions)
                }
            }
        }
    }
}
