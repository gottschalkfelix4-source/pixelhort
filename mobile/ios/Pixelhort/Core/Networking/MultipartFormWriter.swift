import Foundation

/// Writes a `multipart/form-data` body to a file so it can be sent with a background
/// `URLSession` upload task (`uploadTask(with:fromFile:)`), which is the only upload
/// type that survives app suspension.
struct MultipartFormWriter: Sendable {
    let boundary: String

    init(boundary: String = "PixelhortBoundary-\(UUID().uuidString)") {
        self.boundary = boundary
    }

    var contentType: String { "multipart/form-data; boundary=\(boundary)" }

    struct FilePart: Sendable {
        let fieldName: String
        let fileURL: URL
        let fileName: String
        let mimeType: String
    }

    /// Writes all text fields followed by the file parts to `output`. Returns the body size in bytes.
    @discardableResult
    func write(fields: [(name: String, value: String)], files: [FilePart], to output: URL) throws -> Int64 {
        let fileManager = FileManager.default
        if fileManager.fileExists(atPath: output.path) {
            try fileManager.removeItem(at: output)
        }
        try fileManager.createDirectory(at: output.deletingLastPathComponent(), withIntermediateDirectories: true)
        guard fileManager.createFile(atPath: output.path, contents: nil) else {
            throw CocoaError(.fileWriteUnknown)
        }
        let handle = try FileHandle(forWritingTo: output)
        defer { try? handle.close() }

        var written: Int64 = 0
        func emit(_ string: String) throws {
            let data = Data(string.utf8)
            try handle.write(contentsOf: data)
            written += Int64(data.count)
        }

        for field in fields {
            try emit("--\(boundary)\r\n")
            try emit("Content-Disposition: form-data; name=\"\(Self.escape(field.name))\"\r\n\r\n")
            try emit("\(field.value)\r\n")
        }

        for file in files {
            try emit("--\(boundary)\r\n")
            try emit("Content-Disposition: form-data; name=\"\(Self.escape(file.fieldName))\"; "
                     + "filename=\"\(Self.escape(file.fileName))\"\r\n")
            try emit("Content-Type: \(file.mimeType)\r\n\r\n")

            let input = try FileHandle(forReadingFrom: file.fileURL)
            defer { try? input.close() }
            while true {
                guard let chunk = try input.read(upToCount: 1 << 20), !chunk.isEmpty else { break }
                try handle.write(contentsOf: chunk)
                written += Int64(chunk.count)
            }
            try emit("\r\n")
        }

        try emit("--\(boundary)--\r\n")
        return written
    }

    private static func escape(_ value: String) -> String {
        value
            .replacingOccurrences(of: "\r", with: "")
            .replacingOccurrences(of: "\n", with: "")
            .replacingOccurrences(of: "\"", with: "%22")
    }
}
