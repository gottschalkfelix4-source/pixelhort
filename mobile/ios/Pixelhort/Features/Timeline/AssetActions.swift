import SwiftUI

/// Where an asset list is shown; determines the timeline filter and the available actions.
enum AssetListContext: Hashable {
    case timeline(withPartners: Bool)
    case favorites
    case archive
    case trash
    case locked
    case album(id: String, canEdit: Bool)
    case person(id: String)

    var filter: TimelineFilter {
        switch self {
        case let .timeline(withPartners):
            return TimelineFilter(visibility: .timeline, withPartners: withPartners ? true : nil)
        case .favorites:
            return TimelineFilter(isFavorite: true)
        case .archive:
            return TimelineFilter(visibility: .archive)
        case .trash:
            return TimelineFilter(isTrashed: true)
        case .locked:
            return TimelineFilter(visibility: .locked)
        case let .album(id, _):
            return TimelineFilter(albumId: id)
        case let .person(id):
            return TimelineFilter(personId: id)
        }
    }

    /// Actions offered for a multi-selection.
    var selectionActions: [AssetAction] {
        switch self {
        case .timeline, .favorites, .person:
            return [.share, .favorite, .addToAlbum, .sharedLink, .archive, .moveToLocked, .delete]
        case .archive:
            return [.share, .favorite, .addToAlbum, .unarchive, .moveToLocked, .delete]
        case .trash:
            return [.restore, .deletePermanently]
        case .locked:
            return [.share, .removeFromLocked, .delete]
        case let .album(_, canEdit):
            return canEdit
                ? [.share, .favorite, .sharedLink, .removeFromAlbum, .archive, .delete]
                : [.share, .favorite, .addToAlbum]
        }
    }
}

enum AssetAction: String, Identifiable, Hashable {
    case share
    case favorite
    case unfavorite
    case addToAlbum
    case sharedLink
    case archive
    case unarchive
    case moveToLocked
    case removeFromLocked
    case delete
    case deletePermanently
    case restore
    case removeFromAlbum

    var id: String { rawValue }

    var title: LocalizedStringKey {
        switch self {
        case .share: return "action.share"
        case .favorite: return "action.favorite"
        case .unfavorite: return "action.unfavorite"
        case .addToAlbum: return "action.addToAlbum"
        case .sharedLink: return "action.sharedLink"
        case .archive: return "action.archive"
        case .unarchive: return "action.unarchive"
        case .moveToLocked: return "action.moveToLocked"
        case .removeFromLocked: return "action.removeFromLocked"
        case .delete: return "action.delete"
        case .deletePermanently: return "action.deletePermanently"
        case .restore: return "action.restore"
        case .removeFromAlbum: return "action.removeFromAlbum"
        }
    }

    var systemImage: String {
        switch self {
        case .share: return "square.and.arrow.up"
        case .favorite: return "heart"
        case .unfavorite: return "heart.slash"
        case .addToAlbum: return "rectangle.stack.badge.plus"
        case .sharedLink: return "link"
        case .archive: return "archivebox"
        case .unarchive: return "archivebox.fill"
        case .moveToLocked: return "lock"
        case .removeFromLocked: return "lock.open"
        case .delete: return "trash"
        case .deletePermanently: return "trash.slash"
        case .restore: return "arrow.uturn.backward"
        case .removeFromAlbum: return "minus.circle"
        }
    }

    var isDestructive: Bool {
        self == .delete || self == .deletePermanently || self == .removeFromAlbum
    }

    /// Actions that only need an API call (others open a sheet).
    var needsSheet: Bool {
        self == .addToAlbum || self == .sharedLink || self == .share
    }
}

/// Executes API side effects of asset actions.
@MainActor
enum AssetActionPerformer {
    static func perform(_ action: AssetAction, ids: [String], context: AssetListContext, api: APIClient) async throws {
        guard !ids.isEmpty else { return }
        switch action {
        case .favorite:
            try await api.updateAssets(BulkUpdateAssetsRequest(ids: ids, isFavorite: true))
        case .unfavorite:
            try await api.updateAssets(BulkUpdateAssetsRequest(ids: ids, isFavorite: false))
        case .archive:
            try await api.updateAssets(BulkUpdateAssetsRequest(ids: ids, visibility: .archive))
        case .unarchive, .removeFromLocked:
            try await api.updateAssets(BulkUpdateAssetsRequest(ids: ids, visibility: .timeline))
        case .moveToLocked:
            try await api.updateAssets(BulkUpdateAssetsRequest(ids: ids, visibility: .locked))
        case .delete:
            try await api.deleteAssets(ids: ids, force: false)
        case .deletePermanently:
            try await api.deleteAssets(ids: ids, force: true)
        case .restore:
            try await api.restoreTrash(ids: ids)
        case .removeFromAlbum:
            if case let .album(albumId, _) = context {
                try await api.removeAssets(fromAlbum: albumId, assetIds: ids)
            }
        case .share, .addToAlbum, .sharedLink:
            break // handled by the presenting view
        }
    }

    /// Downloads originals for the share sheet.
    static func downloadForSharing(_ assets: [AssetDto], api: APIClient) async throws -> [URL] {
        var urls: [URL] = []
        for asset in assets {
            let name = asset.originalFileName ?? "\(asset.id).\(asset.isVideo ? "mp4" : "jpg")"
            urls.append(try await api.download(api.originalPath(assetId: asset.id), fileName: name))
        }
        return urls
    }
}
