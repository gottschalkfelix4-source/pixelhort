import SwiftUI

struct ServerSetupView: View {
    @Environment(AppSession.self) private var session
    @State private var urlText = ""
    @State private var isChecking = false
    @State private var errorMessage: String?

    var body: some View {
        Form {
            Section {
                VStack(spacing: 12) {
                    Image(systemName: "photo.stack.fill")
                        .font(.system(size: 56))
                        .foregroundStyle(.tint)
                    Text("onboarding.welcome")
                        .font(.title.bold())
                    Text("onboarding.welcome.subtitle")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 12)
            }
            .listRowBackground(Color.clear)

            Section {
                TextField("onboarding.server.placeholder", text: $urlText)
                    .keyboardType(.URL)
                    .textContentType(.URL)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .submitLabel(.continue)
                    .onSubmit { Task { await check() } }
                    .accessibilityIdentifier("server.url")
            } header: {
                Text("onboarding.server.title")
            } footer: {
                Text("onboarding.server.footer")
            }

            Section {
                Button {
                    Task { await check() }
                } label: {
                    HStack {
                        Spacer()
                        if isChecking {
                            ProgressView()
                        } else {
                            Text("onboarding.server.connect").bold()
                        }
                        Spacer()
                    }
                }
                .disabled(urlText.trimmingCharacters(in: .whitespaces).isEmpty || isChecking)
                .accessibilityIdentifier("server.connect")
            }
        }
        .navigationTitle("Pixelhort")
        .errorAlert($errorMessage)
    }

    private func check() async {
        guard let url = APIClient.normalizeServerURL(urlText) else {
            errorMessage = APIError.invalidServerURL.localizedDescription
            return
        }
        isChecking = true
        defer { isChecking = false }
        let client = APIClient(baseURL: url)
        do {
            guard try await client.ping() else {
                errorMessage = NSLocalizedString("onboarding.server.notPixelhort", comment: "")
                return
            }
            let status = try await client.authStatus()
            session.configureServer(url: url, status: status)
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}
