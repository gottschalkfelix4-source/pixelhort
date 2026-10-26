package app.pixelhort.data.settings

import android.content.Context
import android.content.SharedPreferences
import android.util.Log
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.core.stringSetPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import app.pixelhort.data.api.UserDto
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.Json
import java.util.UUID

private val Context.dataStore: DataStore<Preferences> by preferencesDataStore(name = "pixelhort_settings")

data class BackupOptions(
    val enabled: Boolean = false,
    val wifiOnly: Boolean = true,
    val chargingOnly: Boolean = false,
    val includeVideos: Boolean = true,
    val selectedBuckets: Set<String> = emptySet(),
)

data class AppSettings(
    val serverUrl: String? = null,
    val token: String? = null,
    val user: UserDto? = null,
    val deviceId: String = "",
    val backup: BackupOptions = BackupOptions(),
    val biometricLock: Boolean = false,
    val lastBackupAt: Long = 0L,
) {
    val isLoggedIn: Boolean get() = !serverUrl.isNullOrBlank() && !token.isNullOrBlank()
}

/**
 * Persists server URL, backup options etc. in DataStore and the access token in
 * EncryptedSharedPreferences. A synchronous snapshot ([current]) is kept for OkHttp
 * interceptors and workers.
 */
class SettingsStore(private val context: Context, scope: CoroutineScope, private val json: Json) {

    private object Keys {
        val SERVER = stringPreferencesKey("server_url")
        val USER = stringPreferencesKey("user_json")
        val DEVICE_ID = stringPreferencesKey("device_id")
        val BACKUP_ENABLED = booleanPreferencesKey("backup_enabled")
        val WIFI_ONLY = booleanPreferencesKey("backup_wifi_only")
        val CHARGING_ONLY = booleanPreferencesKey("backup_charging_only")
        val VIDEOS = booleanPreferencesKey("backup_videos")
        val BUCKETS = stringSetPreferencesKey("backup_buckets")
        val BIOMETRIC = booleanPreferencesKey("biometric_lock")
        val LAST_BACKUP = longPreferencesKey("last_backup_at")
    }

    private val securePrefs: SharedPreferences = createSecurePrefs()

    private val _state = MutableStateFlow(runBlocking { read(context.dataStore.data.first()) })
    val state: StateFlow<AppSettings> = _state.asStateFlow()

    val current: AppSettings get() = _state.value

    init {
        scope.launch {
            ensureDeviceId()
            context.dataStore.data.collect { prefs -> _state.value = read(prefs) }
        }
    }

    private fun read(p: Preferences): AppSettings = AppSettings(
        serverUrl = p[Keys.SERVER],
        token = securePrefs.getString(TOKEN_KEY, null),
        user = p[Keys.USER]?.let { runCatching { json.decodeFromString(UserDto.serializer(), it) }.getOrNull() },
        deviceId = p[Keys.DEVICE_ID] ?: "",
        backup = BackupOptions(
            enabled = p[Keys.BACKUP_ENABLED] ?: false,
            wifiOnly = p[Keys.WIFI_ONLY] ?: true,
            chargingOnly = p[Keys.CHARGING_ONLY] ?: false,
            includeVideos = p[Keys.VIDEOS] ?: true,
            selectedBuckets = p[Keys.BUCKETS] ?: emptySet(),
        ),
        biometricLock = p[Keys.BIOMETRIC] ?: false,
        lastBackupAt = p[Keys.LAST_BACKUP] ?: 0L,
    )

    private suspend fun ensureDeviceId() {
        context.dataStore.edit { if (it[Keys.DEVICE_ID].isNullOrBlank()) it[Keys.DEVICE_ID] = UUID.randomUUID().toString() }
    }

    suspend fun deviceId(): String {
        ensureDeviceId()
        return context.dataStore.data.first()[Keys.DEVICE_ID]!!
    }

    suspend fun setServerUrl(url: String) {
        context.dataStore.edit { it[Keys.SERVER] = url }
    }

    suspend fun setSession(token: String, user: UserDto) {
        securePrefs.edit().putString(TOKEN_KEY, token).apply()
        context.dataStore.edit { it[Keys.USER] = json.encodeToString(UserDto.serializer(), user) }
        _state.update { it.copy(token = token, user = user) }
    }

    suspend fun setUser(user: UserDto) {
        context.dataStore.edit { it[Keys.USER] = json.encodeToString(UserDto.serializer(), user) }
    }

    suspend fun clearSession() {
        securePrefs.edit().remove(TOKEN_KEY).apply()
        context.dataStore.edit { it.remove(Keys.USER) }
        _state.update { it.copy(token = null, user = null) }
    }

    suspend fun updateBackup(transform: (BackupOptions) -> BackupOptions) {
        context.dataStore.edit { p ->
            val cur = read(p).backup
            val n = transform(cur)
            p[Keys.BACKUP_ENABLED] = n.enabled
            p[Keys.WIFI_ONLY] = n.wifiOnly
            p[Keys.CHARGING_ONLY] = n.chargingOnly
            p[Keys.VIDEOS] = n.includeVideos
            p[Keys.BUCKETS] = n.selectedBuckets
        }
    }

    suspend fun setBiometricLock(enabled: Boolean) {
        context.dataStore.edit { it[Keys.BIOMETRIC] = enabled }
    }

    suspend fun setLastBackupAt(ts: Long) {
        context.dataStore.edit { it[Keys.LAST_BACKUP] = ts }
    }

    private fun createSecurePrefs(): SharedPreferences = try {
        val key = MasterKey.Builder(context).setKeyScheme(MasterKey.KeyScheme.AES256_GCM).build()
        @Suppress("DEPRECATION")
        EncryptedSharedPreferences.create(
            context,
            "pixelhort_secure",
            key,
            EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
            EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
        )
    } catch (e: Exception) {
        // Keystore problems (rare, e.g. after restore on another device): fall back to private prefs.
        Log.w("SettingsStore", "EncryptedSharedPreferences unavailable, falling back", e)
        context.getSharedPreferences("pixelhort_secure_fallback", Context.MODE_PRIVATE)
    }

    companion object {
        private const val TOKEN_KEY = "access_token"

        /** Normalises user input like "photos.example.com" → "https://photos.example.com". */
        fun normalizeServerUrl(input: String): String {
            var u = input.trim()
            if (u.isEmpty()) return u
            if (!u.startsWith("http://", true) && !u.startsWith("https://", true)) u = "https://$u"
            u = u.trimEnd('/')
            if (u.endsWith("/api")) u = u.removeSuffix("/api")
            return u
        }
    }
}
