import SwiftUI

/// Create a new album, optionally pre-filled with assets.
struct CreateAlbumSheet: View {
    let assetIds: [String]
    var onCreated: (AlbumDto) -> Void

    @Environment(AppSession.self) private var session
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var description = ""
    @State private var isSaving = false
    @State private var errorMessage: String?

    var body: some View {
        NavigationStack {
            Form {
                TextField("album.name", text: $name)
                TextField("album.description", text: $description, axis: .vertical)
                    .lineLimit(2...5)
                if !assetIds.isEmpty {
                    Text("album.create.withAssets \(assetIds.count)")
                        .foregroundStyle(.secondary)
                }
            }
            .navigationTitle("albums.create")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("common.cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    if isSaving {
                        ProgressView()
                    } else {
                        Button("common.create") { Task { await create() } }
                            .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                    }
                }
            }
            .errorAlert($errorMessage)
        }
    }

    private func create() async {
        guard let api = session.api else { return }
        isSaving = true
        defer { isSaving = false }
        do {
            let album = try await api.createAlbum(CreateAlbumRequest(
                albumName: name.trimmingCharacters(in: .whitespaces),
                description: description.isEmpty ? nil : description,
                assetIds: assetIds.isEmpty ? nil : assetIds))
            onCreated(album)
            dismiss()
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

/// Pick an album to add assets to (or create a new one).
struct AlbumPickerSheet: View {
    let assetIds: [String]
    var onDone: () -> Void

    @Environment(AppSession.self) private var session
    @Environment(\.dismiss) private var dismiss
    @State private var albums: [AlbumDto] = []
    @State private var isLoading = true
    @State private var showCreate = false
    @State private var errorMessage: String?

    var body: some View {
        NavigationStack {
            List {
                Button {
                    showCreate = true
                } label: {
                    Label("albums.create", systemImage: "plus.rectangle.on.rectangle")
                }
                ForEach(albums) { album in
                    Button {
                        Task { await add(to: album) }
                    } label: {
                        HStack(spacing: 12) {
                            Group {
                                if let cover = album.albumThumbnailAssetId {
                                    AssetThumbnail(assetId: cover)
                                } else {
                                    Rectangle().fill(Color.secondary.opacity(0.15))
                                }
                            }
                            .frame(width: 48, height: 48)
                            .clipShape(RoundedRectangle(cornerRadius: 8))
                            VStack(alignment: .leading) {
                                Text(verbatim: album.albumName)
                                    .foregroundStyle(.primary)
                                Text("albums.count \(album.assetCount ?? 0)")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            }
            .overlay { if isLoading { ProgressView() } }
            .navigationTitle("action.addToAlbum")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("common.cancel") { dismiss() }
                }
            }
            .sheet(isPresented: $showCreate) {
                CreateAlbumSheet(assetIds: assetIds) { _ in
                    onDone()
                    dismiss()
                }
            }
            .task { await load() }
            .errorAlert($errorMessage)
        }
    }

    private func load() async {
        guard let api = session.api else { return }
        defer { isLoading = false }
        do {
            let me = session.user?.id
            albums = try await api.albums().filter { album in
                album.ownerId == me || (album.albumUsers?.contains { $0.user.id == me && $0.role == .editor } ?? false)
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func add(to album: AlbumDto) async {
        guard let api = session.api else { return }
        do {
            try await api.addAssets(toAlbum: album.id, assetIds: assetIds)
            onDone()
            dismiss()
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

/// Share an album with other users (`PUT /api/albums/{id}/users`).
struct AlbumUsersSheet: View {
    let album: AlbumDto
    var onUpdated: (AlbumDto) -> Void

    @Environment(AppSession.self) private var session
    @Environment(\.dismiss) private var dismiss
    @State private var users: [UserSummary] = []
    @State private var roles: [String: AlbumRole] = [:]
    @State private var isSaving = false
    @State private var errorMessage: String?

    var body: some View {
        NavigationStack {
            List {
                Section {
                    ForEach(users) { user in
                        HStack {
                            VStack(alignment: .leading) {
                                Text(verbatim: user.name)
                                if let email = user.email {
                                    Text(verbatim: email).font(.caption).foregroundStyle(.secondary)
                                }
                            }
                            Spacer()
                            Menu {
                                Button("album.role.none") { roles[user.id] = nil }
                                Button("album.role.viewer") { roles[user.id] = .viewer }
                                Button("album.role.editor") { roles[user.id] = .editor }
                            } label: {
                                Text(roleTitle(roles[user.id]))
                                    .font(.subheadline)
                            }
                        }
                    }
                } footer: {
                    Text("album.share.footer")
                }
            }
            .navigationTitle("album.share")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("common.cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    if isSaving {
                        ProgressView()
                    } else {
                        Button("common.save") { Task { await save() } }
                    }
                }
            }
            .task { await load() }
            .errorAlert($errorMessage)
        }
    }

    private func roleTitle(_ role: AlbumRole?) -> LocalizedStringKey {
        switch role {
        case nil: return "album.role.none"
        case .some(.viewer): return "album.role.viewer"
        case .some(.editor): return "album.role.editor"
        }
    }

    private func load() async {
        guard let api = session.api else { return }
        for member in album.albumUsers ?? [] {
            roles[member.user.id] = member.role ?? .viewer
        }
        do {
            let me = session.user?.id
            users = try await api.users().filter { $0.id != me && $0.id != album.ownerId }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func save() async {
        guard let api = session.api else { return }
        isSaving = true
        defer { isSaving = false }
        do {
            // Remove users that were deselected.
            let previous = Set((album.albumUsers ?? []).map(\.user.id))
            for userId in previous where roles[userId] == nil {
                try await api.removeAlbumUser(albumId: album.id, userId: userId)
            }
            let requests = roles.map { AlbumUserRequest(userId: $0.key, role: $0.value) }
            var updated = album
            if !requests.isEmpty {
                updated = try await api.setAlbumUsers(id: album.id, users: requests)
            } else {
                updated = try await api.album(id: album.id)
            }
            onUpdated(updated)
            dismiss()
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}
