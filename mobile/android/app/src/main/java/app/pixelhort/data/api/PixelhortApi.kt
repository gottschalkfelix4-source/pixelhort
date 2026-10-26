package app.pixelhort.data.api

import okhttp3.MultipartBody
import okhttp3.RequestBody
import retrofit2.http.Body
import retrofit2.http.GET
import retrofit2.http.HTTP
import retrofit2.http.Header
import retrofit2.http.Multipart
import retrofit2.http.PATCH
import retrofit2.http.POST
import retrofit2.http.PUT
import retrofit2.http.Part
import retrofit2.http.Path
import retrofit2.http.Query

/** Retrofit binding of docs/API.md (only the endpoints the Android app needs). */
interface PixelhortApi {

    // Server
    @GET("api/server/ping")
    suspend fun ping(): PingResponse

    @GET("api/server/info")
    suspend fun serverInfo(): ServerInfo

    // Auth
    @GET("api/auth/status")
    suspend fun authStatus(): AuthStatus

    @POST("api/auth/login")
    suspend fun login(@Body body: LoginRequest): LoginResponse

    @POST("api/auth/logout")
    suspend fun logout()

    @GET("api/auth/oauth/authorize")
    suspend fun oauthAuthorize(@Query("redirectUri") redirectUri: String): OAuthAuthorizeResponse

    @POST("api/auth/oauth/callback")
    suspend fun oauthCallback(@Body body: OAuthCallbackRequest): LoginResponse

    @POST("api/auth/session/unlock")
    suspend fun unlockSession(@Body body: PinRequest): UnlockResponse

    @POST("api/auth/session/lock")
    suspend fun lockSession()

    @GET("api/auth/session")
    suspend fun session(): SessionDto

    // Users
    @GET("api/users/me")
    suspend fun me(): UserDto

    @PUT("api/users/me/pin")
    suspend fun setPin(@Body body: SetPinRequest)

    @GET("api/users")
    suspend fun users(): List<UserSummary>

    // Assets
    @Multipart
    @POST("api/assets")
    suspend fun upload(
        @Header("x-checksum") checksum: String?,
        @Part assetData: MultipartBody.Part,
        @Part("deviceAssetId") deviceAssetId: RequestBody,
        @Part("deviceId") deviceId: RequestBody,
        @Part("fileCreatedAt") fileCreatedAt: RequestBody,
        @Part("fileModifiedAt") fileModifiedAt: RequestBody,
        @Part("isFavorite") isFavorite: RequestBody,
    ): UploadResponse

    @POST("api/assets/bulk-upload-check")
    suspend fun bulkUploadCheck(@Body body: BulkCheckRequest): BulkCheckResponse

    @POST("api/assets/exist")
    suspend fun exist(@Body body: ExistRequest): ExistResponse

    @GET("api/assets/{id}")
    suspend fun asset(@Path("id") id: String): AssetDto

    @PUT("api/assets/{id}")
    suspend fun updateAsset(@Path("id") id: String, @Body body: AssetUpdateRequest): AssetDto

    @PUT("api/assets")
    suspend fun updateAssets(@Body body: AssetBulkUpdateRequest)

    @HTTP(method = "DELETE", path = "api/assets", hasBody = true)
    suspend fun deleteAssets(@Body body: AssetDeleteRequest)

    @POST("api/assets/{id}/cast-url")
    suspend fun castUrl(@Path("id") id: String): CastUrlResponse

    // Trash
    @GET("api/trash")
    suspend fun trash(): List<AssetDto>

    @POST("api/trash/restore")
    suspend fun restoreTrash(@Body body: RestoreRequest)

    @POST("api/trash/empty")
    suspend fun emptyTrash()

    // Timeline
    @GET("api/timeline/buckets")
    suspend fun timeBuckets(
        @Query("visibility") visibility: String? = null,
        @Query("isFavorite") isFavorite: Boolean? = null,
        @Query("albumId") albumId: String? = null,
        @Query("personId") personId: String? = null,
        @Query("withPartners") withPartners: Boolean? = null,
        @Query("isTrashed") isTrashed: Boolean? = null,
        @Query("order") order: String? = null,
    ): List<TimeBucketDto>

    @GET("api/timeline/bucket")
    suspend fun timeBucket(
        @Query("timeBucket") timeBucket: String,
        @Query("visibility") visibility: String? = null,
        @Query("isFavorite") isFavorite: Boolean? = null,
        @Query("albumId") albumId: String? = null,
        @Query("personId") personId: String? = null,
        @Query("withPartners") withPartners: Boolean? = null,
        @Query("isTrashed") isTrashed: Boolean? = null,
        @Query("order") order: String? = null,
    ): List<AssetDto>

    // Albums
    @GET("api/albums")
    suspend fun albums(@Query("shared") shared: Boolean? = null): List<AlbumDto>

    @POST("api/albums")
    suspend fun createAlbum(@Body body: CreateAlbumRequest): AlbumDto

    @GET("api/albums/{id}")
    suspend fun album(@Path("id") id: String): AlbumDto

    @PATCH("api/albums/{id}")
    suspend fun updateAlbum(@Path("id") id: String, @Body body: UpdateAlbumRequest): AlbumDto

    @HTTP(method = "DELETE", path = "api/albums/{id}")
    suspend fun deleteAlbum(@Path("id") id: String)

    @PUT("api/albums/{id}/assets")
    suspend fun addAssetsToAlbum(@Path("id") id: String, @Body body: IdsRequest): List<BulkIdResult>

    @HTTP(method = "DELETE", path = "api/albums/{id}/assets", hasBody = true)
    suspend fun removeAssetsFromAlbum(@Path("id") id: String, @Body body: IdsRequest): List<BulkIdResult>

    @PUT("api/albums/{id}/users")
    suspend fun addAlbumUsers(@Path("id") id: String, @Body body: AlbumUsersRequest): AlbumDto

    // Map / memories
    @GET("api/map/markers")
    suspend fun mapMarkers(
        @Query("isFavorite") isFavorite: Boolean? = null,
        @Query("withPartners") withPartners: Boolean? = null,
    ): List<MapMarkerDto>

    @GET("api/memories")
    suspend fun memories(@Query("date") date: String? = null): List<MemoryDto>

    // People
    @GET("api/people")
    suspend fun people(@Query("withHidden") withHidden: Boolean = false): PeopleResponse

    @GET("api/people/{id}")
    suspend fun person(@Path("id") id: String): PersonDto

    @PUT("api/people/{id}")
    suspend fun updatePerson(@Path("id") id: String, @Body body: UpdatePersonRequest): PersonDto

    // Search
    @POST("api/search/smart")
    suspend fun smartSearch(@Body body: SmartSearchRequest): SearchResponse

    @POST("api/search/metadata")
    suspend fun metadataSearch(@Body body: MetadataSearchRequest): SearchResponse

    @GET("api/search/suggestions")
    suspend fun suggestions(@Query("type") type: String, @Query("q") q: String? = null): List<String>

    @GET("api/search/explore")
    suspend fun explore(): ExploreResponse

    // Partners
    @GET("api/partners")
    suspend fun partners(@Query("direction") direction: String): List<PartnerDto>

    @POST("api/partners/{userId}")
    suspend fun addPartner(@Path("userId") userId: String): PartnerDto

    @PUT("api/partners/{userId}")
    suspend fun updatePartner(@Path("userId") userId: String, @Body body: PartnerUpdateRequest): PartnerDto

    @HTTP(method = "DELETE", path = "api/partners/{userId}")
    suspend fun removePartner(@Path("userId") userId: String)

    // Shared links
    @GET("api/shared-links")
    suspend fun sharedLinks(): List<SharedLinkDto>

    @POST("api/shared-links")
    suspend fun createSharedLink(@Body body: CreateSharedLinkRequest): SharedLinkDto

    @HTTP(method = "DELETE", path = "api/shared-links/{id}")
    suspend fun deleteSharedLink(@Path("id") id: String)
}
