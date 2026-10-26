import Foundation

enum APIError: LocalizedError, Sendable {
    case invalidServerURL
    case notLoggedIn
    case unauthorized
    case locked
    case http(status: Int, message: String?)
    case decoding(String)
    case transport(String)
    case invalidResponse

    var errorDescription: String? {
        switch self {
        case .invalidServerURL:
            return NSLocalizedString("error.invalidServerURL", comment: "")
        case .notLoggedIn:
            return NSLocalizedString("error.notLoggedIn", comment: "")
        case .unauthorized:
            return NSLocalizedString("error.unauthorized", comment: "")
        case .locked:
            return NSLocalizedString("error.locked", comment: "")
        case let .http(status, message):
            if let message, !message.isEmpty {
                return "\(message) (HTTP \(status))"
            }
            return String(format: NSLocalizedString("error.http", comment: ""), status)
        case let .decoding(details):
            return NSLocalizedString("error.decoding", comment: "") + "\n" + details
        case let .transport(details):
            return NSLocalizedString("error.transport", comment: "") + "\n" + details
        case .invalidResponse:
            return NSLocalizedString("error.invalidResponse", comment: "")
        }
    }

    var isUnauthorized: Bool {
        if case .unauthorized = self { return true }
        return false
    }

    var isLocked: Bool {
        if case .locked = self { return true }
        return false
    }
}
