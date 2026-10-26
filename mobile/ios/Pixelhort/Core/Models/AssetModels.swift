import Foundation

enum AssetType: String, Codable, Hashable, Sendable {
    case image = "IMAGE"
    case video = "VIDEO"

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = AssetType(rawValue: raw.uppercased()) ?? .image
    }
}

enum AssetVisibility: String, Codable, Hashable, Sendable, CaseIterable {
    case timeline
    case archive
    case hidden
    case locked

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = AssetVisibility(rawValue: raw.lowercased()) ?? .timeline
    }
}

struct AssetTagRef: Decodable, Identifiable, Hashable, Sendable {
    let id: String
    var value: String?
    var color: String?
}

struct AssetPersonRef: Decodable, Identifiable, Hashable, Sendable {
    let id: String
    var name: String?
}

struct ExifDto: Decodable, Hashable, Sendable {
    var make: String?
    var model: String?
    var lensModel: String?
    var fNumber: Double?
    var exposureTime: String?
    var iso: Double?
    var focalLength: Double?
    var latitude: Double?
    var longitude: Double?
    var altitude: Double?
    var city: String?
    var state: String?
    var country: String?
    var orientation: String?
    var dateTimeOriginal: Date?
    var timeZone: String?
    var projectionType: String?
    var description: String?

    private enum CodingKeys: String, CodingKey {
        case make, model, lensModel, fNumber, exposureTime, iso, focalLength, latitude, longitude,
             altitude, city, state, country, orientation, dateTimeOriginal, timeZone, projectionType, description
    }

    init(from decoder: Decoder) throws {
        // The contract marks every field nullable without fixing numeric vs. string
        // representation, so decode leniently.
        let c = try decoder.container(keyedBy: CodingKeys.self)
        make = c.lenientString(.make)
        model = c.lenientString(.model)
        lensModel = c.lenientString(.lensModel)
        fNumber = c.lenientDouble(.fNumber)
        exposureTime = c.lenientString(.exposureTime)
        iso = c.lenientDouble(.iso)
        focalLength = c.lenientDouble(.focalLength)
        latitude = c.lenientDouble(.latitude)
        longitude = c.lenientDouble(.longitude)
        altitude = c.lenientDouble(.altitude)
        city = c.lenientString(.city)
        state = c.lenientString(.state)
        country = c.lenientString(.country)
        orientation = c.lenientString(.orientation)
        dateTimeOriginal = try? c.decodeIfPresent(Date.self, forKey: .dateTimeOriginal)
        timeZone = c.lenientString(.timeZone)
        projectionType = c.lenientString(.projectionType)
        description = c.lenientString(.description)
    }

    var hasLocation: Bool { latitude != nil && longitude != nil }
}

struct AssetDto: Decodable, Identifiable, Hashable, Sendable {
    let id: String
    var ownerId: String?
    var libraryId: String?
    var deviceAssetId: String?
    var deviceId: String?
    var type: AssetType
    var originalFileName: String?
    var originalMimeType: String?
    var checksum: String?
    var fileSize: Int64?
    var fileCreatedAt: Date?
    var fileModifiedAt: Date?
    var localDateTime: Date?
    var createdAt: Date?
    var updatedAt: Date?
    var width: Int?
    var height: Int?
    var duration: Double?
    var isFavorite: Bool
    var visibility: AssetVisibility
    var isTrashed: Bool
    var isOffline: Bool?
    var livePhotoVideoId: String?
    var isMotionPhoto: Bool?
    var projectionType: String?
    var isPanorama: Bool?
    var isRaw: Bool?
    var thumbhash: String?
    var rating: Int?
    var description: String?
    var hasEdits: Bool?
    var duplicateId: String?
    var exif: ExifDto?
    var tags: [AssetTagRef]?
    var people: [AssetPersonRef]?

    private enum CodingKeys: String, CodingKey {
        case id, ownerId, libraryId, deviceAssetId, deviceId, type, originalFileName, originalMimeType,
             checksum, fileSize, fileCreatedAt, fileModifiedAt, localDateTime, createdAt, updatedAt,
             width, height, duration, isFavorite, visibility, isTrashed, isOffline, livePhotoVideoId,
             isMotionPhoto, projectionType, isPanorama, isRaw, thumbhash, rating, description, hasEdits,
             duplicateId, exif, tags, people
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(String.self, forKey: .id)
        ownerId = try? c.decodeIfPresent(String.self, forKey: .ownerId)
        libraryId = try? c.decodeIfPresent(String.self, forKey: .libraryId)
        deviceAssetId = try? c.decodeIfPresent(String.self, forKey: .deviceAssetId)
        deviceId = try? c.decodeIfPresent(String.self, forKey: .deviceId)
        type = (try? c.decodeIfPresent(AssetType.self, forKey: .type)) ?? .image
        originalFileName = try? c.decodeIfPresent(String.self, forKey: .originalFileName)
        originalMimeType = try? c.decodeIfPresent(String.self, forKey: .originalMimeType)
        checksum = try? c.decodeIfPresent(String.self, forKey: .checksum)
        fileSize = try? c.decodeIfPresent(Int64.self, forKey: .fileSize)
        fileCreatedAt = try? c.decodeIfPresent(Date.self, forKey: .fileCreatedAt)
        fileModifiedAt = try? c.decodeIfPresent(Date.self, forKey: .fileModifiedAt)
        localDateTime = try? c.decodeIfPresent(Date.self, forKey: .localDateTime)
        createdAt = try? c.decodeIfPresent(Date.self, forKey: .createdAt)
        updatedAt = try? c.decodeIfPresent(Date.self, forKey: .updatedAt)
        width = try? c.decodeIfPresent(Int.self, forKey: .width)
        height = try? c.decodeIfPresent(Int.self, forKey: .height)
        duration = c.lenientDouble(.duration)
        isFavorite = (try? c.decodeIfPresent(Bool.self, forKey: .isFavorite)) ?? false
        visibility = (try? c.decodeIfPresent(AssetVisibility.self, forKey: .visibility)) ?? .timeline
        isTrashed = (try? c.decodeIfPresent(Bool.self, forKey: .isTrashed)) ?? false
        isOffline = try? c.decodeIfPresent(Bool.self, forKey: .isOffline)
        livePhotoVideoId = try? c.decodeIfPresent(String.self, forKey: .livePhotoVideoId)
        isMotionPhoto = try? c.decodeIfPresent(Bool.self, forKey: .isMotionPhoto)
        projectionType = try? c.decodeIfPresent(String.self, forKey: .projectionType)
        isPanorama = try? c.decodeIfPresent(Bool.self, forKey: .isPanorama)
        isRaw = try? c.decodeIfPresent(Bool.self, forKey: .isRaw)
        thumbhash = try? c.decodeIfPresent(String.self, forKey: .thumbhash)
        rating = try? c.decodeIfPresent(Int.self, forKey: .rating)
        description = try? c.decodeIfPresent(String.self, forKey: .description)
        hasEdits = try? c.decodeIfPresent(Bool.self, forKey: .hasEdits)
        duplicateId = try? c.decodeIfPresent(String.self, forKey: .duplicateId)
        exif = try? c.decodeIfPresent(ExifDto.self, forKey: .exif)
        tags = try? c.decodeIfPresent([AssetTagRef].self, forKey: .tags)
        people = try? c.decodeIfPresent([AssetPersonRef].self, forKey: .people)
    }

    var isVideo: Bool { type == .video }
    var isLivePhoto: Bool { livePhotoVideoId != nil }
    /// Capture time used for grouping/sorting (falls back to file creation time).
    var sortDate: Date { localDateTime ?? fileCreatedAt ?? createdAt ?? Date(timeIntervalSince1970: 0) }
    var aspectRatio: Double {
        guard let w = width, let h = height, w > 0, h > 0 else { return 1 }
        return Double(w) / Double(h)
    }
}

// MARK: - Asset requests / responses

struct UpdateAssetRequest: Encodable, Sendable {
    var isFavorite: Bool?
    var visibility: AssetVisibility?
    var description: String?
    var rating: Int?
}

struct BulkUpdateAssetsRequest: Encodable, Sendable {
    let ids: [String]
    var isFavorite: Bool?
    var visibility: AssetVisibility?
    var rating: Int?
}

struct DeleteAssetsRequest: Encodable, Sendable {
    let ids: [String]
    var force: Bool?
}

struct IdsRequest: Encodable, Sendable {
    let ids: [String]
}

struct TrashRestoreRequest: Encodable, Sendable {
    var ids: [String]?
}

struct UploadResponse: Decodable, Sendable {
    let id: String
    let status: String

    var isDuplicate: Bool { status == "duplicate" }
}

struct BulkUploadCheckItem: Codable, Sendable {
    let id: String
    let checksum: String
}

struct BulkUploadCheckRequest: Encodable, Sendable {
    let assets: [BulkUploadCheckItem]
}

struct BulkUploadCheckResult: Decodable, Sendable {
    let id: String
    let action: String
    var reason: String?
    var assetId: String?

    var isAccepted: Bool { action == "accept" }
}

struct BulkUploadCheckResponse: Decodable, Sendable {
    let results: [BulkUploadCheckResult]
}

struct AssetExistRequest: Encodable, Sendable {
    let deviceId: String
    let deviceAssetIds: [String]
}

struct AssetExistResponse: Decodable, Sendable {
    let existingIds: [String]
}

struct CastURLResponse: Decodable, Sendable {
    let url: String
    var mimeType: String?
    var imageUrl: String?
}

struct AssetStatistics: Decodable, Sendable {
    var images: Int
    var videos: Int
    var total: Int
}

// MARK: - Timeline

struct TimeBucket: Decodable, Identifiable, Hashable, Sendable {
    let timeBucket: String
    let count: Int

    var id: String { timeBucket }
}

/// Common filter query parameters of `/api/timeline/*`.
struct TimelineFilter: Hashable, Sendable {
    var visibility: AssetVisibility?
    var isFavorite: Bool?
    var albumId: String?
    var personId: String?
    var tagId: String?
    var withPartners: Bool?
    var isTrashed: Bool?
    var order: String?

    init(visibility: AssetVisibility? = nil, isFavorite: Bool? = nil, albumId: String? = nil,
         personId: String? = nil, tagId: String? = nil, withPartners: Bool? = nil,
         isTrashed: Bool? = nil, order: String? = nil) {
        self.visibility = visibility
        self.isFavorite = isFavorite
        self.albumId = albumId
        self.personId = personId
        self.tagId = tagId
        self.withPartners = withPartners
        self.isTrashed = isTrashed
        self.order = order
    }

    var queryItems: [URLQueryItem] {
        var items: [URLQueryItem] = []
        if let visibility { items.append(URLQueryItem(name: "visibility", value: visibility.rawValue)) }
        if let isFavorite { items.append(URLQueryItem(name: "isFavorite", value: isFavorite ? "true" : "false")) }
        if let albumId { items.append(URLQueryItem(name: "albumId", value: albumId)) }
        if let personId { items.append(URLQueryItem(name: "personId", value: personId)) }
        if let tagId { items.append(URLQueryItem(name: "tagId", value: tagId)) }
        if let withPartners { items.append(URLQueryItem(name: "withPartners", value: withPartners ? "true" : "false")) }
        if let isTrashed { items.append(URLQueryItem(name: "isTrashed", value: isTrashed ? "true" : "false")) }
        if let order { items.append(URLQueryItem(name: "order", value: order)) }
        return items
    }
}

struct SyncDeltaResponse: Decodable, Sendable {
    let upserted: [AssetDto]
    let deleted: [String]
    let hasMore: Bool
    let cursor: String?
}

enum ThumbnailSize: String, Sendable {
    case thumbnail
    case preview
}
