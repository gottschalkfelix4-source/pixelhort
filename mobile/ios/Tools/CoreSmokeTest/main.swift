// Linux smoke test for Pixelhort/Core (run via Tools/check-core.sh). Not part of the Xcode target.
import Foundation

func check(_ cond: Bool, _ msg: String) { if !cond { print("FAIL: \(msg)"); exit(1) } else { print("ok: \(msg)") } }

let decoder = JSONCoding.makeDecoder()
let assetJSON = """
{"id":"a1","ownerId":"u1","libraryId":null,"deviceAssetId":"local-id","deviceId":"dev","type":"IMAGE",
 "originalFileName":"IMG_0001.HEIC","originalMimeType":"image/heic","checksum":"abc","fileSize":123456,
 "fileCreatedAt":"2024-05-01T12:00:00Z","fileModifiedAt":"2024-05-01T12:00:00.123Z","localDateTime":"2024-05-01T12:00:00Z",
 "createdAt":"2024-05-01T12:00:00Z","updatedAt":"2024-05-01T12:00:00Z","width":4032,"height":3024,"duration":null,
 "isFavorite":false,"visibility":"timeline","isTrashed":false,"isOffline":false,"livePhotoVideoId":"v1","isMotionPhoto":false,
 "projectionType":null,"isPanorama":false,"isRaw":false,"thumbhash":null,"rating":null,"description":"","hasEdits":false,"duplicateId":null,
 "exif":{"make":"Apple","model":"iPhone","fNumber":1.8,"exposureTime":"1/120","iso":100,"latitude":52.5,"longitude":13.4,"dateTimeOriginal":"2024-05-01T12:00:00Z"},
 "tags":[{"id":"t","value":"x","color":null}],"people":[{"id":"p","name":"Alice"}]}
"""
let asset = try decoder.decode(AssetDto.self, from: Data(assetJSON.utf8))
check(asset.isLivePhoto && asset.exif?.iso == 100 && asset.exif?.exposureTime == "1/120", "asset decode")
check(asset.fileModifiedAt != nil, "fractional date")

let buckets = try decoder.decode([TimeBucket].self, from: Data(#"[{"timeBucket":"2024-05-01","count":42}]"#.utf8))
check(buckets.first?.count == 42, "buckets")
check(TimelineGrouping.bucketDate("2024-05-01") != nil, "bucket date")
let groups = TimelineGrouping.groupByDay([asset])
check(groups.count == 1 && groups[0].id == "2024-05-01", "group by day")

let album = try decoder.decode(AlbumDto.self, from: Data(#"{"id":"al","ownerId":"u","owner":{"id":"u","name":"A","email":"a@b.c"},"albumName":"X","description":"","albumThumbnailAssetId":null,"assetCount":10,"startDate":null,"endDate":null,"createdAt":"2024-05-01T12:00:00Z","updatedAt":"2024-05-01T12:00:00Z","shared":true,"order":"desc","albumUsers":[{"user":{"id":"u2","name":"B","email":"b@c.d"},"role":"editor"}]}"#.utf8))
check(album.albumUsers?.first?.role == .editor, "album")

let link = try decoder.decode(SharedLinkDto.self, from: Data(#"{"id":"l","key":"k","type":"INDIVIDUAL","albumId":null,"album":null,"assetIds":["a"],"description":null,"hasPassword":false,"expiresAt":null,"allowDownload":true,"allowUpload":false,"showMetadata":true,"createdAt":"2024-05-01T12:00:00Z","url":"http://x/s/k"}"#.utf8))
check(link.type == .individual, "shared link")

let people = try decoder.decode(PeopleResponse.self, from: Data(#"{"people":[{"id":"p","name":"","birthDate":"1990-01-02","isHidden":false,"faceCount":12,"thumbnailUrl":"/api/people/p/thumbnail","updatedAt":"2024-05-01T12:00:00Z"}],"total":1,"hidden":0}"#.utf8))
check(people.people.first?.displayName == nil, "people")

let explore = try decoder.decode(ExploreResponse.self, from: Data(#"{"places":[{"value":"Berlin","assetId":"a"}],"objects":[],"people":[]}"#.utf8))
check(explore.places.count == 1, "explore")

let mem = try decoder.decode([Memory].self, from: Data("[{\"id\":\"on-this-day-2019\",\"type\":\"on_this_day\",\"yearsAgo\":5,\"year\":2019,\"title\":\"5 years ago\",\"assets\":[\(assetJSON)]}]".utf8))
check(mem.first?.assets.count == 1, "memories")

let check2 = try decoder.decode(BulkUploadCheckResponse.self, from: Data(#"{"results":[{"id":"c","action":"reject","reason":"duplicate","assetId":"x"}]}"#.utf8))
check(!check2.results[0].isAccepted, "bulk check")

let session = try decoder.decode(SessionInfo.self, from: Data(#"{"id":"s","unlockedUntil":null,"hasPin":true}"#.utf8))
check(!session.isUnlocked, "session")

let login = try decoder.decode(LoginResponse.self, from: Data(#"{"accessToken":"t","user":{"id":"u","email":"a@b.c","name":"Alice","isAdmin":true,"quotaBytes":null,"usageBytes":12345,"hasPin":false,"oauthId":null,"createdAt":"2024-05-01T12:00:00Z","updatedAt":"2024-05-01T12:00:00Z"}}"#.utf8))
check(login.user.usageBytes == 12345, "login")

// Encoding omits nils
let enc = JSONCoding.makeEncoder()
let body = String(data: try enc.encode(BulkUpdateAssetsRequest(ids: ["a"], isFavorite: nil, visibility: .archive)), encoding: .utf8)!
check(body.contains("\"visibility\":\"archive\"") && !body.contains("isFavorite"), "encode partial: \(body)")
let link2 = String(data: try enc.encode(CreateSharedLinkRequest(type: .individual, assetIds: ["a"], expiresAt: Date(timeIntervalSince1970: 0))), encoding: .utf8)!
check(link2.contains("1970-01-01T00:00:00Z"), "encode date: \(link2)")

// URL building
let client = APIClient(baseURL: APIClient.normalizeServerURL("192.168.1.5:2342/")!, accessToken: "tok")
check(client.baseURL.absoluteString == "http://192.168.1.5:2342", "normalize")
check(client.thumbnailURL(assetId: "a", size: .preview).absoluteString == "http://192.168.1.5:2342/api/assets/a/thumbnail?size=preview", "thumb url")
check(APIClient.normalizeServerURL("https://photos.example.com/api/")!.absoluteString == "https://photos.example.com", "normalize api suffix")
let f = TimelineFilter(visibility: .archive, isFavorite: true)
check(client.url("/api/timeline/buckets", query: f.queryItems).absoluteString.contains("visibility=archive&isFavorite=true"), "filter query")

// Multipart
let tmp = FileManager.default.temporaryDirectory
let src = tmp.appendingPathComponent("src.bin"); try Data("HELLO".utf8).write(to: src)
let out = tmp.appendingPathComponent("body.multipart")
let writer = MultipartFormWriter(boundary: "B")
let size = try writer.write(fields: [("deviceId", "d")], files: [.init(fieldName: "assetData", fileURL: src, fileName: "a\"b.jpg", mimeType: "image/jpeg")], to: out)
let text = String(data: try Data(contentsOf: out), encoding: .utf8)!
check(Int64(text.utf8.count) == size && text.hasSuffix("--B--\r\n") && text.contains("HELLO\r\n") && text.contains("a%22b.jpg"), "multipart")

// Upload state store
let store = UploadStateStore(fileURL: tmp.appendingPathComponent("state-test.json"))
let semaphore = DispatchSemaphore(value: 0)
Task {
    await store.markUploaded("L1", checksum: "c", remoteAssetId: "r", modificationDate: Date(timeIntervalSince1970: 10))
    let backed = await store.isBackedUp("L1", modificationDate: Date(timeIntervalSince1970: 10))
    let changed = await store.isBackedUp("L1", modificationDate: Date(timeIntervalSince1970: 100))
    await store.flush()
    let reload = UploadStateStore(fileURL: tmp.appendingPathComponent("state-test.json"))
    let again = await reload.record(for: "L1")
    check(backed && !changed && again?.remoteAssetId == "r", "upload store")
    let ctx = UploadTaskContext(localIdentifier: "L/1", role: .livePhotoVideo, checksum: "c", bodyFilePath: "/x", fileName: "f", modificationDate: Date())
    check(UploadTaskContext.decode(ctx.encoded())?.role == .livePhotoVideo, "task context")
    semaphore.signal()
}
semaphore.wait()
print("ALL OK")
