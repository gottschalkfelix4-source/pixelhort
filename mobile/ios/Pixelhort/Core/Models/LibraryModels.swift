import Foundation

// MARK: - Albums

enum AlbumRole: String, Codable, Hashable, Sendable, CaseIterable {
    case editor
    case viewer
}

struct AlbumUser: Decodable, Hashable, Sendable {
    let user: UserSummary
    var role: AlbumRole?
}

struct AlbumDto: Decodable, Identifiable, Hashable, Sendable {
    let id: String
    var ownerId: String?
    var owner: UserSummary?
    var albumName: String
    var description: String?
    var albumThumbnailAssetId: String?
    var assetCount: Int?
    var startDate: Date?
    var endDate: Date?
    var createdAt: Date?
    var updatedAt: Date?
    var shared: Bool?
    var order: String?
    var albumUsers: [AlbumUser]?
    var assets: [AssetDto]?
}

struct AlbumUserRequest: Encodable, Sendable {
    let userId: String
    let role: AlbumRole
}

struct CreateAlbumRequest: Encodable, Sendable {
    let albumName: String
    var description: String?
    var assetIds: [String]?
    var albumUsers: [AlbumUserRequest]?
}

struct UpdateAlbumRequest: Encodable, Sendable {
    var albumName: String?
    var description: String?
    var albumThumbnailAssetId: String?
    var order: String?
}

struct AlbumUsersRequest: Encodable, Sendable {
    let albumUsers: [AlbumUserRequest]
}

struct AlbumAssetResult: Decodable, Sendable {
    let id: String
    let success: Bool
    var error: String?
}

// MARK: - People

struct PersonDto: Decodable, Identifiable, Hashable, Sendable {
    let id: String
    var name: String?
    var birthDate: String?
    var isHidden: Bool?
    var faceCount: Int?
    var thumbnailUrl: String?
    var updatedAt: Date?

    var displayName: String? {
        guard let name, !name.isEmpty else { return nil }
        return name
    }
}

struct PeopleResponse: Decodable, Sendable {
    let people: [PersonDto]
    var total: Int?
    var hidden: Int?
}

struct UpdatePersonRequest: Encodable, Sendable {
    var name: String?
    var birthDate: String?
    var isHidden: Bool?
    var featureFaceId: String?
}

// MARK: - Search

struct SearchResponse: Decodable, Sendable {
    let assets: [AssetDto]
    var nextPage: Int?
    var total: Int?
}

struct SmartSearchRequest: Encodable, Sendable {
    let query: String
    var page: Int?
    var size: Int?
}

struct MetadataSearchRequest: Encodable, Sendable {
    var originalFileName: String?
    var text: String?
    var city: String?
    var state: String?
    var country: String?
    var type: AssetType?
    var isFavorite: Bool?
    var personIds: [String]?
    var objects: [String]?
    var page: Int?
    var size: Int?
}

struct ExploreItem: Decodable, Hashable, Sendable, Identifiable {
    let value: String
    var assetId: String?

    var id: String { value }
}

struct ExploreResponse: Decodable, Sendable {
    var places: [ExploreItem]
    var objects: [ExploreItem]
    var people: [PersonDto]

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        places = (try? c.decodeIfPresent([ExploreItem].self, forKey: .places)) ?? []
        objects = (try? c.decodeIfPresent([ExploreItem].self, forKey: .objects)) ?? []
        people = (try? c.decodeIfPresent([PersonDto].self, forKey: .people)) ?? []
    }

    private enum CodingKeys: String, CodingKey { case places, objects, people }
}

// MARK: - Map

struct MapMarkerDto: Decodable, Identifiable, Hashable, Sendable {
    let id: String
    let lat: Double
    let lon: Double
    var city: String?
    var state: String?
    var country: String?
}

// MARK: - Memories

struct Memory: Decodable, Identifiable, Hashable, Sendable {
    let id: String
    var type: String?
    var yearsAgo: Int?
    var year: Int?
    var title: String?
    var assets: [AssetDto]
}

// MARK: - Shared links

enum SharedLinkType: String, Codable, Hashable, Sendable {
    case album = "ALBUM"
    case individual = "INDIVIDUAL"
}

struct SharedLinkDto: Decodable, Identifiable, Hashable, Sendable {
    let id: String
    let key: String
    var type: SharedLinkType?
    var albumId: String?
    var album: AlbumDto?
    var assetIds: [String]?
    var description: String?
    var hasPassword: Bool?
    var expiresAt: Date?
    var allowDownload: Bool?
    var allowUpload: Bool?
    var showMetadata: Bool?
    var createdAt: Date?
    var url: String?

    var isExpired: Bool {
        guard let expiresAt else { return false }
        return expiresAt < Date()
    }
}

struct CreateSharedLinkRequest: Encodable, Sendable {
    let type: SharedLinkType
    var albumId: String?
    var assetIds: [String]?
    var description: String?
    var password: String?
    var expiresAt: Date?
    var allowDownload: Bool?
    var allowUpload: Bool?
    var showMetadata: Bool?
}

// MARK: - Partners

enum PartnerDirection: String, Sendable {
    case sharedBy = "shared-by"
    case sharedWith = "shared-with"
}

struct Partner: Decodable, Identifiable, Hashable, Sendable {
    let user: UserSummary
    var inTimeline: Bool?

    var id: String { user.id }
}

struct PartnerUpdateRequest: Encodable, Sendable {
    let inTimeline: Bool
}
