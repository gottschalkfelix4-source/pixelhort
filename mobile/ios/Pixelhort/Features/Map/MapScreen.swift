import MapKit
import SwiftUI

/// Map of all geotagged assets (`/api/map/markers`) with simple grid clustering.
struct MapScreen: View {
    @Environment(AppSession.self) private var session
    @AppStorage("timeline.withPartners") private var withPartners = false
    @State private var markers: [MapMarkerDto] = []
    @State private var clusters: [MarkerCluster] = []
    @State private var position: MapCameraPosition = .automatic
    @State private var region: MKCoordinateRegion?
    @State private var favoritesOnly = false
    @State private var selectedClusterId: String?
    @State private var viewer: ViewerRequest?
    @State private var isLoading = false
    @State private var errorMessage: String?

    var body: some View {
        Map(position: $position, selection: $selectedClusterId) {
            ForEach(clusters) { cluster in
                Annotation(cluster.title, coordinate: cluster.coordinate) {
                    // Annotation content is hosted separately and does not inherit the environment.
                    ClusterBadge(cluster: cluster)
                        .environment(session)
                }
                .tag(cluster.id)
            }
        }
        .mapControls {
            MapCompass()
            MapScaleView()
        }
        .onMapCameraChange(frequency: .onEnd) { context in
            region = context.region
            recluster()
        }
        .onChange(of: selectedClusterId) { _, id in
            guard let id, let cluster = clusters.first(where: { $0.id == id }) else { return }
            selectedClusterId = nil
            Task { await open(cluster) }
        }
        .overlay {
            if isLoading && markers.isEmpty { ProgressView() }
        }
        .navigationTitle("library.map")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Toggle(isOn: $favoritesOnly) {
                    Image(systemName: favoritesOnly ? "heart.fill" : "heart")
                }
                .toggleStyle(.button)
                .accessibilityLabel(Text("library.favorites"))
            }
        }
        .task(id: favoritesOnly) { await load() }
        .fullScreenCover(item: $viewer) { request in
            AssetViewer(request: request) {}
        }
        .errorAlert($errorMessage)
    }

    private func load() async {
        guard let api = session.api else { return }
        isLoading = true
        defer { isLoading = false }
        do {
            markers = try await api.mapMarkers(isFavorite: favoritesOnly ? true : nil,
                                               withPartners: withPartners ? true : nil)
            if let rect = Self.mapRect(fitting: markers) {
                position = .rect(rect)
            }
            recluster()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    /// Map rect containing all markers (with padding); MapKit fits it completely into the view,
    /// so the photos are in sight after loading.
    static func mapRect(fitting markers: [MapMarkerDto]) -> MKMapRect? {
        guard !markers.isEmpty else { return nil }
        var rect = MKMapRect.null
        for marker in markers {
            let point = MKMapPoint(CLLocationCoordinate2D(latitude: marker.lat, longitude: marker.lon))
            rect = rect.union(MKMapRect(origin: point, size: MKMapSize(width: 0, height: 0)))
        }
        // At least ~2 km across, plus 20 % padding on each side.
        let minSize = MKMapPointsPerMeterAtLatitude(MKMapPoint(x: rect.midX, y: rect.midY).coordinate.latitude) * 2000
        let width = max(rect.width, minSize), height = max(rect.height, minSize)
        let padded = MKMapRect(x: rect.midX - width * 0.7, y: rect.midY - height * 0.7, width: width * 1.4, height: height * 1.4)
        return padded.intersection(.world)
    }

    /// Groups markers into a grid whose cell size depends on the visible span.
    private func recluster() {
        let span = region?.span ?? MKCoordinateSpan(latitudeDelta: 60, longitudeDelta: 60)
        let cellLat = max(span.latitudeDelta / 8, 0.0005)
        let cellLon = max(span.longitudeDelta / 6, 0.0005)
        var buckets: [String: [MapMarkerDto]] = [:]
        for marker in markers {
            let key = "\(Int(floor(marker.lat / cellLat)))_\(Int(floor(marker.lon / cellLon)))"
            buckets[key, default: []].append(marker)
        }
        clusters = buckets.map { key, items in
            let lat = items.map(\.lat).reduce(0, +) / Double(items.count)
            let lon = items.map(\.lon).reduce(0, +) / Double(items.count)
            return MarkerCluster(id: key, coordinate: CLLocationCoordinate2D(latitude: lat, longitude: lon),
                                 markers: items)
        }
    }

    private func open(_ cluster: MarkerCluster) async {
        guard let api = session.api else { return }
        // Markers carry ids only; load full assets for the viewer (limit for responsiveness).
        var assets: [AssetDto] = []
        for marker in cluster.markers.prefix(60) {
            if let asset = try? await api.asset(id: marker.id) {
                assets.append(asset)
            }
        }
        guard let first = assets.first else { return }
        viewer = ViewerRequest(assets: assets.sorted { $0.sortDate > $1.sortDate }, startId: first.id)
    }
}

struct MarkerCluster: Identifiable, Hashable {
    let id: String
    let coordinate: CLLocationCoordinate2D
    let markers: [MapMarkerDto]

    var title: String {
        markers.first?.city ?? markers.first?.country ?? ""
    }

    static func == (lhs: MarkerCluster, rhs: MarkerCluster) -> Bool { lhs.id == rhs.id }
    func hash(into hasher: inout Hasher) { hasher.combine(id) }
}

private struct ClusterBadge: View {
    let cluster: MarkerCluster

    var body: some View {
        ZStack(alignment: .topTrailing) {
            if let first = cluster.markers.first {
                AssetThumbnail(assetId: first.id)
                    .frame(width: 48, height: 48)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
                    .overlay(RoundedRectangle(cornerRadius: 8).stroke(.white, lineWidth: 2))
                    .shadow(radius: 3)
            }
            if cluster.markers.count > 1 {
                Text(verbatim: "\(cluster.markers.count)")
                    .font(.caption2.bold())
                    .foregroundStyle(.white)
                    .padding(.horizontal, 5)
                    .padding(.vertical, 2)
                    .background(Color.accentColor, in: Capsule())
                    .offset(x: 8, y: -8)
            }
        }
    }
}
