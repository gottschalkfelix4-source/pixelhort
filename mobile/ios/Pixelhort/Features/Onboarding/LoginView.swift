import AuthenticationServices
import SwiftUI

struct LoginView: View {
    @Environment(AppSession.self) private var session
    @State private var email = ""
    @State private var password = ""
    @State private var isWorking = false
    @State private var errorMessage: String?
    @State private var oauth = OAuthSession()

    private var status: AuthStatus? { session.authStatus }

    /// UI tests disable password autofill so iOS does not show its "Save Password?" sheet.
    private static let autofill = !ProcessInfo.processInfo.arguments.contains("-uiTesting")

    var body: some View {
        Form {
            Section {
                LabeledContent("login.server") {
                    Text(verbatim: session.serverURL?.absoluteString ?? "–")
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }

            if status?.setupRequired == true {
                Section {
                    Label("login.setupRequired", systemImage: "info.circle")
                        .foregroundStyle(.secondary)
                }
            }

            if status?.passwordLoginEnabled ?? true {
                Section("login.title") {
                    TextField("login.email", text: $email)
                        .keyboardType(.emailAddress)
                        .textContentType(Self.autofill ? .username : nil)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .accessibilityIdentifier("login.email")
                    SecureField("login.password", text: $password)
                        .textContentType(Self.autofill ? .password : nil)
                        .submitLabel(.go)
                        .onSubmit { Task { await login() } }
                        .accessibilityIdentifier("login.password")
                    Button {
                        Task { await login() }
                    } label: {
                        HStack {
                            Spacer()
                            if isWorking { ProgressView() } else { Text("login.submit").bold() }
                            Spacer()
                        }
                    }
                    .disabled(email.isEmpty || password.isEmpty || isWorking)
                    .accessibilityIdentifier("login.submit")
                }
            }

            if status?.oauthEnabled == true {
                Section {
                    Button {
                        Task { await loginWithOAuth() }
                    } label: {
                        HStack {
                            Spacer()
                            Label {
                                if let text = status?.oauthButtonText, !text.isEmpty {
                                    Text(verbatim: text)
                                } else {
                                    Text("login.oauth")
                                }
                            } icon: {
                                Image(systemName: "person.badge.key")
                            }
                            Spacer()
                        }
                    }
                    .disabled(isWorking)
                }
            }

            Section {
                Button("login.changeServer", role: .destructive) {
                    session.changeServer()
                }
            }
        }
        .navigationTitle("login.navigationTitle")
        .task {
            if session.authStatus == nil {
                await session.refreshAuthStatus()
            }
        }
        .errorAlert($errorMessage)
    }

    private func login() async {
        guard let client = session.anonymousClient else { return }
        isWorking = true
        defer { isWorking = false }
        do {
            let response = try await client.login(email: email.trimmingCharacters(in: .whitespaces),
                                                  password: password,
                                                  deviceName: AppSession.deviceName)
            session.completeLogin(response)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func loginWithOAuth() async {
        guard let client = session.anonymousClient else { return }
        isWorking = true
        defer { isWorking = false }
        do {
            let authorizeURL = try await client.oauthAuthorizeURL(redirectUri: AppSession.oauthRedirectURI)
            let callbackURL = try await oauth.start(url: authorizeURL, callbackScheme: AppSession.oauthCallbackScheme)
            let response = try await client.oauthCallback(url: callbackURL)
            session.completeLogin(response)
        } catch {
            if let authError = error as? ASWebAuthenticationSessionError, authError.code == .canceledLogin {
                return // user cancelled
            }
            errorMessage = error.localizedDescription
        }
    }
}
