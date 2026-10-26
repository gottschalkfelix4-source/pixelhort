import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

/// Typed wrappers for every endpoint of docs/API.md the iOS app uses.
extension APIClient {
    // MARK: Server

    func ping() async throws -> Bool {
        let response: PingResponse = try await get("/api/server/ping")
        return response.res == "pong"
    }

    func serverInfo() async throws -> ServerInfo {
        try await get("/api/server/info")
    }

    // MARK: Auth

    func authStatus() async throws -> AuthStatus {
        try await get("/api/auth/status")
    }

    func login(email: String, password: String, deviceName: String?) async throws -> LoginResponse {
        try await send(.post, "/api/auth/login",
                       body: LoginRequest(email: email, password: password, deviceType: "ios", deviceName: deviceName))
    }

    func logout() async throws {
        try await sendVoid(.post, "/api/auth/logout")
    }

    func oauthAuthorizeURL(redirectUri: String) async throws -> URL {
        let response: OAuthAuthorizeResponse = try await get(
            "/api/auth/oauth/authorize", query: [URLQueryItem(name: "redirectUri", value: redirectUri)])
        guard let url = URL(string: response.url) else { throw APIError.invalidResponse }
        return url
    }

    func oauthCallback(url: URL) async throws -> LoginResponse {
        try await send(.post, "/api/auth/oauth/callback", body: OAuthCallbackRequest(url: url.absoluteString))
    }

    func unlockSession(pin: String) async throws -> UnlockResponse {
        try await send(.post, "/api/auth/session/unlock", body: PinRequest(pin: pin))
    }

    func lockSession() async throws {
        try await sendVoid(.post, "/api/auth/session/lock")
    }

    func sessionInfo() async throws -> SessionInfo {
        try await get("/api/auth/session")
    }

    // MARK: Users

    func me() async throws -> UserDto {
        try await get("/api/users/me")
    }

    func updateMe(_ request: UpdateMeRequest) async throws -> UserDto {
        try await send(.put, "/api/users/me", body: request)
    }

    func setPin(_ pin: String, currentPin: String?) async throws {
        try await sendVoid(.put, "/api/users/me/pin", body: SetPinRequest(pin: pin, currentPin: currentPin))
    }

    func users() async throws -> [UserSummary] {
        try await get("/api/users")
    }

    func preferences() async throws -> UserPreferences {
        try await get("/api/users/me/preferences")
    }

    func updatePreferences(_ preferences: UserPreferences) async throws -> UserPreferences {
        try await send(.put, "/api/users/me/preferences", body: preferences)
    }

    func sessions() async throws -> [AuthSessionDto] {
        try await get("/api/sessions")
    }

    func deleteSession(id: String) async throws {
        try await sendVoid(.delete, "/api/sessions/\(id)")
    }

    func deleteOtherSessions() async throws {
        try await sendVoid(.delete, "/api/sessions")
    }

    // MARK: Assets

    func asset(id: String) async throws -> AssetDto {
        try await get("/api/assets/\(id)")
    }

    func updateAsset(id: String, _ request: UpdateAssetRequest) async throws -> AssetDto {
        try await send(.put, "/api/assets/\(id)", body: request)
    }

    func updateAssets(_ request: BulkUpdateAssetsRequest) async throws {
        try await sendVoid(.put, "/api/assets", body: request)
    }

    func deleteAssets(ids: [String], force: Bool = false) async throws {
        try await sendVoid(.delete, "/api/assets", body: DeleteAssetsRequest(ids: ids, force: force ? true : nil))
    }

    func bulkUploadCheck(_ items: [BulkUploadCheckItem]) async throws -> [BulkUploadCheckResult] {
        let response: BulkUploadCheckResponse = try await send(
            .post, "/api/assets/bulk-upload-check", body: BulkUploadCheckRequest(assets: items))
        return response.results
    }

    func existingAssets(deviceId: String, deviceAssetIds: [String]) async throws -> [String] {
        let response: AssetExistResponse = try await send(
            .post, "/api/assets/exist", body: AssetExistRequest(deviceId: deviceId, deviceAssetIds: deviceAssetIds))
        return response.existingIds
    }

    func castURL(assetId: String) async throws -> CastURLResponse {
        try await send(.post, "/api/assets/\(assetId)/cast-url")
    }

    func assetStatistics() async throws -> AssetStatistics {
        try await get("/api/assets/statistics")
    }

    func thumbnailPath(assetId: String) -> String { "/api/assets/\(assetId)/thumbnail" }
    func originalPath(assetId: String) -> String { "/api/assets/\(assetId)/original" }
    func videoPath(assetId: String) -> String { "/api/assets/\(assetId)/video" }

    func thumbnailURL(assetId: String, size: ThumbnailSize) -> URL {
        url(thumbnailPath(assetId: assetId), query: [URLQueryItem(name: "size", value: size.rawValue)])
    }

    func videoURL(assetId: String) -> URL {
        url(videoPath(assetId: assetId))
    }

    func originalURL(assetId: String) -> URL {
        url(originalPath(assetId: assetId))
    }

    func thumbnailData(assetId: String, size: ThumbnailSize) async throws -> Data {
        try await data(thumbnailPath(assetId: assetId), query: [URLQueryItem(name: "size", value: size.rawValue)])
    }

    // MARK: Trash

    func trash() async throws -> [AssetDto] {
        try await get("/api/trash")
    }

    func restoreTrash(ids: [String]?) async throws {
        try await sendVoid(.post, "/api/trash/restore", body: TrashRestoreRequest(ids: ids))
    }

    func emptyTrash() async throws {
        try await sendVoid(.post, "/api/trash/empty")
    }

    // MARK: Timeline

    func timelineBuckets(_ filter: TimelineFilter) async throws -> [TimeBucket] {
        try await get("/api/timeline/buckets", query: filter.queryItems)
    }

    func timelineBucket(_ timeBucket: String, filter: TimelineFilter) async throws -> [AssetDto] {
        try await get("/api/timeline/bucket",
                      query: [URLQueryItem(name: "timeBucket", value: timeBucket)] + filter.queryItems)
    }

    func syncDelta(updatedAfter: String?, limit: Int = 1000) async throws -> SyncDeltaResponse {
        var query = [URLQueryItem(name: "limit", value: String(limit))]
        if let updatedAfter { query.append(URLQueryItem(name: "updatedAfter", value: updatedAfter)) }
        return try await get("/api/sync/delta", query: query)
    }

    // MARK: Albums

    func albums(shared: Bool? = nil) async throws -> [AlbumDto] {
        var query: [URLQueryItem] = []
        if let shared { query.append(URLQueryItem(name: "shared", value: shared ? "true" : "false")) }
        return try await get("/api/albums", query: query)
    }

    func album(id: String) async throws -> AlbumDto {
        try await get("/api/albums/\(id)")
    }

    func createAlbum(_ request: CreateAlbumRequest) async throws -> AlbumDto {
        try await send(.post, "/api/albums", body: request)
    }

    func updateAlbum(id: String, _ request: UpdateAlbumRequest) async throws -> AlbumDto {
        try await send(.patch, "/api/albums/\(id)", body: request)
    }

    func deleteAlbum(id: String) async throws {
        try await sendVoid(.delete, "/api/albums/\(id)")
    }

    @discardableResult
    func addAssets(toAlbum id: String, assetIds: [String]) async throws -> [AlbumAssetResult] {
        try await send(.put, "/api/albums/\(id)/assets", body: IdsRequest(ids: assetIds))
    }

    @discardableResult
    func removeAssets(fromAlbum id: String, assetIds: [String]) async throws -> [AlbumAssetResult] {
        try await send(.delete, "/api/albums/\(id)/assets", body: IdsRequest(ids: assetIds))
    }

    func setAlbumUsers(id: String, users: [AlbumUserRequest]) async throws -> AlbumDto {
        try await send(.put, "/api/albums/\(id)/users", body: AlbumUsersRequest(albumUsers: users))
    }

    /// `userId` may be `"me"` to leave a shared album.
    func removeAlbumUser(albumId: String, userId: String) async throws {
        try await sendVoid(.delete, "/api/albums/\(albumId)/users/\(userId)")
    }

    // MARK: Map & memories

    func mapMarkers(isFavorite: Bool? = nil, withPartners: Bool? = nil) async throws -> [MapMarkerDto] {
        var query: [URLQueryItem] = []
        if let isFavorite { query.append(URLQueryItem(name: "isFavorite", value: isFavorite ? "true" : "false")) }
        if let withPartners { query.append(URLQueryItem(name: "withPartners", value: withPartners ? "true" : "false")) }
        return try await get("/api/map/markers", query: query)
    }

    func memories(date: Date = Date()) async throws -> [Memory] {
        try await get("/api/memories", query: [URLQueryItem(name: "date", value: JSONCoding.dayString(date))])
    }

    // MARK: People

    func people(withHidden: Bool = false) async throws -> PeopleResponse {
        try await get("/api/people", query: [URLQueryItem(name: "withHidden", value: withHidden ? "true" : "false")])
    }

    func person(id: String) async throws -> PersonDto {
        try await get("/api/people/\(id)")
    }

    func updatePerson(id: String, _ request: UpdatePersonRequest) async throws -> PersonDto {
        try await send(.put, "/api/people/\(id)", body: request)
    }

    func personAssets(id: String) async throws -> [AssetDto] {
        try await get("/api/people/\(id)/assets")
    }

    func personThumbnailPath(id: String) -> String { "/api/people/\(id)/thumbnail" }

    // MARK: Search

    func smartSearch(query: String, page: Int = 1, size: Int = 100) async throws -> SearchResponse {
        try await send(.post, "/api/search/smart", body: SmartSearchRequest(query: query, page: page, size: size))
    }

    func metadataSearch(_ request: MetadataSearchRequest) async throws -> SearchResponse {
        try await send(.post, "/api/search/metadata", body: request)
    }

    func explore() async throws -> ExploreResponse {
        try await get("/api/search/explore")
    }

    // MARK: Shared links

    func sharedLinks() async throws -> [SharedLinkDto] {
        try await get("/api/shared-links")
    }

    func createSharedLink(_ request: CreateSharedLinkRequest) async throws -> SharedLinkDto {
        try await send(.post, "/api/shared-links", body: request)
    }

    func deleteSharedLink(id: String) async throws {
        try await sendVoid(.delete, "/api/shared-links/\(id)")
    }

    // MARK: Partners

    func partners(direction: PartnerDirection) async throws -> [Partner] {
        try await get("/api/partners", query: [URLQueryItem(name: "direction", value: direction.rawValue)])
    }

    func addPartner(userId: String) async throws -> Partner {
        try await send(.post, "/api/partners/\(userId)")
    }

    func updatePartner(userId: String, inTimeline: Bool) async throws -> Partner {
        try await send(.put, "/api/partners/\(userId)", body: PartnerUpdateRequest(inTimeline: inTimeline))
    }

    func removePartner(userId: String) async throws {
        try await sendVoid(.delete, "/api/partners/\(userId)")
    }

    // MARK: Upload (foreground request builder; the background uploader streams from file)

    /// Builds the `POST /api/assets` request for a multipart body that was written to disk.
    func uploadRequest(contentType: String, checksum: String?, allowsCellular: Bool) -> URLRequest {
        var request = URLRequest(url: url("/api/assets"))
        request.httpMethod = HTTPMethod.post.rawValue
        request.setValue(contentType, forHTTPHeaderField: "Content-Type")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        if let checksum { request.setValue(checksum, forHTTPHeaderField: "x-checksum") }
        request.allowsCellularAccess = allowsCellular
        authorize(&request)
        return request
    }
}
