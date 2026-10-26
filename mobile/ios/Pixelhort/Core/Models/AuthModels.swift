import Foundation

// MARK: - Server

struct PingResponse: Decodable, Sendable {
    let res: String
}

struct ServerFeatures: Decodable, Hashable, Sendable {
    var smartSearch: Bool?
    var faces: Bool?
    var ocr: Bool?
    var objects: Bool?
    var oauth: Bool?
    var reverseGeocoding: Bool?
    var cast: Bool?
}

struct ServerInfo: Decodable, Hashable, Sendable {
    var version: String?
    var features: ServerFeatures?
    var publicUrl: String?
}

// MARK: - Auth

struct AuthStatus: Decodable, Hashable, Sendable {
    var setupRequired: Bool
    var oauthEnabled: Bool
    var oauthButtonText: String?
    var passwordLoginEnabled: Bool

    init(setupRequired: Bool, oauthEnabled: Bool, oauthButtonText: String?, passwordLoginEnabled: Bool) {
        self.setupRequired = setupRequired
        self.oauthEnabled = oauthEnabled
        self.oauthButtonText = oauthButtonText
        self.passwordLoginEnabled = passwordLoginEnabled
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        setupRequired = (try? c.decodeIfPresent(Bool.self, forKey: .setupRequired)) ?? false
        oauthEnabled = (try? c.decodeIfPresent(Bool.self, forKey: .oauthEnabled)) ?? false
        oauthButtonText = try? c.decodeIfPresent(String.self, forKey: .oauthButtonText)
        passwordLoginEnabled = (try? c.decodeIfPresent(Bool.self, forKey: .passwordLoginEnabled)) ?? true
    }

    private enum CodingKeys: String, CodingKey {
        case setupRequired, oauthEnabled, oauthButtonText, passwordLoginEnabled
    }
}

struct LoginRequest: Encodable, Sendable {
    let email: String
    let password: String
    var deviceType: String = "ios"
    var deviceName: String?
}

struct LoginResponse: Decodable, Sendable {
    let accessToken: String
    let user: UserDto
}

struct OAuthAuthorizeResponse: Decodable, Sendable {
    let url: String
}

struct OAuthCallbackRequest: Encodable, Sendable {
    let url: String
}

struct PinRequest: Encodable, Sendable {
    let pin: String
}

struct SetPinRequest: Encodable, Sendable {
    let pin: String
    var currentPin: String?
}

struct UnlockResponse: Decodable, Sendable {
    var unlockedUntil: Date?
}

struct SessionInfo: Decodable, Sendable {
    let id: String
    var unlockedUntil: Date?
    var hasPin: Bool?

    var isUnlocked: Bool {
        guard let until = unlockedUntil else { return false }
        return until > Date()
    }
}

// MARK: - Users

struct UserDto: Decodable, Identifiable, Hashable, Sendable {
    let id: String
    var email: String
    var name: String
    var isAdmin: Bool?
    var quotaBytes: Int64?
    var usageBytes: Int64?
    var hasPin: Bool?
    var oauthId: String?
    var createdAt: Date?
    var updatedAt: Date?
}

struct UserSummary: Codable, Identifiable, Hashable, Sendable {
    let id: String
    var name: String
    var email: String?
}

struct UpdateMeRequest: Encodable, Sendable {
    var name: String?
    var email: String?
    var password: String?
    var currentPassword: String?
}

// MARK: - Errors

struct APIErrorBody: Decodable, Sendable {
    var statusCode: Int?
    var error: String?
    var message: String?
}
