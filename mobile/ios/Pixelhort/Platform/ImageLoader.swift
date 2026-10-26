import CryptoKit
import Foundation
import UIKit

/// Thread-safe wrapper so the memory cache can be read synchronously from views.
final class MemoryImageCache: @unchecked Sendable {
    private let cache = NSCache<NSString, UIImage>()

    init(countLimit: Int, totalCostLimit: Int) {
        cache.countLimit = countLimit
        cache.totalCostLimit = totalCostLimit
    }

    func image(forKey key: String) -> UIImage? {
        cache.object(forKey: key as NSString)
    }

    func set(_ image: UIImage, forKey key: String) {
        let cost = Int(image.size.width * image.size.height * image.scale * image.scale * 4)
        cache.setObject(image, forKey: key as NSString, cost: cost)
    }

    func removeAll() {
        cache.removeAllObjects()
    }
}

/// Authenticated image loading with an in-memory (`NSCache`) and on-disk cache.
actor ImageLoader {
    static let shared = ImageLoader()

    private nonisolated let memory = MemoryImageCache(countLimit: 600, totalCostLimit: 150 * 1024 * 1024)
    private var inFlight: [String: Task<UIImage?, Never>] = [:]
    private let diskDirectory: URL

    init() {
        let caches = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
        diskDirectory = caches.appendingPathComponent("images", isDirectory: true)
        try? FileManager.default.createDirectory(at: diskDirectory, withIntermediateDirectories: true)
    }

    static func assetKey(_ assetId: String, _ size: ThumbnailSize) -> String {
        "asset-\(assetId)-\(size.rawValue)"
    }

    static func personKey(_ personId: String) -> String {
        "person-\(personId)"
    }

    /// Synchronous memory-cache lookup (safe to call from views; `NSCache` is thread-safe).
    nonisolated func cachedImage(forKey key: String) -> UIImage? {
        memory.image(forKey: key)
    }

    func assetImage(_ assetId: String, size: ThumbnailSize, api: APIClient) async -> UIImage? {
        await image(forKey: Self.assetKey(assetId, size)) {
            try await api.thumbnailData(assetId: assetId, size: size)
        }
    }

    func personImage(_ personId: String, api: APIClient) async -> UIImage? {
        await image(forKey: Self.personKey(personId)) {
            try await api.data(api.personThumbnailPath(id: personId))
        }
    }

    func image(forKey key: String, fetch: @escaping @Sendable () async throws -> Data) async -> UIImage? {
        if let cached = memory.image(forKey: key) {
            return cached
        }
        if let task = inFlight[key] {
            return await task.value
        }
        let fileURL = diskURL(for: key)
        let task = Task<UIImage?, Never> {
            if let data = try? Data(contentsOf: fileURL), let image = UIImage(data: data) {
                return await image.byPreparingForDisplay() ?? image
            }
            guard let data = try? await fetch(), let image = UIImage(data: data) else {
                return nil
            }
            try? data.write(to: fileURL, options: .atomic)
            return await image.byPreparingForDisplay() ?? image
        }
        inFlight[key] = task
        let image = await task.value
        inFlight[key] = nil
        if let image {
            memory.set(image, forKey: key)
        }
        return image
    }

    func evict(prefix: String) {
        memory.removeAll()
        let files = (try? FileManager.default.contentsOfDirectory(at: diskDirectory, includingPropertiesForKeys: nil)) ?? []
        for file in files where file.lastPathComponent.hasPrefix(prefix) {
            try? FileManager.default.removeItem(at: file)
        }
    }

    func clearAll() {
        memory.removeAll()
        try? FileManager.default.removeItem(at: diskDirectory)
        try? FileManager.default.createDirectory(at: diskDirectory, withIntermediateDirectories: true)
    }

    func diskUsageBytes() -> Int64 {
        let files = (try? FileManager.default.contentsOfDirectory(
            at: diskDirectory, includingPropertiesForKeys: [.fileSizeKey])) ?? []
        return files.reduce(Int64(0)) { sum, url in
            let size = (try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
            return sum + Int64(size)
        }
    }

    private func diskURL(for key: String) -> URL {
        let digest = Insecure.SHA1.hash(data: Data(key.utf8)).map { String(format: "%02x", $0) }.joined()
        let prefix = key.hasPrefix("person-") ? "person-" : "asset-"
        return diskDirectory.appendingPathComponent(prefix + digest)
    }
}
