import SwiftUI

/// Partner sharing: libraries I share and libraries shared with me.
struct PartnersView: View {
    @Environment(AppSession.self) private var session
    @AppStorage("timeline.withPartners") private var withPartners = false
    @State private var sharedBy: [Partner] = []
    @State private var sharedWith: [Partner] = []
    @State private var showAdd = false
    @State private var isLoading = false
    @State private var errorMessage: String?

    var body: some View {
        List {
            Section {
                ForEach(sharedBy) { partner in
                    PartnerRow(user: partner.user)
                        .swipeActions {
                            Button(role: .destructive) {
                                Task { await remove(partner) }
                            } label: {
                                Label("partners.stop", systemImage: "person.badge.minus")
                            }
                        }
                }
                Button {
                    showAdd = true
                } label: {
                    Label("partners.add", systemImage: "person.badge.plus")
                }
            } header: {
                Text("partners.sharedBy")
            } footer: {
                Text("partners.sharedBy.footer")
            }

            Section {
                if sharedWith.isEmpty {
                    Text("partners.sharedWith.empty").foregroundStyle(.secondary)
                }
                ForEach(sharedWith) { partner in
                    Toggle(isOn: Binding(
                        get: { partner.inTimeline ?? false },
                        set: { newValue in Task { await setInTimeline(partner, newValue) } }
                    )) {
                        PartnerRow(user: partner.user)
                    }
                }
            } header: {
                Text("partners.sharedWith")
            } footer: {
                Text("partners.sharedWith.footer")
            }

            Section {
                Toggle("partners.showInTimeline", isOn: $withPartners)
            }
        }
        .overlay { if isLoading && sharedBy.isEmpty && sharedWith.isEmpty { ProgressView() } }
        .navigationTitle("library.partners")
        .sheet(isPresented: $showAdd) {
            UserPickerSheet(excluded: Set(sharedBy.map(\.id))) { user in
                Task { await add(user) }
            }
        }
        .task { await load() }
        .refreshable { await load() }
        .errorAlert($errorMessage)
    }

    private func load() async {
        guard let api = session.api else { return }
        isLoading = true
        defer { isLoading = false }
        do {
            async let by = api.partners(direction: .sharedBy)
            async let with = api.partners(direction: .sharedWith)
            sharedBy = try await by
            sharedWith = try await with
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func add(_ user: UserSummary) async {
        guard let api = session.api else { return }
        do {
            _ = try await api.addPartner(userId: user.id)
            await load()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func remove(_ partner: Partner) async {
        guard let api = session.api else { return }
        do {
            try await api.removePartner(userId: partner.user.id)
            sharedBy.removeAll { $0.id == partner.id }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func setInTimeline(_ partner: Partner, _ value: Bool) async {
        guard let api = session.api else { return }
        do {
            let updated = try await api.updatePartner(userId: partner.user.id, inTimeline: value)
            if let index = sharedWith.firstIndex(where: { $0.id == partner.id }) {
                sharedWith[index] = updated
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

private struct PartnerRow: View {
    let user: UserSummary

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(verbatim: user.name)
            if let email = user.email {
                Text(verbatim: email)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }
}

/// Lists all users of the server (`GET /api/users`).
struct UserPickerSheet: View {
    let excluded: Set<String>
    var onPick: (UserSummary) -> Void

    @Environment(AppSession.self) private var session
    @Environment(\.dismiss) private var dismiss
    @State private var users: [UserSummary] = []
    @State private var errorMessage: String?

    var body: some View {
        NavigationStack {
            List(users) { user in
                Button {
                    onPick(user)
                    dismiss()
                } label: {
                    PartnerRow(user: user)
                        .foregroundStyle(.primary)
                }
            }
            .navigationTitle("partners.add")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("common.cancel") { dismiss() }
                }
            }
            .task {
                guard let api = session.api else { return }
                do {
                    let me = session.user?.id
                    users = try await api.users().filter { $0.id != me && !excluded.contains($0.id) }
                } catch {
                    errorMessage = error.localizedDescription
                }
            }
            .errorAlert($errorMessage)
        }
    }
}
