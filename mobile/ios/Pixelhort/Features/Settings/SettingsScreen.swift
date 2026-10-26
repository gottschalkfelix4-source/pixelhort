import SwiftUI

struct SettingsScreen: View {
    @Environment(AppSession.self) private var session
    @Environment(BackupService.self) private var backup
    @AppStorage("grid.columns") private var columnCount = 4
    @AppStorage("timeline.withPartners") private var withPartners = false
    @State private var cacheSize: Int64 = 0
    @State private var confirmLogout = false
    @State private var showEditName = false
    @State private var newName = ""
    @State private var errorMessage: String?

    var body: some View {
        Form {
            Section("settings.account") {
                if let user = session.user {
                    LabeledContent("settings.name") { Text(verbatim: user.name) }
                    LabeledContent("settings.email") { Text(verbatim: user.email) }
                    if let usage = user.usageBytes {
                        VStack(alignment: .leading, spacing: 6) {
                            if let quota = user.quotaBytes, quota > 0 {
                                ProgressView(value: min(Double(usage) / Double(quota), 1))
                                Text("settings.storage.quota \(format(usage)) \(format(quota))")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            } else {
                                Text("settings.storage.used \(format(usage))")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                    Button("settings.editName") {
                        newName = user.name
                        showEditName = true
                    }
                } else {
                    ProgressView()
                }
            }

            Section("settings.backup") {
                NavigationLink {
                    BackupSettingsView()
                } label: {
                    LabeledContent("settings.backup.options") {
                        Text(backup.settings.isEnabled ? LocalizedStringKey("common.on") : LocalizedStringKey("common.off"))
                    }
                }
                .accessibilityIdentifier("settings.backupOptions")
                NavigationLink {
                    BackupProgressView()
                } label: {
                    Label("backup.title", systemImage: "icloud.and.arrow.up")
                }
                .accessibilityIdentifier("settings.backupProgress")
            }

            Section("settings.display") {
                Stepper(value: $columnCount, in: 2...7) {
                    LabeledContent("settings.columns") { Text(verbatim: "\(columnCount)") }
                }
                Toggle("partners.showInTimeline", isOn: $withPartners)
            }

            Section("settings.server") {
                LabeledContent("settings.serverURL") {
                    Text(verbatim: session.serverURL?.absoluteString ?? "–")
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
                if let version = session.serverInfo?.version {
                    LabeledContent("settings.serverVersion") { Text(verbatim: version) }
                }
                LabeledContent("settings.appVersion") {
                    Text(verbatim: Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "–")
                }
                LabeledContent("settings.deviceId") {
                    Text(verbatim: String(session.deviceId.prefix(8)))
                        .font(.caption.monospaced())
                }
            }

            Section("settings.cache") {
                LabeledContent("settings.cache.size") { Text(verbatim: format(cacheSize)) }
                Button("settings.cache.clear") {
                    Task {
                        await ImageLoader.shared.clearAll()
                        cacheSize = await ImageLoader.shared.diskUsageBytes()
                    }
                }
            }

            Section {
                Button("settings.logout", role: .destructive) { confirmLogout = true }
            }
        }
        .navigationTitle("tab.settings")
        .task {
            await session.refresh()
            cacheSize = await ImageLoader.shared.diskUsageBytes()
        }
        .confirmationDialog("settings.logout.confirm", isPresented: $confirmLogout, titleVisibility: .visible) {
            Button("settings.logout", role: .destructive) {
                Task { await session.logout() }
            }
        }
        .alert("settings.editName", isPresented: $showEditName) {
            TextField("settings.name", text: $newName)
            Button("common.save") { Task { await saveName() } }
            Button("common.cancel", role: .cancel) {}
        }
        .errorAlert($errorMessage)
    }

    private func format(_ bytes: Int64) -> String {
        ByteCountFormatter.string(fromByteCount: bytes, countStyle: .file)
    }

    private func saveName() async {
        guard let api = session.api else { return }
        do {
            let user = try await api.updateMe(UpdateMeRequest(name: newName))
            session.updateUser(user)
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}
