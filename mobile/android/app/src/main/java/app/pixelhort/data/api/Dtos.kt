package app.pixelhort.data.api

import kotlinx.serialization.Serializable

// ---------- Auth & users ----------

@Serializable
data class PingResponse(val res: String = "")

@Serializable
data class ServerFeatures(
    val smartSearch: Boolean = false,
    val faces: Boolean = false,
    val ocr: Boolean = false,
    val objects: Boolean = false,
    val oauth: Boolean = false,
    val reverseGeocoding: Boolean = false,
    val cast: Boolean = false,
)

@Serializable
data class ServerInfo(
    val version: String = "",
    val features: ServerFeatures = ServerFeatures(),
    val publicUrl: String? = null,
)

@Serializable
data class AuthStatus(
    val setupRequired: Boolean = false,
    val oauthEnabled: Boolean = false,
    val oauthButtonText: String? = null,
    val passwordLoginEnabled: Boolean = true,
)

@Serializable
data class LoginRequest(
    val email: String,
    val password: String,
    val deviceType: String = "android",
    val deviceName: String? = null,
)

@Serializable
data class UserDto(
    val id: String,
    val email: String = "",
    val name: String = "",
    val isAdmin: Boolean = false,
    val quotaBytes: Long? = null,
    val usageBytes: Long = 0,
    val hasPin: Boolean = false,
    val oauthId: String? = null,
    val createdAt: String? = null,
    val updatedAt: String? = null,
)

@Serializable
data class UserSummary(val id: String, val name: String = "", val email: String = "")

@Serializable
data class LoginResponse(val accessToken: String, val user: UserDto)

@Serializable
data class OAuthAuthorizeResponse(val url: String)

@Serializable
data class OAuthCallbackRequest(val url: String)

@Serializable
data class PinRequest(val pin: String)

@Serializable
data class SetPinRequest(val pin: String, val currentPin: String? = null)

@Serializable
data class UnlockResponse(val unlockedUntil: String? = null)

@Serializable
data class SessionDto(val id: String = "", val unlockedUntil: String? = null, val hasPin: Boolean = false)

// ---------- Assets ----------

@Serializable
data class ExifDto(
    val make: String? = null,
    val model: String? = null,
    val lensModel: String? = null,
    val fNumber: Double? = null,
    val exposureTime: String? = null,
    val iso: Double? = null,
    val focalLength: Double? = null,
    val latitude: Double? = null,
    val longitude: Double? = null,
    val altitude: Double? = null,
    val city: String? = null,
    val state: String? = null,
    val country: String? = null,
    val orientation: String? = null,
    val dateTimeOriginal: String? = null,
    val timeZone: String? = null,
    val projectionType: String? = null,
    val description: String? = null,
)

@Serializable
data class TagRef(val id: String, val value: String = "", val color: String? = null)

@Serializable
data class PersonRef(val id: String, val name: String = "")

@Serializable
data class AssetDto(
    val id: String,
    val ownerId: String = "",
    val libraryId: String? = null,
    val deviceAssetId: String? = null,
    val deviceId: String? = null,
    val type: String = "IMAGE",
    val originalFileName: String = "",
    val originalMimeType: String? = null,
    val checksum: String? = null,
    val fileSize: Long? = null,
    val fileCreatedAt: String? = null,
    val fileModifiedAt: String? = null,
    val localDateTime: String? = null,
    val createdAt: String? = null,
    val updatedAt: String? = null,
    val width: Int? = null,
    val height: Int? = null,
    val duration: Double? = null,
    val isFavorite: Boolean = false,
    val visibility: String = "timeline",
    val isTrashed: Boolean = false,
    val isOffline: Boolean = false,
    val livePhotoVideoId: String? = null,
    val isMotionPhoto: Boolean = false,
    val projectionType: String? = null,
    val isPanorama: Boolean = false,
    val isRaw: Boolean = false,
    val thumbhash: String? = null,
    val rating: Int? = null,
    val description: String? = null,
    val hasEdits: Boolean = false,
    val duplicateId: String? = null,
    val exif: ExifDto? = null,
    val tags: List<TagRef> = emptyList(),
    val people: List<PersonRef> = emptyList(),
) {
    val isVideo: Boolean get() = type == "VIDEO"
    val hasLiveVideo: Boolean get() = livePhotoVideoId != null
}

@Serializable
data class UploadResponse(val id: String, val status: String = "created")

@Serializable
data class BulkCheckItem(val id: String, val checksum: String)

@Serializable
data class BulkCheckRequest(val assets: List<BulkCheckItem>)

@Serializable
data class BulkCheckResult(
    val id: String,
    val action: String,
    val reason: String? = null,
    val assetId: String? = null,
)

@Serializable
data class BulkCheckResponse(val results: List<BulkCheckResult> = emptyList())

@Serializable
data class ExistRequest(val deviceId: String, val deviceAssetIds: List<String>)

@Serializable
data class ExistResponse(val existingIds: List<String> = emptyList())

@Serializable
data class AssetUpdateRequest(
    val isFavorite: Boolean? = null,
    val visibility: String? = null,
    val description: String? = null,
    val rating: Int? = null,
)

@Serializable
data class AssetBulkUpdateRequest(
    val ids: List<String>,
    val isFavorite: Boolean? = null,
    val visibility: String? = null,
    val rating: Int? = null,
)

@Serializable
data class AssetDeleteRequest(val ids: List<String>, val force: Boolean = false)

@Serializable
data class CastUrlResponse(val url: String, val mimeType: String? = null, val imageUrl: String? = null)

@Serializable
data class RestoreRequest(val ids: List<String>? = null)

// ---------- Timeline ----------

@Serializable
data class TimeBucketDto(val timeBucket: String, val count: Int)

// ---------- Albums ----------

@Serializable
data class AlbumUserDto(val user: UserSummary, val role: String = "viewer")

@Serializable
data class AlbumDto(
    val id: String,
    val ownerId: String = "",
    val owner: UserSummary? = null,
    val albumName: String = "",
    val description: String? = null,
    val albumThumbnailAssetId: String? = null,
    val assetCount: Int = 0,
    val startDate: String? = null,
    val endDate: String? = null,
    val createdAt: String? = null,
    val updatedAt: String? = null,
    val shared: Boolean = false,
    val order: String? = "desc",
    val albumUsers: List<AlbumUserDto> = emptyList(),
    val assets: List<AssetDto> = emptyList(),
)

@Serializable
data class AlbumUserRole(val userId: String, val role: String)

@Serializable
data class CreateAlbumRequest(
    val albumName: String,
    val description: String? = null,
    val assetIds: List<String>? = null,
    val albumUsers: List<AlbumUserRole>? = null,
)

@Serializable
data class UpdateAlbumRequest(val albumName: String? = null, val description: String? = null)

@Serializable
data class IdsRequest(val ids: List<String>)

@Serializable
data class BulkIdResult(val id: String, val success: Boolean, val error: String? = null)

@Serializable
data class AlbumUsersRequest(val albumUsers: List<AlbumUserRole>)

// ---------- Map / memories / people / search ----------

@Serializable
data class MapMarkerDto(
    val id: String,
    val lat: Double,
    val lon: Double,
    val city: String? = null,
    val state: String? = null,
    val country: String? = null,
)

@Serializable
data class MemoryDto(
    val id: String,
    val type: String = "on_this_day",
    val yearsAgo: Int? = null,
    val year: Int? = null,
    val title: String = "",
    val assets: List<AssetDto> = emptyList(),
)

@Serializable
data class PersonDto(
    val id: String,
    val name: String = "",
    val birthDate: String? = null,
    val isHidden: Boolean = false,
    val faceCount: Int = 0,
    val thumbnailUrl: String? = null,
    val updatedAt: String? = null,
)

@Serializable
data class PeopleResponse(val people: List<PersonDto> = emptyList(), val total: Int = 0, val hidden: Int = 0)

@Serializable
data class UpdatePersonRequest(val name: String? = null, val isHidden: Boolean? = null)

@Serializable
data class SearchResponse(val assets: List<AssetDto> = emptyList(), val nextPage: Int? = null, val total: Int = 0)

@Serializable
data class SmartSearchRequest(
    val query: String,
    val page: Int? = null,
    val size: Int? = null,
    val type: String? = null,
    val isFavorite: Boolean? = null,
    val city: String? = null,
    val country: String? = null,
    val takenAfter: String? = null,
    val takenBefore: String? = null,
)

@Serializable
data class MetadataSearchRequest(
    val originalFileName: String? = null,
    val text: String? = null,
    val city: String? = null,
    val country: String? = null,
    val takenAfter: String? = null,
    val takenBefore: String? = null,
    val type: String? = null,
    val isFavorite: Boolean? = null,
    val personIds: List<String>? = null,
    val objects: List<String>? = null,
    val page: Int? = null,
    val size: Int? = null,
)

@Serializable
data class ExploreItem(val value: String, val assetId: String? = null)

@Serializable
data class ExploreResponse(
    val places: List<ExploreItem> = emptyList(),
    val objects: List<ExploreItem> = emptyList(),
    val people: List<PersonDto> = emptyList(),
)

// ---------- Partners ----------

@Serializable
data class PartnerDto(val user: UserSummary, val inTimeline: Boolean = false)

@Serializable
data class PartnerUpdateRequest(val inTimeline: Boolean)

// ---------- Shared links ----------

@Serializable
data class SharedLinkDto(
    val id: String,
    val key: String = "",
    val type: String = "INDIVIDUAL",
    val albumId: String? = null,
    val album: AlbumDto? = null,
    val assetIds: List<String> = emptyList(),
    val description: String? = null,
    val hasPassword: Boolean = false,
    val expiresAt: String? = null,
    val allowDownload: Boolean = true,
    val allowUpload: Boolean = false,
    val showMetadata: Boolean = true,
    val createdAt: String? = null,
    val url: String = "",
)

@Serializable
data class CreateSharedLinkRequest(
    val type: String,
    val albumId: String? = null,
    val assetIds: List<String>? = null,
    val description: String? = null,
    val password: String? = null,
    val expiresAt: String? = null,
    val allowDownload: Boolean? = null,
    val allowUpload: Boolean? = null,
    val showMetadata: Boolean? = null,
)

@Serializable
data class ApiError(val statusCode: Int = 0, val error: String? = null, val message: String? = null)
