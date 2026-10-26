import MapKit
import SwiftUI

/// Details of an asset (`GET /api/assets/{id}` incl. EXIF, people, tags).
struct AssetInfoSheet: View {
    let assetId: String

    @Environment(AppSession.self) private var session
    @Environment(\.dismiss) private var dismiss
    @State private var asset: AssetDto?
    @State private var errorMessage: String?

    var body: some View {
        NavigationStack {
            Group {
                if let asset {
                    content(asset)
                } else if let errorMessage {
                    StateView(kind: .error(errorMessage)) { Task { await load() } }
                } else {
                    StateView(kind: .loading)
                }
            }
            .navigationTitle("info.title")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("common.done") { dismiss() }
                }
            }
        }
        .task { await load() }
    }

    private func load() async {
        guard let api = session.api else { return }
        do {
            asset = try await api.asset(id: assetId)
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    @ViewBuilder
    private func content(_ asset: AssetDto) -> some View {
        List {
            if let description = asset.description, !description.isEmpty {
                Section { Text(verbatim: description) }
            }

            Section("info.section.details") {
                row("info.date", TimelineGrouping.wallClockString(asset.sortDate))
                if let name = asset.originalFileName { row("info.fileName", name) }
                if let size = asset.fileSize {
                    row("info.fileSize", ByteCountFormatter.string(fromByteCount: size, countStyle: .file))
                }
                if let width = asset.width, let height = asset.height {
                    row("info.dimensions", "\(width) × \(height)")
                }
                if let duration = asset.duration, asset.isVideo {
                    row("info.duration", TimelineGrouping.durationString(duration))
                }
                if let mime = asset.originalMimeType { row("info.type", mime) }
            }

            if let exif = asset.exif, hasCameraInfo(exif) {
                Section("info.section.camera") {
                    let camera = [exif.make, exif.model].compactMap { $0 }.joined(separator: " ")
                    if !camera.isEmpty { row("info.camera", camera) }
                    if let lens = exif.lensModel { row("info.lens", lens) }
                    let settings = cameraSettings(exif)
                    if !settings.isEmpty { row("info.exposure", settings) }
                }
            }

            if let people = asset.people, !people.isEmpty {
                Section("info.section.people") {
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 14) {
                            ForEach(people) { person in
                                VStack(spacing: 4) {
                                    PersonAvatar(personId: person.id, diameter: 56)
                                    Text(verbatim: person.name?.isEmpty == false ? person.name! : "–")
                                        .font(.caption)
                                        .lineLimit(1)
                                }
                                .frame(width: 70)
                            }
                        }
                        .padding(.vertical, 4)
                    }
                }
            }

            if let tags = asset.tags, !tags.isEmpty {
                Section("info.section.tags") {
                    Text(verbatim: tags.compactMap(\.value).joined(separator: ", "))
                }
            }

            if let exif = asset.exif, let lat = exif.latitude, let lon = exif.longitude {
                Section("info.section.location") {
                    let place = [exif.city, exif.state, exif.country].compactMap { $0 }.joined(separator: ", ")
                    if !place.isEmpty { Text(verbatim: place) }
                    let coordinate = CLLocationCoordinate2D(latitude: lat, longitude: lon)
                    Map(initialPosition: .region(MKCoordinateRegion(
                        center: coordinate,
                        span: MKCoordinateSpan(latitudeDelta: 0.05, longitudeDelta: 0.05)))) {
                        Marker(place.isEmpty ? (asset.originalFileName ?? "") : place, coordinate: coordinate)
                    }
                    .frame(height: 200)
                    .listRowInsets(EdgeInsets())
                }
            }
        }
    }

    private func row(_ title: LocalizedStringKey, _ value: String) -> some View {
        LabeledContent {
            Text(verbatim: value)
                .multilineTextAlignment(.trailing)
                .textSelection(.enabled)
        } label: {
            Text(title)
        }
    }

    private func hasCameraInfo(_ exif: ExifDto) -> Bool {
        exif.make != nil || exif.model != nil || exif.lensModel != nil || exif.fNumber != nil
    }

    private func cameraSettings(_ exif: ExifDto) -> String {
        var parts: [String] = []
        if let f = exif.fNumber { parts.append(String(format: "ƒ/%.1f", f)) }
        if let exposure = exif.exposureTime { parts.append(exposure.hasSuffix("s") ? exposure : exposure + " s") }
        if let focal = exif.focalLength { parts.append(String(format: "%.0f mm", focal)) }
        if let iso = exif.iso { parts.append("ISO \(Int(iso))") }
        return parts.joined(separator: " · ")
    }
}
