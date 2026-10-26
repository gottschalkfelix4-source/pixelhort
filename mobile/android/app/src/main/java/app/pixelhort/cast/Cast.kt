package app.pixelhort.cast

import android.content.Context
import android.net.Uri
import android.util.Log
import com.google.android.gms.cast.CastMediaControlIntent
import com.google.android.gms.cast.MediaInfo
import com.google.android.gms.cast.MediaLoadRequestData
import com.google.android.gms.cast.MediaMetadata
import com.google.android.gms.cast.framework.CastContext
import com.google.android.gms.cast.framework.CastOptions
import com.google.android.gms.cast.framework.OptionsProvider
import com.google.android.gms.cast.framework.SessionProvider
import com.google.android.gms.cast.framework.media.CastMediaOptions
import com.google.android.gms.common.ConnectionResult
import com.google.android.gms.common.GoogleApiAvailability
import com.google.android.gms.common.images.WebImage
import app.pixelhort.data.api.AssetDto
import app.pixelhort.data.api.CastUrlResponse

/** Google Cast configuration: uses the Default Media Receiver. */
class CastOptionsProvider : OptionsProvider {
    override fun getCastOptions(context: Context): CastOptions = CastOptions.Builder()
        .setReceiverApplicationId(CastMediaControlIntent.DEFAULT_MEDIA_RECEIVER_APPLICATION_ID)
        .setCastMediaOptions(CastMediaOptions.Builder().setNotificationOptions(null).build())
        .build()

    override fun getAdditionalSessionProviders(context: Context): List<SessionProvider>? = null
}

object CastHelper {
    private const val TAG = "CastHelper"

    fun isAvailable(context: Context): Boolean =
        GoogleApiAvailability.getInstance().isGooglePlayServicesAvailable(context) == ConnectionResult.SUCCESS

    @Suppress("DEPRECATION")
    fun context(context: Context): CastContext? = if (!isAvailable(context)) null else try {
        CastContext.getSharedInstance(context)
    } catch (e: Exception) {
        Log.w(TAG, "Cast unavailable", e)
        null
    }

    fun isConnected(context: Context): Boolean =
        context(context)?.sessionManager?.currentCastSession?.isConnected == true

    /** Loads the asset on the connected receiver. Returns false if no cast session is active. */
    fun load(context: Context, asset: AssetDto, cast: CastUrlResponse): Boolean {
        val session = context(context)?.sessionManager?.currentCastSession ?: return false
        val client = session.remoteMediaClient ?: return false
        val isVideo = asset.isVideo
        val meta = MediaMetadata(if (isVideo) MediaMetadata.MEDIA_TYPE_MOVIE else MediaMetadata.MEDIA_TYPE_PHOTO).apply {
            putString(MediaMetadata.KEY_TITLE, asset.originalFileName)
            cast.imageUrl?.let { addImage(WebImage(Uri.parse(it))) }
        }
        val url = if (isVideo) cast.url else (cast.imageUrl ?: cast.url)
        val mime = if (isVideo) (cast.mimeType ?: "video/mp4") else "image/jpeg"
        val info = MediaInfo.Builder(url)
            .setStreamType(MediaInfo.STREAM_TYPE_BUFFERED)
            .setContentType(mime)
            .setMetadata(meta)
            .build()
        client.load(MediaLoadRequestData.Builder().setMediaInfo(info).setAutoplay(true).build())
        return true
    }
}
