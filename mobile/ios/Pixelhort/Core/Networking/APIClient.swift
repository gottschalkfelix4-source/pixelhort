import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

enum HTTPMethod: String, Sendable {
    case get = "GET"
    case post = "POST"
    case put = "PUT"
    case patch = "PATCH"
    case delete = "DELETE"
}

/// Thin async/await REST client for the Pixelhort API (see docs/API.md).
///
/// The client is immutable; a new instance is created whenever the server URL or the
/// access token changes.
final class APIClient: @unchecked Sendable {
    let baseURL: URL
    let accessToken: String?
    private let session: URLSession

    init(baseURL: URL, accessToken: String? = nil, session: URLSession = .shared) {
        self.baseURL = baseURL
        self.accessToken = accessToken
        self.session = session
    }

    func withToken(_ token: String?) -> APIClient {
        APIClient(baseURL: baseURL, accessToken: token, session: session)
    }

    // MARK: - URL building

    /// Normalizes user input like `192.168.1.5:2342` or `https://photos.example.com/` into a base URL.
    static func normalizeServerURL(_ input: String) -> URL? {
        var text = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return nil }
        if !text.lowercased().hasPrefix("http://") && !text.lowercased().hasPrefix("https://") {
            text = "http://" + text
        }
        while text.hasSuffix("/") { text.removeLast() }
        if text.lowercased().hasSuffix("/api") { text.removeLast(4) }
        guard let url = URL(string: text),
              let scheme = url.scheme?.lowercased(), scheme == "http" || scheme == "https",
              let host = url.host, !host.isEmpty else {
            return nil
        }
        return url
    }

    func url(_ path: String, query: [URLQueryItem] = []) -> URL {
        var absolute = baseURL.absoluteString
        while absolute.hasSuffix("/") { absolute.removeLast() }
        let normalizedPath = path.hasPrefix("/") ? path : "/" + path
        var components = URLComponents(string: absolute + normalizedPath) ?? URLComponents()
        if !query.isEmpty {
            components.queryItems = (components.queryItems ?? []) + query
        }
        return components.url ?? baseURL
    }

    /// Headers needed to fetch protected media (e.g. for `AVURLAsset`).
    var authHeaders: [String: String] {
        guard let accessToken else { return [:] }
        return ["Authorization": "Bearer \(accessToken)"]
    }

    func authorize(_ request: inout URLRequest) {
        for (key, value) in authHeaders {
            request.setValue(value, forHTTPHeaderField: key)
        }
    }

    func makeRequest(_ method: HTTPMethod, _ path: String, query: [URLQueryItem] = [],
                     body: Data? = nil, contentType: String? = "application/json") -> URLRequest {
        var request = URLRequest(url: url(path, query: query))
        request.httpMethod = method.rawValue
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        if let body {
            request.httpBody = body
            if let contentType {
                request.setValue(contentType, forHTTPHeaderField: "Content-Type")
            }
        }
        authorize(&request)
        return request
    }

    // MARK: - Core request execution

    func perform(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: request)
        } catch is CancellationError {
            throw CancellationError()
        } catch let error as URLError where error.code == .cancelled {
            throw CancellationError()
        } catch {
            throw APIError.transport(error.localizedDescription)
        }
        guard let http = response as? HTTPURLResponse else {
            throw APIError.invalidResponse
        }
        try Self.validate(status: http.statusCode, data: data)
        return (data, http)
    }

    static func validate(status: Int, data: Data) throws {
        guard !(200..<300).contains(status) else { return }
        let body = try? JSONCoding.makeDecoder().decode(APIErrorBody.self, from: data)
        switch status {
        case 401:
            throw APIError.unauthorized
        case 403 where body?.message == "locked":
            throw APIError.locked
        default:
            throw APIError.http(status: status, message: body?.message)
        }
    }

    func decode<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
        do {
            return try JSONCoding.makeDecoder().decode(T.self, from: data)
        } catch {
            throw APIError.decoding(String(describing: error))
        }
    }

    func encode(_ body: any Encodable) throws -> Data {
        try JSONCoding.makeEncoder().encode(body)
    }

    // MARK: - Convenience

    func get<T: Decodable>(_ path: String, query: [URLQueryItem] = [], as type: T.Type = T.self) async throws -> T {
        let (data, _) = try await perform(makeRequest(.get, path, query: query))
        return try decode(T.self, from: data)
    }

    func send<T: Decodable>(_ method: HTTPMethod, _ path: String, query: [URLQueryItem] = [],
                            body: (any Encodable)? = nil, as type: T.Type = T.self) async throws -> T {
        let data = try body.map { try encode($0) }
        let (responseData, _) = try await perform(makeRequest(method, path, query: query, body: data))
        return try decode(T.self, from: responseData)
    }

    /// For endpoints answering `204 No Content` (or whose body we ignore).
    func sendVoid(_ method: HTTPMethod, _ path: String, query: [URLQueryItem] = [],
                  body: (any Encodable)? = nil) async throws {
        let data = try body.map { try encode($0) }
        _ = try await perform(makeRequest(method, path, query: query, body: data))
    }

    /// Fetches raw bytes (thumbnails, face crops, ...).
    func data(_ path: String, query: [URLQueryItem] = []) async throws -> Data {
        var request = makeRequest(.get, path, query: query)
        request.setValue("*/*", forHTTPHeaderField: "Accept")
        let (data, _) = try await perform(request)
        return data
    }

    /// Downloads a protected file to a temporary location named `fileName`.
    func download(_ path: String, query: [URLQueryItem] = [], fileName: String) async throws -> URL {
        var request = makeRequest(.get, path, query: query)
        request.setValue("*/*", forHTTPHeaderField: "Accept")
        let temporaryURL: URL
        let response: URLResponse
        do {
            (temporaryURL, response) = try await session.download(for: request)
        } catch let error as URLError where error.code == .cancelled {
            throw CancellationError()
        } catch {
            throw APIError.transport(error.localizedDescription)
        }
        guard let http = response as? HTTPURLResponse else { throw APIError.invalidResponse }
        try Self.validate(status: http.statusCode, data: Data())
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("downloads", isDirectory: true)
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let safeName = fileName.replacingOccurrences(of: "/", with: "_")
        let destination = directory.appendingPathComponent(safeName.isEmpty ? "download" : safeName)
        try FileManager.default.moveItem(at: temporaryURL, to: destination)
        return destination
    }
}
