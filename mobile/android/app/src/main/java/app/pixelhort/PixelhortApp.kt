package app.pixelhort

import android.app.Application
import android.content.Context
import app.pixelhort.backup.BackupEngine
import app.pixelhort.data.AuthRepository
import app.pixelhort.backup.BackupNotifications
import app.pixelhort.backup.BackupScheduler
import app.pixelhort.backup.MediaStoreScanner
import app.pixelhort.data.api.ApiClient
import app.pixelhort.data.api.AssetDto
import app.pixelhort.data.api.PixelhortJson
import app.pixelhort.data.db.BackupDatabase
import app.pixelhort.data.settings.SettingsStore
import coil3.ImageLoader
import coil3.PlatformContext
import coil3.SingletonImageLoader
import coil3.disk.DiskCache
import coil3.disk.directory
import coil3.memory.MemoryCache
import coil3.network.okhttp.OkHttpNetworkFetcherFactory
import coil3.request.crossfade
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import org.osmdroid.config.Configuration

/** Hand-written dependency container (the app is small enough to not need Hilt). */
class AppContainer(context: Context) {
    val appScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    val json = PixelhortJson
    val settings = SettingsStore(context, appScope, json)
    val apiClient = ApiClient(settings)
    val db = BackupDatabase.create(context)
    val scanner = MediaStoreScanner(context)
    val backupEngine = BackupEngine(context.contentResolver, scanner, db.backupDao(), settings, apiClient)
    val auth = AuthRepository(context, settings, apiClient, db.backupDao())

    /** Assets handed to the full screen viewer (avoids serialising lists into nav routes). */
    @Volatile var viewerAssets: List<AssetDto> = emptyList()

    /** Bumped whenever assets were changed outside a grid (viewer actions) → grids refresh on resume. */
    val dataVersion = kotlinx.coroutines.flow.MutableStateFlow(0)
    fun markDataChanged() { dataVersion.value++ }
}

class PixelhortApp : Application(), SingletonImageLoader.Factory {
    lateinit var container: AppContainer
        private set

    override fun onCreate() {
        super.onCreate()
        container = AppContainer(this)
        BackupNotifications.ensureChannels(this)
        Configuration.getInstance().apply {
            userAgentValue = packageName
            osmdroidBasePath = cacheDir.resolve("osmdroid")
            osmdroidTileCache = cacheDir.resolve("osmdroid/tiles")
        }
        // Keep background backup scheduling in sync with settings (also re-arms content
        // triggers, which do not survive reboots).
        container.appScope.launch {
            container.settings.state
                .map { Triple(it.backup, it.isLoggedIn, it.serverUrl) }
                .distinctUntilChanged()
                .collect { (opts, loggedIn, _) -> BackupScheduler.apply(this@PixelhortApp, opts, loggedIn) }
        }
    }

    override fun newImageLoader(context: PlatformContext): ImageLoader = ImageLoader.Builder(context)
        .components {
            add(OkHttpNetworkFetcherFactory(callFactory = { container.apiClient.okHttp }))
            add(coil3.video.VideoFrameDecoder.Factory()) // covers of local video-only albums
        }
        .memoryCache { MemoryCache.Builder().maxSizePercent(context, 0.25).build() }
        .diskCache { DiskCache.Builder().directory(context.cacheDir.resolve("thumbs")).maxSizeBytes(512L * 1024 * 1024).build() }
        .crossfade(true)
        .build()
}

val Context.container: AppContainer get() = (applicationContext as PixelhortApp).container
