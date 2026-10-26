import SwiftUI

/// Albums tab: own albums and albums shared with me.
struct AlbumsScreen: View {
    enum Scope: String, CaseIterable, Identifiable {
        case mine
        case shared
        var id: String { rawValue }
        var title: LocalizedStringKey { self == .mine ? "albums.mine" : "albums.shared" }
    }

    @Environment(AppSession.self) private var session
    @State private var scope: Scope = .mine
    @State private var albums: [AlbumDto] = []
    @State private var isLoading = false
    @State private var errorMessage: String?
    @State private var showCreate = false

    private let columns = [GridItem(.adaptive(minimum: 150), spacing: 14)]

    var body: some View {
        ScrollView {
            Picker("albums.scope", selection: $scope) {
                ForEach(Scope.allCases) { scope in
                    Text(scope.title).tag(scope)
                }
            }
            .pickerStyle(.segmented)
            .padding(.horizontal)

            if !isLoading && albums.isEmpty && errorMessage == nil {
                StateView(kind: .empty(scope == .mine ? "albums.empty" : "albums.shared.empty", "rectangle.stack"))
                    .padding(.top, 60)
            }

            LazyVGrid(columns: columns, spacing: 18) {
                ForEach(albums) { album in
                    NavigationLink(value: album) {
                        AlbumCard(album: album)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding()
        }
        .overlay {
            if isLoading && albums.isEmpty { ProgressView() }
        }
        .navigationTitle("tab.albums")
        .navigationDestination(for: AlbumDto.self) { album in
            AlbumDetailView(album: album)
        }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    showCreate = true
                } label: {
                    Image(systemName: "plus")
                }
                .accessibilityLabel(Text("albums.create"))
            }
        }
        .sheet(isPresented: $showCreate) {
            CreateAlbumSheet(assetIds: []) { _ in
                Task { await load() }
            }
        }
        .refreshable { await load() }
        .task(id: scope) { await load() }
        .errorAlert($errorMessage)
    }

    private func load() async {
        guard let api = session.api else { return }
        isLoading = true
        defer { isLoading = false }
        do {
            let all = try await api.albums(shared: scope == .shared ? true : nil)
            let myId = session.user?.id
            albums = scope == .mine ? all.filter { myId == nil || $0.ownerId == myId } : all
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

struct AlbumCard: View {
    let album: AlbumDto

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Color.clear
                .aspectRatio(1, contentMode: .fit)
                .overlay {
                    if let cover = album.albumThumbnailAssetId {
                        AssetThumbnail(assetId: cover, size: .preview)
                    } else {
                        ZStack {
                            Rectangle().fill(Color.secondary.opacity(0.15))
                            Image(systemName: "photo.on.rectangle")
                                .font(.largeTitle)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
                .clipShape(RoundedRectangle(cornerRadius: 12))
            Text(verbatim: album.albumName)
                .font(.subheadline.weight(.semibold))
                .lineLimit(1)
            HStack(spacing: 4) {
                Text("albums.count \(album.assetCount ?? 0)")
                if album.shared == true {
                    Image(systemName: "person.2.fill")
                }
            }
            .font(.caption)
            .foregroundStyle(.secondary)
        }
    }
}
