import SwiftUI

/// Full-screen pager over a list of assets.
struct AssetViewer: View {
    let request: ViewerRequest
    let onChange: () -> Void

    @Environment(AppSession.self) private var session
    @Environment(\.dismiss) private var dismiss
    @State private var assets: [AssetDto]
    @State private var currentId: String?
    @State private var isZoomed = false
    @State private var chromeVisible = true
    @State private var showInfo = false
    @State private var showCast = false
    @State private var shareItem: ShareItem?
    @State private var albumPicker: AssetIdsRequest?
    @State private var sharedLinkRequest: AssetIdsRequest?
    @State private var confirmDelete = false
    @State private var isBusy = false
    @State private var errorMessage: String?
    @State private var toast: LocalizedStringKey?
    @State private var didChange = false

    init(request: ViewerRequest, onChange: @escaping () -> Void) {
        self.request = request
        self.onChange = onChange
        _assets = State(initialValue: request.assets)
        _currentId = State(initialValue: request.startId)
    }

    private var current: AssetDto? {
        assets.first { $0.id == currentId }
    }

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()

            ScrollViewReader { proxy in
                ScrollView(.horizontal) {
                    LazyHStack(spacing: 0) {
                        ForEach(assets) { asset in
                            AssetPage(asset: asset,
                                      isCurrent: asset.id == currentId,
                                      isZoomed: $isZoomed,
                                      onTap: { withAnimation(.easeInOut(duration: 0.2)) { chromeVisible.toggle() } })
                                .containerRelativeFrame([.horizontal, .vertical])
                                .id(asset.id)
                        }
                    }
                    .scrollTargetLayout()
                }
                .scrollTargetBehavior(.paging)
                .scrollPosition(id: $currentId)
                .scrollDisabled(isZoomed)
                .scrollIndicators(.hidden)
                .ignoresSafeArea()
                .onAppear {
                    proxy.scrollTo(request.startId)
                }
            }
            .simultaneousGesture(
                DragGesture(minimumDistance: 40)
                    .onEnded { value in
                        if !isZoomed, value.translation.height > 140,
                           abs(value.translation.width) < 80 {
                            dismiss()
                        }
                    }
            )

            if chromeVisible {
                VStack {
                    topBar
                    Spacer()
                    bottomBar
                }
                .transition(.opacity)
            }

            if let toast {
                Text(toast)
                    .font(.subheadline.weight(.medium))
                    .padding(.horizontal, 16)
                    .padding(.vertical, 10)
                    .background(.ultraThinMaterial, in: Capsule())
                    .transition(.opacity)
            }
        }
        .foregroundStyle(.white)
        .statusBarHidden(!chromeVisible)
        .persistentSystemOverlays(chromeVisible ? .automatic : .hidden)
        .onDisappear {
            if didChange { onChange() }
        }
        .sheet(isPresented: $showInfo) {
            if let current {
                AssetInfoSheet(assetId: current.id)
                    .presentationDetents([.medium, .large])
            }
        }
        .sheet(isPresented: $showCast) {
            if let current {
                CastSheet(asset: current)
            }
        }
        .sheet(item: $shareItem) { item in
            ActivityView(items: item.items)
        }
        .sheet(item: $albumPicker) { picker in
            AlbumPickerSheet(assetIds: picker.assetIds) {
                showToast("toast.addedToAlbum")
            }
        }
        .sheet(item: $sharedLinkRequest) { link in
            CreateSharedLinkSheet(type: .individual, assetIds: link.assetIds, albumId: nil)
        }
        .confirmationDialog("viewer.delete.title", isPresented: $confirmDelete, titleVisibility: .visible) {
            Button(role: .destructive) {
                Task { await delete() }
            } label: {
                Text(deleteTitle)
            }
        }
        .errorAlert($errorMessage)
    }

    private var deleteTitle: LocalizedStringKey {
        isTrashContext ? "action.deletePermanently" : "action.delete"
    }

    private var isTrashContext: Bool {
        if case .trash = request.context { return true }
        return current?.isTrashed ?? false
    }

    // MARK: Chrome

    private var topBar: some View {
        HStack(spacing: 16) {
            Button {
                dismiss()
            } label: {
                Image(systemName: "xmark")
                    .font(.title3.weight(.semibold))
                    .frame(width: 44, height: 44)
            }
            VStack(alignment: .leading, spacing: 2) {
                if let current {
                    Text(verbatim: TimelineGrouping.wallClockString(current.sortDate))
                        .font(.subheadline.weight(.semibold))
                    if let name = current.originalFileName {
                        Text(verbatim: name)
                            .font(.caption)
                            .opacity(0.7)
                            .lineLimit(1)
                    }
                }
            }
            Spacer()
            if current?.isVideo == true {
                AirPlayButton()
                    .frame(width: 36, height: 36)
            }
            if isBusy {
                ProgressView().tint(.white)
            }
            moreMenu
        }
        .padding(.horizontal, 8)
        .padding(.bottom, 8)
        .background(LinearGradient(colors: [.black.opacity(0.6), .clear], startPoint: .top, endPoint: .bottom)
            .ignoresSafeArea())
    }

    private var bottomBar: some View {
        HStack {
            if isTrashContext {
                barButton("action.restore", "arrow.uturn.backward") { Task { await perform(.restore) } }
                Spacer()
                barButton("action.deletePermanently", "trash.slash") { confirmDelete = true }
            } else {
                barButton("action.share", "square.and.arrow.up") { Task { await share() } }
                Spacer()
                barButton(current?.isFavorite == true ? "action.unfavorite" : "action.favorite",
                          current?.isFavorite == true ? "heart.fill" : "heart") {
                    Task { await toggleFavorite() }
                }
                Spacer()
                barButton("viewer.info", "info.circle") { showInfo = true }
                Spacer()
                barButton("action.delete", "trash") { confirmDelete = true }
            }
        }
        .padding(.horizontal, 28)
        .padding(.top, 12)
        .padding(.bottom, 4)
        .background(LinearGradient(colors: [.clear, .black.opacity(0.6)], startPoint: .top, endPoint: .bottom)
            .ignoresSafeArea())
    }

    private func barButton(_ title: LocalizedStringKey, _ systemImage: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.title2)
                .frame(width: 44, height: 44)
        }
        .accessibilityLabel(Text(title))
        .disabled(current == nil || isBusy)
    }

    private var moreMenu: some View {
        Menu {
            if let current, !isTrashContext {
                Button { Task { await saveToDevice() } } label: {
                    Label("viewer.saveToDevice", systemImage: "square.and.arrow.down")
                }
                Button { albumPicker = AssetIdsRequest(assetIds: [current.id]) } label: {
                    Label("action.addToAlbum", systemImage: "rectangle.stack.badge.plus")
                }
                Button { sharedLinkRequest = AssetIdsRequest(assetIds: [current.id]) } label: {
                    Label("action.sharedLink", systemImage: "link")
                }
                Button { showCast = true } label: {
                    Label("viewer.cast", systemImage: "tv")
                }
                Divider()
                if current.visibility == .archive {
                    Button { Task { await perform(.unarchive) } } label: {
                        Label("action.unarchive", systemImage: "archivebox.fill")
                    }
                } else if current.visibility != .locked {
                    Button { Task { await perform(.archive) } } label: {
                        Label("action.archive", systemImage: "archivebox")
                    }
                }
                if current.visibility == .locked {
                    Button { Task { await perform(.removeFromLocked) } } label: {
                        Label("action.removeFromLocked", systemImage: "lock.open")
                    }
                } else {
                    Button { Task { await perform(.moveToLocked) } } label: {
                        Label("action.moveToLocked", systemImage: "lock")
                    }
                }
            }
        } label: {
            Image(systemName: "ellipsis.circle")
                .font(.title3)
                .frame(width: 44, height: 44)
        }
    }

    // MARK: Actions

    private func showToast(_ message: LocalizedStringKey) {
        withAnimation { toast = message }
        Task {
            try? await Task.sleep(nanoseconds: 1_800_000_000)
            withAnimation { toast = nil }
        }
    }

    private func toggleFavorite() async {
        guard let current, let api = session.api else { return }
        do {
            let updated = try await api.updateAsset(id: current.id, UpdateAssetRequest(isFavorite: !current.isFavorite))
            replace(updated)
            didChange = true
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    /// Runs an action that removes the asset from the current list.
    private func perform(_ action: AssetAction) async {
        guard let current, let api = session.api else { return }
        isBusy = true
        defer { isBusy = false }
        do {
            try await AssetActionPerformer.perform(action, ids: [current.id], context: request.context, api: api)
            didChange = true
            removeCurrent()
            showToast("toast.done")
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func delete() async {
        await perform(isTrashContext ? .deletePermanently : .delete)
    }

    private func share() async {
        guard let current, let api = session.api else { return }
        isBusy = true
        defer { isBusy = false }
        do {
            let urls = try await AssetActionPerformer.downloadForSharing([current], api: api)
            shareItem = ShareItem(items: urls)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func saveToDevice() async {
        guard let current, let api = session.api else { return }
        isBusy = true
        defer { isBusy = false }
        do {
            let name = current.originalFileName ?? "\(current.id).\(current.isVideo ? "mp4" : "jpg")"
            let file = try await api.download(api.originalPath(assetId: current.id), fileName: name)
            var paired: URL?
            if let videoId = current.livePhotoVideoId {
                paired = try? await api.download(api.videoPath(assetId: videoId), fileName: "\(videoId).mov")
            }
            try await PhotoLibrarySaver.save(fileURL: file, isVideo: current.isVideo, pairedVideoURL: paired)
            showToast("toast.saved")
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func replace(_ asset: AssetDto) {
        if let index = assets.firstIndex(where: { $0.id == asset.id }) {
            assets[index] = asset
        }
    }

    private func removeCurrent() {
        guard let currentId, let index = assets.firstIndex(where: { $0.id == currentId }) else { return }
        assets.remove(at: index)
        if assets.isEmpty {
            dismiss()
            return
        }
        self.currentId = assets[min(index, assets.count - 1)].id
    }
}
