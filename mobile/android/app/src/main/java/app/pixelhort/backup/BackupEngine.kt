package app.pixelhort.backup

import android.content.ContentResolver
import android.net.Uri
import android.util.Log
import app.pixelhort.data.api.ApiClient
import app.pixelhort.data.api.BulkCheckItem
import app.pixelhort.data.api.BulkCheckRequest
import app.pixelhort.data.api.UploadResponse
import app.pixelhort.data.api.userMessage
import app.pixelhort.data.db.BackupDao
import app.pixelhort.data.db.BackupItem
import app.pixelhort.data.db.BackupStatus
import app.pixelhort.data.settings.BackupOptions
import app.pixelhort.data.settings.SettingsStore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import okhttp3.MediaType
import okhttp3.MediaType.Companion.toMediaTypeOrNull
import okhttp3.MultipartBody
import okhttp3.RequestBody
import okhttp3.RequestBody.Companion.toRequestBody
import okio.BufferedSink
import okio.source
import retrofit2.HttpException
import java.io.FileNotFoundException
import java.io.IOException
import java.time.Instant
import kotlin.coroutines.coroutineContext

data class BackupProgress(
    val done: Int,
    val total: Int,
    val currentFile: String?,
    val uploaded: Int,
    val skipped: Int,
    val failed: Int,
)

data class BackupResult(
    val uploaded: Int,
    val skipped: Int,
    val failed: Int,
    val total: Int,
    /** true if the run was aborted because the server/network is unavailable → retry later. */
    val retry: Boolean = false,
    val error: String? = null,
)

/**
 * The automatic backup core: index selected device albums into Room, hash new items (SHA-1,
 * streamed), ask the server which ones it already has (bulk-upload-check) and upload the rest.
 */
class BackupEngine(
    private val resolver: ContentResolver,
    private val scanner: MediaStoreScanner,
    private val dao: BackupDao,
    private val settings: SettingsStore,
    private val api: ApiClient,
) {
    private val mutex = Mutex()

    /** Synchronises the Room index with the MediaStore content of the selected albums. */
    suspend fun syncIndex(options: BackupOptions = settings.current.backup) = withContext(Dispatchers.IO) {
        if (!scanner.hasMediaPermission() || options.selectedBuckets.isEmpty()) return@withContext
        val local = scanner.media(options.selectedBuckets, includeVideos = true)
        val byId = local.associateBy { it.id }

        // new rows
        local.chunked(500).forEach { chunk ->
            dao.insertIgnore(chunk.map { it.toItem() })
        }
        // rows that moved between albums / were changed
        local.chunked(500).forEach { chunk ->
            val existing = dao.byIds(chunk.map { it.id })
            for (row in existing) {
                val m = byId[row.localId] ?: continue
                if (row.bucketId != m.bucketId) dao.moveToBucket(row.localId, m.bucketId, m.bucketName)
            }
        }
        // rows whose local file no longer exists
        for (bucket in options.selectedBuckets) {
            val gone = dao.idsInBucket(bucket).filter { it !in byId }
            gone.chunked(500).forEach { dao.deleteIds(it) }
        }
    }

    suspend fun run(onProgress: suspend (BackupProgress) -> Unit): BackupResult = mutex.withLock {
        withContext(Dispatchers.IO) { runLocked(onProgress) }
    }

    private suspend fun runLocked(onProgress: suspend (BackupProgress) -> Unit): BackupResult {
        val s = settings.current
        if (!s.isLoggedIn) return BackupResult(0, 0, 0, 0, error = "not logged in")
        val options = s.backup
        if (options.selectedBuckets.isEmpty()) return BackupResult(0, 0, 0, 0)
        if (!scanner.hasMediaPermission()) return BackupResult(0, 0, 0, 0, error = "no media permission")

        syncIndex(options)
        val deviceId = settings.deviceId()
        val pending = dao.pending(options.selectedBuckets.toList(), options.includeVideos, MAX_ATTEMPTS)
        val total = pending.size
        var done = 0
        var uploaded = 0
        var skipped = 0
        var failed = 0
        var consecutiveNetworkErrors = 0
        onProgress(BackupProgress(0, total, null, 0, 0, 0))

        for (chunk in pending.chunked(CHUNK)) {
            coroutineContext.ensureActive()

            // 1) hash
            val hashed = ArrayList<BackupItem>(chunk.size)
            for (item in chunk) {
                coroutineContext.ensureActive()
                onProgress(BackupProgress(done, total, item.displayName, uploaded, skipped, failed))
                val key = "${item.dateModified}:${item.size}"
                if (item.checksum != null && item.checksumFor == key) {
                    hashed += item
                    continue
                }
                try {
                    val uri = scanner.originalUri(scanner.uriFor(item.localId, item.isVideo))
                    val input = resolver.openInputStream(uri) ?: throw FileNotFoundException(item.displayName)
                    val sha = Hashing.sha1Hex(input)
                    val updated = item.copy(checksum = sha, checksumFor = key, updatedAt = System.currentTimeMillis())
                    dao.update(updated)
                    hashed += updated
                } catch (e: Exception) {
                    if (e is kotlinx.coroutines.CancellationException) throw e
                    Log.w(TAG, "hash failed for ${item.displayName}", e)
                    markError(item, "Lesen fehlgeschlagen: ${e.message}")
                    failed++; done++
                }
            }
            if (hashed.isEmpty()) continue

            // 2) ask the server which ones it already has
            val results = try {
                api.api.bulkUploadCheck(
                    BulkCheckRequest(hashed.map { BulkCheckItem(it.localId.toString(), it.checksum!!) })
                ).results.associateBy { it.id }
            } catch (e: Exception) {
                if (e is kotlinx.coroutines.CancellationException) throw e
                return abort(e, uploaded, skipped, failed, total)
            }

            // 3) upload accepted ones
            for (item in hashed) {
                coroutineContext.ensureActive()
                val r = results[item.localId.toString()]
                if (r != null && r.action == "reject") {
                    dao.update(
                        item.copy(
                            status = BackupStatus.DUPLICATE, remoteAssetId = r.assetId, error = null,
                            updatedAt = System.currentTimeMillis(),
                        )
                    )
                    skipped++; done++
                    onProgress(BackupProgress(done, total, item.displayName, uploaded, skipped, failed))
                    continue
                }
                onProgress(BackupProgress(done, total, item.displayName, uploaded, skipped, failed))
                try {
                    val res = upload(item, deviceId)
                    dao.update(
                        item.copy(
                            status = if (res.status == "duplicate") BackupStatus.DUPLICATE else BackupStatus.UPLOADED,
                            remoteAssetId = res.id, error = null, updatedAt = System.currentTimeMillis(),
                        )
                    )
                    if (res.status == "duplicate") skipped++ else uploaded++
                    consecutiveNetworkErrors = 0
                } catch (e: Exception) {
                    if (e is kotlinx.coroutines.CancellationException) throw e
                    Log.w(TAG, "upload failed for ${item.displayName}", e)
                    when {
                        e is HttpException && e.code() == 401 -> return abort(e, uploaded, skipped, failed, total)
                        e is HttpException && e.code() >= 500 || e is IOException && e !is FileNotFoundException -> {
                            markError(item, e.userMessage())
                            failed++
                            if (++consecutiveNetworkErrors >= 3) return abort(e, uploaded, skipped, failed, total)
                        }
                        else -> {
                            markError(item, e.userMessage())
                            failed++
                        }
                    }
                }
                done++
                onProgress(BackupProgress(done, total, item.displayName, uploaded, skipped, failed))
            }
        }
        settings.setLastBackupAt(System.currentTimeMillis())
        return BackupResult(uploaded, skipped, failed, total)
    }

    private fun abort(e: Exception, uploaded: Int, skipped: Int, failed: Int, total: Int) =
        BackupResult(uploaded, skipped, failed, total, retry = true, error = e.userMessage())

    private suspend fun markError(item: BackupItem, msg: String) {
        dao.update(
            item.copy(
                status = BackupStatus.ERROR, error = msg, attempts = item.attempts + 1,
                updatedAt = System.currentTimeMillis(),
            )
        )
    }

    private suspend fun upload(item: BackupItem, deviceId: String): UploadResponse {
        val uri = scanner.originalUri(scanner.uriFor(item.localId, item.isVideo))
        val body = ContentUriRequestBody(resolver, uri, item.mimeType.toMediaTypeOrNull(), item.size)
        val part = MultipartBody.Part.createFormData("assetData", item.displayName, body)
        val created = Instant.ofEpochMilli(if (item.dateTaken > 0) item.dateTaken else item.dateModified * 1000).toString()
        val modified = Instant.ofEpochMilli(item.dateModified * 1000).toString()
        val text = "text/plain".toMediaTypeOrNull()
        return api.uploadApi.upload(
            checksum = item.checksum,
            assetData = part,
            deviceAssetId = item.localId.toString().toRequestBody(text),
            deviceId = deviceId.toRequestBody(text),
            fileCreatedAt = created.toRequestBody(text),
            fileModifiedAt = modified.toRequestBody(text),
            isFavorite = "false".toRequestBody(text),
        )
    }

    private fun LocalMedia.toItem() = BackupItem(
        localId = id,
        bucketId = bucketId,
        bucketName = bucketName,
        displayName = displayName,
        mimeType = mimeType,
        isVideo = isVideo,
        size = size,
        dateTaken = if (dateTaken > 0) dateTaken else dateAdded * 1000,
        dateModified = dateModified,
    )

    companion object {
        private const val TAG = "BackupEngine"
        const val CHUNK = 50
        const val MAX_ATTEMPTS = 5
    }
}

/** Streams a content:// URI as request body without loading it into memory. */
class ContentUriRequestBody(
    private val resolver: ContentResolver,
    private val uri: Uri,
    private val type: MediaType?,
    private val length: Long,
) : RequestBody() {
    override fun contentType(): MediaType? = type
    // MediaStore SIZE can be stale; stream with chunked encoding to never send a wrong length.
    override fun contentLength(): Long = -1
    override fun writeTo(sink: BufferedSink) {
        val input = resolver.openInputStream(uri) ?: throw FileNotFoundException(uri.toString())
        input.source().use { sink.writeAll(it) }
    }
}
