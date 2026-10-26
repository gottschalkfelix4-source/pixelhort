import SwiftUI

/// Devices and browsers signed in to the account, with sign-out per device.
struct DevicesView: View {
    @Environment(AppSession.self) private var session
    @State private var sessions: [AuthSessionDto] = []
    @State private var loading = true
    @State private var confirmOthers = false
    @State private var errorMessage: String?

    var body: some View {
        List {
            Section {
                ForEach(sessions) { item in
                    HStack(spacing: 14) {
                        Image(systemName: icon(for: item))
                            .font(.title3)
                            .foregroundStyle(.secondary)
                            .frame(width: 28)
                        VStack(alignment: .leading, spacing: 2) {
                            HStack {
                                Text(verbatim: label(for: item))
                                if item.current {
                                    Text("devices.current")
                                        .font(.caption.bold())
                                        .padding(.horizontal, 6)
                                        .padding(.vertical, 2)
                                        .background(Color.accentColor.opacity(0.2), in: Capsule())
                                }
                            }
                            Text("devices.lastUsed \(item.lastUsedAt.formatted(date: .abbreviated, time: .shortened))")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                    .swipeActions {
                        if !item.current {
                            Button("devices.signOut", role: .destructive) {
                                Task { await remove(item) }
                            }
                        }
                    }
                }
            } footer: {
                Text("devices.footer")
            }

            if sessions.contains(where: { !$0.current }) {
                Section {
                    Button("devices.signOutOthers", role: .destructive) { confirmOthers = true }
                }
            }
        }
        .overlay {
            if loading && sessions.isEmpty { ProgressView() }
        }
        .navigationTitle("settings.devices")
        .refreshable { await load() }
        .task { await load() }
        .confirmationDialog("devices.signOutOthers.confirm", isPresented: $confirmOthers, titleVisibility: .visible) {
            Button("devices.signOutOthers", role: .destructive) { Task { await removeOthers() } }
        }
        .errorAlert($errorMessage)
    }

    private func icon(for item: AuthSessionDto) -> String {
        switch item.deviceType?.lowercased() {
        case "ios", "android", "mobile": return "iphone"
        default: return "laptopcomputer"
        }
    }

    private func label(for item: AuthSessionDto) -> String {
        let type: String? = switch item.deviceType?.lowercased() {
        case "ios": "iOS"
        case "android": "Android"
        case "web": "Web"
        case "mobile": "Mobile"
        default: item.deviceType
        }
        var name = item.deviceName
        if let agent = name, agent.hasPrefix("Mozilla/") {
            name = browserName(agent)
        }
        let parts = [type, name].compactMap { $0 }.filter { !$0.isEmpty }
        return parts.isEmpty ? String(localized: "devices.unknown") : parts.joined(separator: " · ")
    }

    private func browserName(_ agent: String) -> String? {
        let browser = ["Edg/": "Edge", "Firefox/": "Firefox", "Chrome/": "Chrome", "Safari/": "Safari"]
            .first { agent.contains($0.key) }?.value
        let os = ["iPhone": "iOS", "iPad": "iPadOS", "Android": "Android", "Mac OS X": "macOS", "Windows": "Windows", "Linux": "Linux"]
            .first { agent.contains($0.key) }?.value
        let parts = [browser, os].compactMap { $0 }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }

    private func load() async {
        guard let api = session.api else { return }
        defer { loading = false }
        do {
            sessions = try await api.sessions()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func remove(_ item: AuthSessionDto) async {
        guard let api = session.api else { return }
        do {
            try await api.deleteSession(id: item.id)
            sessions.removeAll { $0.id == item.id }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func removeOthers() async {
        guard let api = session.api else { return }
        do {
            try await api.deleteOtherSessions()
            await load()
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

/// Changes the account password. The server signs out all other devices afterwards.
struct ChangePasswordView: View {
    @Environment(AppSession.self) private var session
    @Environment(\.dismiss) private var dismiss
    @State private var current = ""
    @State private var new = ""
    @State private var repeated = ""
    @State private var busy = false
    @State private var errorMessage: String?

    private var valid: Bool { new.count >= 8 && new == repeated }

    var body: some View {
        Form {
            Section {
                SecureField("password.current", text: $current)
                    .textContentType(.password)
                SecureField("password.new", text: $new)
                    .textContentType(.newPassword)
                SecureField("password.repeat", text: $repeated)
                    .textContentType(.newPassword)
            } footer: {
                if !repeated.isEmpty && new != repeated {
                    Text("password.mismatch").foregroundStyle(.red)
                } else {
                    Text("password.footer")
                }
            }
            Section {
                Button {
                    Task { await save() }
                } label: {
                    if busy { ProgressView() } else { Text("common.save") }
                }
                .disabled(!valid || busy)
            }
        }
        .navigationTitle("settings.changePassword")
        .errorAlert($errorMessage)
    }

    private func save() async {
        guard let api = session.api else { return }
        busy = true
        defer { busy = false }
        do {
            let user = try await api.updateMe(UpdateMeRequest(password: new, currentPassword: current))
            session.updateUser(user)
            dismiss()
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}
