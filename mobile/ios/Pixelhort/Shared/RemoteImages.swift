import SwiftUI
import UIKit

/// Authenticated asset thumbnail (memory → disk → network).
struct AssetThumbnail: View {
    let assetId: String
    var size: ThumbnailSize = .thumbnail
    var contentMode: ContentMode = .fill

    @Environment(AppSession.self) private var session
    @State private var image: UIImage?

    var body: some View {
        ZStack {
            Rectangle().fill(Color.secondary.opacity(0.15))
            if let image {
                Image(uiImage: image)
                    .resizable()
                    .aspectRatio(contentMode: contentMode)
            }
        }
        .task(id: assetId + size.rawValue) {
            let key = ImageLoader.assetKey(assetId, size)
            if let cached = ImageLoader.shared.cachedImage(forKey: key) {
                image = cached
                return
            }
            image = nil
            guard let api = session.api else { return }
            let loaded = await ImageLoader.shared.assetImage(assetId, size: size, api: api)
            if !Task.isCancelled {
                image = loaded
            }
        }
    }
}

/// Square grid cell (thumbnail clipped to the cell).
struct SquareThumbnail: View {
    let assetId: String

    var body: some View {
        Color.clear
            .aspectRatio(1, contentMode: .fit)
            .overlay {
                AssetThumbnail(assetId: assetId)
            }
            .clipped()
            .contentShape(Rectangle())
    }
}

/// Round face crop of a person.
struct PersonAvatar: View {
    let personId: String
    var diameter: CGFloat = 64

    @Environment(AppSession.self) private var session
    @State private var image: UIImage?

    var body: some View {
        ZStack {
            Circle().fill(Color.secondary.opacity(0.2))
            if let image {
                Image(uiImage: image)
                    .resizable()
                    .scaledToFill()
            } else {
                Image(systemName: "person.fill")
                    .foregroundStyle(.secondary)
            }
        }
        .frame(width: diameter, height: diameter)
        .clipShape(Circle())
        .task(id: personId) {
            if let cached = ImageLoader.shared.cachedImage(forKey: ImageLoader.personKey(personId)) {
                image = cached
                return
            }
            guard let api = session.api else { return }
            image = await ImageLoader.shared.personImage(personId, api: api)
        }
    }
}
