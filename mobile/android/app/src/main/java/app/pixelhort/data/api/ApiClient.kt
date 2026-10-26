package app.pixelhort.data.api

import app.pixelhort.BuildConfig
import app.pixelhort.data.settings.SettingsStore
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.serialization.json.Json
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull
import okhttp3.Interceptor
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.logging.HttpLoggingInterceptor
import retrofit2.HttpException
import retrofit2.Retrofit
import retrofit2.converter.kotlinx.serialization.asConverterFactory
import java.io.IOException
import java.util.concurrent.TimeUnit

val PixelhortJson = Json {
    ignoreUnknownKeys = true
    explicitNulls = false
    coerceInputValues = true
    isLenient = true
    encodeDefaults = true
}

/**
 * Owns the shared [OkHttpClient] (also used by Coil and ExoPlayer) and lazily builds a
 * Retrofit instance per configured server URL.
 */
class ApiClient(private val settings: SettingsStore) {

    private val _unauthorized = MutableSharedFlow<Unit>(extraBufferCapacity = 1, onBufferOverflow = BufferOverflow.DROP_OLDEST)
    /** Emits when the server rejected our token (session expired / revoked). */
    val unauthorized: SharedFlow<Unit> = _unauthorized

    /** Adds the bearer token, but only for requests that go to the configured server. */
    private val authInterceptor = Interceptor { chain ->
        val req = chain.request()
        val s = settings.current
        val server = s.serverUrl?.toHttpUrlOrNull()
        val token = s.token
        if (server != null && token != null && req.url.host == server.host && req.url.port == server.port &&
            req.header("Authorization") == null
        ) {
            val resp = chain.proceed(req.newBuilder().header("Authorization", "Bearer $token").build())
            if (resp.code == 401 && settings.current.token == token) _unauthorized.tryEmit(Unit)
            resp
        } else {
            chain.proceed(req)
        }
    }

    private val userAgent = Interceptor { chain ->
        chain.proceed(chain.request().newBuilder().header("User-Agent", "Pixelhort-Android/${BuildConfig.VERSION_NAME}").build())
    }

    val okHttp: OkHttpClient = OkHttpClient.Builder()
        .connectTimeout(20, TimeUnit.SECONDS)
        .readTimeout(120, TimeUnit.SECONDS)
        .writeTimeout(10, TimeUnit.MINUTES)
        .addInterceptor(userAgent)
        .addInterceptor(authInterceptor)
        .apply {
            if (BuildConfig.DEBUG) {
                addInterceptor(HttpLoggingInterceptor().apply { level = HttpLoggingInterceptor.Level.BASIC })
            }
        }
        .build()

    /** Client for uploads: no logging of bodies, long timeouts. */
    val uploadHttp: OkHttpClient = okHttp.newBuilder()
        .readTimeout(10, TimeUnit.MINUTES)
        .build()

    @Volatile private var cached: Pair<String, PixelhortApi>? = null
    @Volatile private var cachedUpload: Pair<String, PixelhortApi>? = null

    val api: PixelhortApi get() = forUrl(requireServer())

    val uploadApi: PixelhortApi
        get() {
            val url = requireServer()
            cachedUpload?.let { if (it.first == url) return it.second }
            return build(url, uploadHttp).also { cachedUpload = url to it }
        }

    fun forUrl(url: String): PixelhortApi {
        cached?.let { if (it.first == url) return it.second }
        return build(url, okHttp).also { cached = url to it }
    }

    private fun build(url: String, client: OkHttpClient): PixelhortApi = Retrofit.Builder()
        .baseUrl("$url/")
        .client(client)
        .addConverterFactory(PixelhortJson.asConverterFactory("application/json".toMediaType()))
        .build()
        .create(PixelhortApi::class.java)

    private fun requireServer(): String =
        settings.current.serverUrl ?: throw IOException("Kein Server konfiguriert")

    // ---- URL helpers for media (used by Coil / ExoPlayer / DownloadManager) ----

    fun serverUrl(): String = settings.current.serverUrl.orEmpty()
    fun thumbnailUrl(assetId: String, preview: Boolean = false) =
        "${serverUrl()}/api/assets/$assetId/thumbnail?size=${if (preview) "preview" else "thumbnail"}"
    fun originalUrl(assetId: String) = "${serverUrl()}/api/assets/$assetId/original"
    fun videoUrl(assetId: String) = "${serverUrl()}/api/assets/$assetId/video"
    fun personThumbnailUrl(personId: String) = "${serverUrl()}/api/people/$personId/thumbnail"
    fun authHeader(): String? = settings.current.token?.let { "Bearer $it" }
}

/** Human readable error message for UI. */
fun Throwable.userMessage(): String = when (this) {
    is HttpException -> {
        val body = runCatching { response()?.errorBody()?.string() }.getOrNull()
        val parsed = body?.let { runCatching { PixelhortJson.decodeFromString(ApiError.serializer(), it) }.getOrNull() }
        parsed?.message?.takeIf { it.isNotBlank() } ?: "HTTP ${code()} ${message()}"
    }
    is java.net.UnknownHostException -> "Server nicht erreichbar"
    is java.net.ConnectException -> "Verbindung fehlgeschlagen"
    is java.net.SocketTimeoutException -> "Zeitüberschreitung"
    is javax.net.ssl.SSLException -> "TLS-Fehler: ${message}"
    else -> message ?: javaClass.simpleName
}

fun Throwable.httpCode(): Int? = (this as? HttpException)?.code()
