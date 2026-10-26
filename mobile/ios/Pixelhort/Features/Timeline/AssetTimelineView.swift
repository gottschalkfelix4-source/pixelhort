import SwiftUI

/// Identifies a viewer presentation (assets to page through + start asset).
struct ViewerRequest: Identifiable {
    let id = UUID()
    let assets: [AssetDto]
    let startId: String
    var context: AssetListContext = .timeline(withPartners: false)
}

/// Asset ids for sheets that act on a selection.
struct AssetIdsRequest: Identifiable {
    let id = UUID()
    let assetIds: [String]
}

/// Month/day grouped grid for any timeline filter, with multi-select actions.
struct AssetTimelineView<Header: View>: View {
    let context: AssetListContext
    private let header: Header

    @Environment(AppSession.self) private var session
    @AppStorage("grid.columns") private var columnCount = 4
    @State private var model: TimelineViewModel
    @State private var viewer: ViewerRequest?
    @State private var albumPicker: AssetIdsRequest?
    @State private var sharedLinkRequest: AssetIdsRequest?
    @State private var shareItem: ShareItem?
    @State private var pendingDestructive: AssetAction?
    @State private var isPerforming = false
    @State private var errorMessage: String?

    init(context: AssetListContext, @ViewBuilder header: () -> Header) {
        self.context = context
        self.header = header()
        _model = State(initialValue: TimelineViewModel(filter: context.filter))
    }

    private var columns: [GridItem] {
        Array(repeating: GridItem(.flexible(), spacing: 2), count: max(2, min(columnCount, 7)))
    }

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0, pinnedViews: [.sectionHeaders]) {
                header
                if model.hasLoaded && model.buckets.isEmpty {
                    StateView(kind: .empty("timeline.empty", "photo.on.rectangle.angled"))
                        .padding(.top, 80)
                }
                ForEach(model.buckets) { bucket in
                    Section {
                        bucketContent(bucket)
                    } header: {
                        Text(verbatim: TimelineGrouping.monthTitle(bucket.timeBucket))
                            .font(.headline)
                            .padding(.horizontal)
                            .padding(.vertical, 8)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(.bar)
                    }
                }
            }
            .padding(.bottom, model.isSelecting ? 80 : 0)
        }
        .overlay {
            if !model.hasLoaded && model.isLoading {
                ProgressView()
            } else if let message = model.errorMessage, model.buckets.isEmpty {
                StateView(kind: .error(message)) { Task { await reload() } }
            }
        }
        .refreshable { await reload() }
        .task {
            if !model.hasLoaded { await reload() }
        }
        .onChange(of: context) { _, newContext in
            model.filter = newContext.filter
            Task { await reload() }
        }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                if model.isSelecting {
                    Button("common.done") { model.endSelection() }
                } else if !model.buckets.isEmpty {
                    Button("common.select") { model.isSelecting = true }
                }
            }
        }
        .safeAreaInset(edge: .bottom) {
            if model.isSelecting {
                selectionBar
            }
        }
        .fullScreenCover(item: $viewer) { request in
            AssetViewer(request: request) {
                Task { await reload() }
            }
        }
        .sheet(item: $albumPicker) { request in
            AlbumPickerSheet(assetIds: request.assetIds) {
                model.endSelection()
            }
        }
        .sheet(item: $sharedLinkRequest) { request in
            CreateSharedLinkSheet(type: .individual, assetIds: request.assetIds, albumId: nil)
        }
        .sheet(item: $shareItem) { item in
            ActivityView(items: item.items)
        }
        .confirmationDialog("action.confirm.title", isPresented: Binding(
            get: { pendingDestructive != nil },
            set: { if !$0 { pendingDestructive = nil } }
        ), titleVisibility: .visible) {
            if let action = pendingDestructive {
                Button(role: .destructive) {
                    Task { await run(action) }
                } label: {
                    Text(action.title)
                }
            }
        } message: {
            Text("action.confirm.message \(model.selection.count)")
        }
        .errorAlert($errorMessage)
    }

    // MARK: Sections

    @ViewBuilder
    private func bucketContent(_ bucket: TimeBucket) -> some View {
        Group {
            if let days = model.days[bucket.id] {
                ForEach(days) { day in
                    dayHeader(day)
                    LazyVGrid(columns: columns, spacing: 2) {
                        ForEach(day.assets) { asset in
                            cell(asset)
                        }
                    }
                }
            } else {
                LazyVGrid(columns: columns, spacing: 2) {
                    ForEach(0..<max(bucket.count, 1), id: \.self) { _ in
                        Rectangle()
                            .fill(Color.secondary.opacity(0.12))
                            .aspectRatio(1, contentMode: .fit)
                    }
                }
            }
        }
        .task(id: "\(bucket.id)-\(model.generation)") {
            guard let api = session.api else { return }
            await model.loadBucket(bucket, api: api)
        }
    }

    private func dayHeader(_ day: DayGroup) -> some View {
        HStack {
            Text(verbatim: TimelineGrouping.dayTitle(day.date))
                .font(.subheadline.weight(.semibold))
            Spacer()
            if model.isSelecting {
                let allSelected = Set(day.assets.map(\.id)).isSubset(of: model.selection)
                Button {
                    model.toggleDaySelection(day)
                } label: {
                    Image(systemName: allSelected ? "checkmark.circle.fill" : "circle")
                }
            }
        }
        .padding(.horizontal)
        .padding(.top, 10)
        .padding(.bottom, 6)
    }

    private func cell(_ asset: AssetDto) -> some View {
        AssetGridCell(asset: asset,
                      isSelecting: model.isSelecting,
                      isSelected: model.selection.contains(asset.id))
            .onTapGesture {
                if model.isSelecting {
                    model.toggleSelection(asset.id)
                } else {
                    viewer = ViewerRequest(assets: model.loadedAssets, startId: asset.id, context: context)
                }
            }
            .onLongPressGesture(minimumDuration: 0.35) {
                model.beginSelection(with: asset.id)
            }
    }

    // MARK: Selection bar

    private var selectionBar: some View {
        VStack(spacing: 6) {
            Text("selection.count \(model.selection.count)")
                .font(.footnote)
                .foregroundStyle(.secondary)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 18) {
                    ForEach(context.selectionActions) { action in
                        Button(role: action.isDestructive ? .destructive : nil) {
                            trigger(action)
                        } label: {
                            VStack(spacing: 4) {
                                Image(systemName: action.systemImage)
                                    .font(.title3)
                                Text(action.title)
                                    .font(.caption2)
                                    .lineLimit(1)
                            }
                            .frame(minWidth: 56)
                        }
                        .disabled(model.selection.isEmpty || isPerforming)
                    }
                }
                .padding(.horizontal)
            }
        }
        .padding(.vertical, 10)
        .frame(maxWidth: .infinity)
        .background(.regularMaterial)
        .overlay(alignment: .top) {
            if isPerforming {
                ProgressView().offset(y: -24)
            }
        }
    }

    private func trigger(_ action: AssetAction) {
        let ids = Array(model.selection)
        switch action {
        case .addToAlbum:
            albumPicker = AssetIdsRequest(assetIds: ids)
        case .sharedLink:
            sharedLinkRequest = AssetIdsRequest(assetIds: ids)
        case .share:
            Task { await share() }
        case .delete, .deletePermanently, .removeFromAlbum:
            pendingDestructive = action
        default:
            Task { await run(action) }
        }
    }

    private func share() async {
        guard let api = session.api else { return }
        isPerforming = true
        defer { isPerforming = false }
        do {
            let urls = try await AssetActionPerformer.downloadForSharing(model.assets(for: model.selection), api: api)
            shareItem = ShareItem(items: urls)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func run(_ action: AssetAction) async {
        guard let api = session.api else { return }
        isPerforming = true
        defer { isPerforming = false }
        let ids = model.selection
        do {
            try await AssetActionPerformer.perform(action, ids: Array(ids), context: context, api: api)
            if action != .favorite && action != .unfavorite {
                model.removeLocally(ids)
            }
            model.endSelection()
            await reload()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func reload() async {
        guard let api = session.api else { return }
        await model.reload(api: api)
    }
}

extension AssetTimelineView where Header == EmptyView {
    init(context: AssetListContext) {
        self.init(context: context) { EmptyView() }
    }
}

/// A single grid cell with badges.
struct AssetGridCell: View {
    let asset: AssetDto
    var isSelecting = false
    var isSelected = false

    var body: some View {
        SquareThumbnail(assetId: asset.id)
            .overlay(alignment: .topTrailing) {
                if asset.isVideo, let duration = asset.duration {
                    Text(verbatim: TimelineGrouping.durationString(duration))
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(.white)
                        .shadow(radius: 2)
                        .padding(4)
                } else if asset.isVideo {
                    Image(systemName: "play.fill")
                        .font(.caption)
                        .foregroundStyle(.white)
                        .shadow(radius: 2)
                        .padding(4)
                } else if asset.isLivePhoto {
                    Image(systemName: "livephoto")
                        .font(.caption)
                        .foregroundStyle(.white)
                        .shadow(radius: 2)
                        .padding(4)
                }
            }
            .overlay(alignment: .bottomLeading) {
                if asset.isFavorite {
                    Image(systemName: "heart.fill")
                        .font(.caption)
                        .foregroundStyle(.white)
                        .shadow(radius: 2)
                        .padding(4)
                }
            }
            .overlay {
                if isSelected {
                    Rectangle().fill(Color.accentColor.opacity(0.25))
                }
            }
            .overlay(alignment: .topLeading) {
                if isSelecting {
                    Image(systemName: isSelected ? "checkmark.circle.fill" : "circle")
                        .font(.title3)
                        .symbolRenderingMode(.palette)
                        .foregroundStyle(.white, isSelected ? Color.accentColor : Color.black.opacity(0.3))
                        .padding(4)
                }
            }
            .scaleEffect(isSelected ? 0.94 : 1)
            .animation(.easeOut(duration: 0.12), value: isSelected)
    }
}

/// Simple flat grid (search results, memories, shared albums).
struct FlatAssetGrid: View {
    let assets: [AssetDto]
    var context: AssetListContext = .timeline(withPartners: false)
    var onReachEnd: (() -> Void)?
    var onChange: (() -> Void)?

    @AppStorage("grid.columns") private var columnCount = 4
    @State private var viewer: ViewerRequest?

    private var columns: [GridItem] {
        Array(repeating: GridItem(.flexible(), spacing: 2), count: max(2, min(columnCount, 7)))
    }

    var body: some View {
        LazyVGrid(columns: columns, spacing: 2) {
            ForEach(assets) { asset in
                AssetGridCell(asset: asset)
                    .onTapGesture {
                        viewer = ViewerRequest(assets: assets, startId: asset.id, context: context)
                    }
                    .onAppear {
                        if asset.id == assets.last?.id { onReachEnd?() }
                    }
            }
        }
        .fullScreenCover(item: $viewer) { request in
            AssetViewer(request: request) { onChange?() }
        }
    }
}
