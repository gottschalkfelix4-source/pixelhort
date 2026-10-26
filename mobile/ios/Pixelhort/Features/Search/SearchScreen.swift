import SwiftUI

/// Smart search (CLIP) + explore (people, places, things).
struct SearchScreen: View {
    @Environment(AppSession.self) private var session
    @State private var query = ""
    @State private var submittedQuery = ""
    @State private var results: [AssetDto] = []
    @State private var nextPage: Int?
    @State private var isSearching = false
    @State private var explore: ExploreResponse?
    @State private var errorMessage: String?

    var body: some View {
        ScrollView {
            if submittedQuery.isEmpty {
                exploreContent
            } else {
                resultsContent
            }
        }
        .navigationTitle("tab.search")
        .searchable(text: $query, prompt: Text("search.prompt"))
        .onSubmit(of: .search) {
            Task { await search(reset: true) }
        }
        .onChange(of: query) { _, newValue in
            if newValue.isEmpty {
                submittedQuery = ""
                results = []
            }
        }
        .task {
            guard explore == nil, let api = session.api else { return }
            explore = try? await api.explore()
        }
        .navigationDestination(for: ExploreDestination.self) { destination in
            MetadataResultsView(destination: destination)
        }
        .errorAlert($errorMessage)
    }

    // MARK: Results

    @ViewBuilder
    private var resultsContent: some View {
        if isSearching && results.isEmpty {
            ProgressView().padding(.top, 80)
        } else if results.isEmpty {
            ContentUnavailableView.search(text: submittedQuery)
        } else {
            FlatAssetGrid(assets: results, onReachEnd: {
                Task { await search(reset: false) }
            })
            if isSearching {
                ProgressView().padding()
            }
        }
    }

    private func search(reset: Bool) async {
        guard let api = session.api else { return }
        let text = query.trimmingCharacters(in: .whitespaces)
        guard !text.isEmpty else { return }
        if reset {
            submittedQuery = text
            results = []
            nextPage = 1
        }
        guard let page = nextPage, !isSearching else { return }
        isSearching = true
        defer { isSearching = false }
        do {
            // Without the ML service, fall back to searching file names (same as the web app).
            let smart = session.serverInfo?.features?.smartSearch ?? true
            let response: SearchResponse
            if smart {
                do {
                    response = try await api.smartSearch(query: text, page: page)
                } catch {
                    response = try await api.metadataSearch(MetadataSearchRequest(originalFileName: text, page: page, size: 100))
                }
            } else {
                response = try await api.metadataSearch(MetadataSearchRequest(originalFileName: text, page: page, size: 100))
            }
            results.append(contentsOf: response.assets)
            nextPage = response.nextPage
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    // MARK: Explore

    @ViewBuilder
    private var exploreContent: some View {
        VStack(alignment: .leading, spacing: 24) {
            if let explore {
                if !explore.people.isEmpty {
                    sectionHeader("explore.people") {
                        NavigationLink("common.showAll") { PeopleView() }
                    }
                    ScrollView(.horizontal, showsIndicators: false) {
                        LazyHStack(spacing: 16) {
                            ForEach(explore.people) { person in
                                NavigationLink {
                                    PersonDetailView(person: person)
                                } label: {
                                    VStack(spacing: 6) {
                                        PersonAvatar(personId: person.id, diameter: 76)
                                        Text(verbatim: person.displayName ?? "")
                                            .font(.caption)
                                            .lineLimit(1)
                                    }
                                    .frame(width: 84)
                                }
                                .buttonStyle(.plain)
                            }
                        }
                        .padding(.horizontal)
                    }
                }

                if !explore.places.isEmpty {
                    sectionHeader("explore.places") {
                        NavigationLink("explore.map") { MapScreen() }
                    }
                    exploreGrid(explore.places) { ExploreDestination(kind: .place, value: $0.value) }
                }

                if !explore.objects.isEmpty {
                    sectionHeader("explore.things") { EmptyView() }
                    exploreGrid(explore.objects) { ExploreDestination(kind: .object, value: $0.value) }
                }
            } else {
                ProgressView()
                    .frame(maxWidth: .infinity)
                    .padding(.top, 80)
            }
        }
        .padding(.vertical)
    }

    private func sectionHeader<Trailing: View>(_ title: LocalizedStringKey,
                                               @ViewBuilder trailing: () -> Trailing) -> some View {
        HStack {
            Text(title).font(.title3.bold())
            Spacer()
            trailing()
        }
        .padding(.horizontal)
    }

    private func exploreGrid(_ items: [ExploreItem],
                             destination: @escaping (ExploreItem) -> ExploreDestination) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            LazyHStack(spacing: 10) {
                ForEach(items) { item in
                    NavigationLink(value: destination(item)) {
                        ZStack(alignment: .bottomLeading) {
                            if let assetId = item.assetId {
                                AssetThumbnail(assetId: assetId)
                            } else {
                                Rectangle().fill(Color.secondary.opacity(0.15))
                            }
                            LinearGradient(colors: [.clear, .black.opacity(0.5)], startPoint: .center, endPoint: .bottom)
                            Text(verbatim: item.value.capitalized)
                                .font(.caption.bold())
                                .foregroundStyle(.white)
                                .padding(6)
                        }
                        .frame(width: 110, height: 110)
                        .clipShape(RoundedRectangle(cornerRadius: 10))
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal)
        }
    }
}

/// A place or object from the explore page, resolved via `/api/search/metadata`.
struct ExploreDestination: Hashable {
    enum Kind: Hashable {
        case place
        case object
    }

    let kind: Kind
    let value: String
}

struct MetadataResultsView: View {
    let destination: ExploreDestination

    @Environment(AppSession.self) private var session
    @State private var assets: [AssetDto] = []
    @State private var nextPage: Int? = 1
    @State private var isLoading = false
    @State private var errorMessage: String?

    var body: some View {
        ScrollView {
            if assets.isEmpty && isLoading {
                ProgressView().padding(.top, 80)
            }
            FlatAssetGrid(assets: assets, onReachEnd: { Task { await load() } })
        }
        .navigationTitle(Text(verbatim: destination.value.capitalized))
        .task { if assets.isEmpty { await load() } }
        .errorAlert($errorMessage)
    }

    private func load() async {
        guard let api = session.api, let page = nextPage, !isLoading else { return }
        isLoading = true
        defer { isLoading = false }
        var request = MetadataSearchRequest(page: page, size: 200)
        switch destination.kind {
        case .place: request.city = destination.value
        case .object: request.objects = [destination.value]
        }
        do {
            let response = try await api.metadataSearch(request)
            assets.append(contentsOf: response.assets)
            nextPage = response.nextPage
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}
