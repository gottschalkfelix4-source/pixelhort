import SwiftUI

struct AlbumDetailView: View {
    @Environment(AppSession.self) private var session
    @Environment(\.dismiss) private var dismiss
    @State private var album: AlbumDto
    @State private var showRename = false
    @State private var newName = ""
    @State private var showUsers = false
    @State private var showSharedLink = false
    @State private var confirmDelete = false
    @State private var errorMessage: String?

    init(album: AlbumDto) {
        _album = State(initialValue: album)
    }

    private var isOwner: Bool {
        album.ownerId == nil || album.ownerId == session.user?.id
    }

    private var canEdit: Bool {
        if isOwner { return true }
        let me = session.user?.id
        return album.albumUsers?.contains { $0.user.id == me && $0.role == .editor } ?? false
    }

    var body: some View {
        AssetTimelineView(context: .album(id: album.id, canEdit: canEdit)) {
            header
        }
        .navigationTitle(Text(verbatim: album.albumName))
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    if isOwner {
                        Button {
                            newName = album.albumName
                            showRename = true
                        } label: { Label("album.rename", systemImage: "pencil") }
                        Button { showUsers = true } label: { Label("album.share", systemImage: "person.badge.plus") }
                    }
                    Button { showSharedLink = true } label: { Label("action.sharedLink", systemImage: "link") }
                    Divider()
                    if isOwner {
                        Button(role: .destructive) { confirmDelete = true } label: {
                            Label("album.delete", systemImage: "trash")
                        }
                    } else {
                        Button(role: .destructive) { Task { await leave() } } label: {
                            Label("album.leave", systemImage: "rectangle.portrait.and.arrow.right")
                        }
                    }
                } label: {
                    Image(systemName: "ellipsis.circle")
                }
            }
        }
        .alert("album.rename", isPresented: $showRename) {
            TextField("album.name", text: $newName)
            Button("common.save") { Task { await rename() } }
            Button("common.cancel", role: .cancel) {}
        }
        .confirmationDialog("album.delete.confirm", isPresented: $confirmDelete, titleVisibility: .visible) {
            Button("album.delete", role: .destructive) { Task { await delete() } }
        }
        .sheet(isPresented: $showUsers) {
            AlbumUsersSheet(album: album) { updated in album = updated }
        }
        .sheet(isPresented: $showSharedLink) {
            CreateSharedLinkSheet(type: .album, assetIds: [], albumId: album.id)
        }
        .task { await refresh() }
        .errorAlert($errorMessage)
    }

    @ViewBuilder
    private var header: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(verbatim: album.albumName)
                .font(.largeTitle.bold())
            if let description = album.description, !description.isEmpty {
                Text(verbatim: description)
                    .foregroundStyle(.secondary)
            }
            HStack(spacing: 8) {
                Text("albums.count \(album.assetCount ?? 0)")
                if let owner = album.owner, !isOwner {
                    Text("album.owner \(owner.name)")
                }
            }
            .font(.footnote)
            .foregroundStyle(.secondary)
            if let users = album.albumUsers, !users.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack {
                        ForEach(users, id: \.user.id) { member in
                            Label {
                                Text(verbatim: member.user.name)
                            } icon: {
                                Image(systemName: member.role == .editor ? "pencil.circle" : "eye.circle")
                            }
                            .font(.caption)
                            .padding(.horizontal, 10)
                            .padding(.vertical, 5)
                            .background(Color.secondary.opacity(0.12), in: Capsule())
                        }
                    }
                }
            }
        }
        .padding(.horizontal)
        .padding(.bottom, 8)
    }

    private func refresh() async {
        guard let api = session.api else { return }
        if let loaded = try? await api.album(id: album.id) {
            album = loaded
        }
    }

    private func rename() async {
        guard let api = session.api, !newName.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        do {
            album = try await api.updateAlbum(id: album.id, UpdateAlbumRequest(albumName: newName))
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func delete() async {
        guard let api = session.api else { return }
        do {
            try await api.deleteAlbum(id: album.id)
            dismiss()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func leave() async {
        guard let api = session.api else { return }
        do {
            try await api.removeAlbumUser(albumId: album.id, userId: "me")
            dismiss()
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}
