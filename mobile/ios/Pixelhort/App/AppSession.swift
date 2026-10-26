import Foundation
import Observation
import UIKit

/// Global login/server state. Restored from UserDefaults + Keychain so background tasks
/// (which launch the app without UI) can use the API as well.
@MainActor
@Observable
final class AppSession {
    static let shared = AppSession()

    enum Phase: Equatable {
        case serverSetup
        case login
        case ready
    }

    private(set) var phase: Phase = .serverSetup
    private(set) var serverURL: URL?
    private(set) var authStatus: AuthStatus?
    private(set) var serverInfo: ServerInfo?
    private(set) var user: UserDto?
    /// Server-side user preferences (feature toggles); defaults until loaded.
    private(set) var preferences = UserPreferences()
    private(set) var api: APIClient?

    let deviceId: String

    private static let serverURLKey = "serverURL"
    static let oauthRedirectURI = "app.pixelhort:/oauth-callback"
    static let oauthCallbackScheme = "app.pixelhort"

    init() {
        deviceId = DeviceIdentity.deviceId
        if let stored = UserDefaults.standard.string(forKey: Self.serverURLKey),
           let url = URL(string: stored) {
            serverURL = url
            if let token = KeychainStore.string(for: .accessToken) {
                api = APIClient(baseURL: url, accessToken: token)
                phase = .ready
            } else {
                phase = .login
            }
        }
    }

    /// Client without credentials for the configured server (ping, auth status, login).
    var anonymousClient: APIClient? {
        serverURL.map { APIClient(baseURL: $0) }
    }

    func requireAPI() throws -> APIClient {
        guard let api else { throw APIError.notLoggedIn }
        return api
    }

    var isAdmin: Bool { user?.isAdmin ?? false }

    // MARK: Onboarding

    func configureServer(url: URL, status: AuthStatus) {
        serverURL = url
        authStatus = status
        UserDefaults.standard.set(url.absoluteString, forKey: Self.serverURLKey)
        phase = .login
    }

    func changeServer() {
        Task { await logout() }
        UserDefaults.standard.removeObject(forKey: Self.serverURLKey)
        serverURL = nil
        authStatus = nil
        phase = .serverSetup
    }

    func refreshAuthStatus() async {
        guard let client = anonymousClient else { return }
        authStatus = try? await client.authStatus()
    }

    func completeLogin(_ response: LoginResponse) {
        guard let serverURL else { return }
        KeychainStore.set(response.accessToken, for: .accessToken)
        api = APIClient(baseURL: serverURL, accessToken: response.accessToken)
        user = response.user
        phase = .ready
    }

    static var deviceName: String {
        UIDevice.current.name
    }

    // MARK: Session

    /// Called on launch / foreground: validates the token and refreshes user + server info.
    func refresh() async {
        guard let api else { return }
        do {
            user = try await api.me()
        } catch let error as APIError where error.isUnauthorized {
            await logout(remote: false)
            return
        } catch {
            // Offline: keep the session, views will show errors.
        }
        serverInfo = try? await api.serverInfo()
        if let prefs = try? await api.preferences() {
            preferences = prefs
        }
    }

    func updatePreferences(_ change: (inout UserPreferences) -> Void) async throws {
        guard let api else { throw APIError.notLoggedIn }
        var next = preferences
        change(&next)
        preferences = next
        do {
            preferences = try await api.updatePreferences(next)
        } catch {
            if let current = try? await api.preferences() { preferences = current }
            throw error
        }
    }

    func updateUser(_ user: UserDto) {
        self.user = user
    }

    func logout(remote: Bool = true) async {
        if remote, let api {
            try? await api.logout()
        }
        BackupService.shared.stop()
        KeychainStore.set(nil, for: .accessToken)
        KeychainStore.set(nil, for: .lockedFolderPin)
        api = nil
        user = nil
        preferences = UserPreferences()
        await ImageLoader.shared.clearAll()
        await UploadStateStore.shared.removeAll()
        phase = serverURL == nil ? .serverSetup : .login
        await refreshAuthStatus()
    }
}
