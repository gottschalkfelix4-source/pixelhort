import CryptoKit
import Foundation

/// Streams a file through SHA-1 (the checksum the server uses for duplicate detection).
enum FileHasher {
    static func sha1Hex(of fileURL: URL) throws -> String {
        let handle = try FileHandle(forReadingFrom: fileURL)
        defer { try? handle.close() }
        var hasher = Insecure.SHA1()
        while true {
            let chunk: Data? = try autoreleasepool {
                try handle.read(upToCount: 4 << 20)
            }
            guard let chunk, !chunk.isEmpty else { break }
            hasher.update(data: chunk)
        }
        return hasher.finalize().map { String(format: "%02x", $0) }.joined()
    }
}
