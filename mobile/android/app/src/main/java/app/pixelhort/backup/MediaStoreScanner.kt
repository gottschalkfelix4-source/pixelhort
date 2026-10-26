package app.pixelhort.backup

import android.Manifest
import android.content.ContentUris
import android.content.Context
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.MediaStore
import androidx.core.content.ContextCompat

/** A device album (MediaStore bucket). */
data class DeviceAlbum(val bucketId: String, val name: String, val count: Int, val coverUri: Uri?)

data class LocalMedia(
    val id: Long,
    val uri: Uri,
    val bucketId: String,
    val bucketName: String,
    val displayName: String,
    val mimeType: String,
    val size: Long,
    val dateTaken: Long,
    val dateAdded: Long,
    val dateModified: Long,
    val isVideo: Boolean,
)

class MediaStoreScanner(private val context: Context) {

    fun hasMediaPermission(): Boolean = mediaPermissions().any {
        ContextCompat.checkSelfPermission(context, it) == PackageManager.PERMISSION_GRANTED
    }

    fun hasLocationPermission(): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.Q ||
            ContextCompat.checkSelfPermission(context, Manifest.permission.ACCESS_MEDIA_LOCATION) == PackageManager.PERMISSION_GRANTED

    /** All device albums containing images and/or videos. */
    fun albums(includeVideos: Boolean = true): List<DeviceAlbum> {
        val map = LinkedHashMap<String, DeviceAlbum>()
        for (video in if (includeVideos) listOf(false, true) else listOf(false)) {
            val base = collection(video)
            val projection = arrayOf(
                MediaStore.MediaColumns._ID,
                MediaStore.MediaColumns.BUCKET_ID,
                MediaStore.MediaColumns.BUCKET_DISPLAY_NAME,
            )
            context.contentResolver.query(base, projection, null, null, "${MediaStore.MediaColumns.DATE_MODIFIED} DESC")?.use { c ->
                val idCol = c.getColumnIndexOrThrow(MediaStore.MediaColumns._ID)
                val bCol = c.getColumnIndexOrThrow(MediaStore.MediaColumns.BUCKET_ID)
                val nCol = c.getColumnIndexOrThrow(MediaStore.MediaColumns.BUCKET_DISPLAY_NAME)
                while (c.moveToNext()) {
                    val bucket = c.getString(bCol) ?: continue
                    val existing = map[bucket]
                    if (existing == null) {
                        map[bucket] = DeviceAlbum(
                            bucket, c.getString(nCol) ?: "?", 1,
                            ContentUris.withAppendedId(base, c.getLong(idCol)),
                        )
                    } else {
                        map[bucket] = existing.copy(count = existing.count + 1)
                    }
                }
            }
        }
        return map.values.sortedByDescending { it.count }
    }

    /** All media in the given buckets. */
    fun media(buckets: Set<String>, includeVideos: Boolean): List<LocalMedia> {
        if (buckets.isEmpty()) return emptyList()
        val out = ArrayList<LocalMedia>()
        for (video in if (includeVideos) listOf(false, true) else listOf(false)) {
            val base = collection(video)
            val projection = arrayOf(
                MediaStore.MediaColumns._ID,
                MediaStore.MediaColumns.BUCKET_ID,
                MediaStore.MediaColumns.BUCKET_DISPLAY_NAME,
                MediaStore.MediaColumns.DISPLAY_NAME,
                MediaStore.MediaColumns.MIME_TYPE,
                MediaStore.MediaColumns.SIZE,
                MediaStore.MediaColumns.DATE_ADDED,
                MediaStore.MediaColumns.DATE_MODIFIED,
                DATE_TAKEN,
            )
            val sel = "${MediaStore.MediaColumns.BUCKET_ID} IN (${buckets.joinToString(",") { "?" }})"
            context.contentResolver.query(base, projection, sel, buckets.toTypedArray(), null)?.use { c ->
                val idCol = c.getColumnIndexOrThrow(MediaStore.MediaColumns._ID)
                val bCol = c.getColumnIndexOrThrow(MediaStore.MediaColumns.BUCKET_ID)
                val bnCol = c.getColumnIndexOrThrow(MediaStore.MediaColumns.BUCKET_DISPLAY_NAME)
                val nameCol = c.getColumnIndexOrThrow(MediaStore.MediaColumns.DISPLAY_NAME)
                val mimeCol = c.getColumnIndexOrThrow(MediaStore.MediaColumns.MIME_TYPE)
                val sizeCol = c.getColumnIndexOrThrow(MediaStore.MediaColumns.SIZE)
                val addedCol = c.getColumnIndexOrThrow(MediaStore.MediaColumns.DATE_ADDED)
                val modCol = c.getColumnIndexOrThrow(MediaStore.MediaColumns.DATE_MODIFIED)
                val takenCol = c.getColumnIndex(DATE_TAKEN)
                while (c.moveToNext()) {
                    val id = c.getLong(idCol)
                    out += LocalMedia(
                        id = id,
                        uri = ContentUris.withAppendedId(base, id),
                        bucketId = c.getString(bCol) ?: "",
                        bucketName = c.getString(bnCol) ?: "",
                        displayName = c.getString(nameCol) ?: "$id",
                        mimeType = c.getString(mimeCol) ?: if (video) "video/mp4" else "image/jpeg",
                        size = c.getLong(sizeCol),
                        dateTaken = if (takenCol >= 0) c.getLong(takenCol) else 0L,
                        dateAdded = c.getLong(addedCol),
                        dateModified = c.getLong(modCol),
                        isVideo = video,
                    )
                }
            }
        }
        return out
    }

    fun uriFor(id: Long, isVideo: Boolean): Uri = ContentUris.withAppendedId(collection(isVideo), id)

    /**
     * URI to read the unmodified original. With ACCESS_MEDIA_LOCATION granted this keeps
     * GPS EXIF data that Android would otherwise redact.
     */
    fun originalUri(uri: Uri): Uri =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && hasLocationPermission()) {
            runCatching { MediaStore.setRequireOriginal(uri) }.getOrDefault(uri)
        } else uri

    companion object {
        @Suppress("InlinedApi")
        private const val DATE_TAKEN = MediaStore.MediaColumns.DATE_TAKEN

        fun collection(video: Boolean): Uri =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                if (video) MediaStore.Video.Media.getContentUri(MediaStore.VOLUME_EXTERNAL)
                else MediaStore.Images.Media.getContentUri(MediaStore.VOLUME_EXTERNAL)
            } else {
                if (video) MediaStore.Video.Media.EXTERNAL_CONTENT_URI else MediaStore.Images.Media.EXTERNAL_CONTENT_URI
            }

        fun mediaPermissions(): Array<String> = when {
            Build.VERSION.SDK_INT >= 34 -> arrayOf(
                Manifest.permission.READ_MEDIA_IMAGES,
                Manifest.permission.READ_MEDIA_VIDEO,
                Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED,
            )
            Build.VERSION.SDK_INT >= 33 -> arrayOf(Manifest.permission.READ_MEDIA_IMAGES, Manifest.permission.READ_MEDIA_VIDEO)
            else -> arrayOf(Manifest.permission.READ_EXTERNAL_STORAGE)
        }

        /** Permissions to request at once (media + location metadata + notifications). */
        fun backupPermissions(): Array<String> {
            val list = mediaPermissions().toMutableList()
            if (Build.VERSION.SDK_INT >= 29) list += Manifest.permission.ACCESS_MEDIA_LOCATION
            if (Build.VERSION.SDK_INT >= 33) list += Manifest.permission.POST_NOTIFICATIONS
            return list.toTypedArray()
        }
    }
}
