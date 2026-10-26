package app.pixelhort.data

import android.content.Context
import android.os.Build
import app.pixelhort.backup.BackupScheduler
import app.pixelhort.data.api.ApiClient
import app.pixelhort.data.api.AuthStatus
import app.pixelhort.data.api.LoginRequest
import app.pixelhort.data.api.LoginResponse
import app.pixelhort.data.api.OAuthCallbackRequest
import app.pixelhort.data.db.BackupDao
import app.pixelhort.data.settings.SettingsStore

class AuthRepository(
    private val context: Context,
    private val settings: SettingsStore,
    private val apiClient: ApiClient,
    private val backupDao: BackupDao,
) {
    /** Validates the server via `/api/server/ping` and returns its auth capabilities. */
    suspend fun checkServer(rawUrl: String): Pair<String, AuthStatus> {
        val url = SettingsStore.normalizeServerUrl(rawUrl)
        val api = apiClient.forUrl(url)
        val pong = api.ping()
        require(pong.res == "pong") { "Unerwartete Antwort vom Server" }
        val status = runCatching { api.authStatus() }.getOrDefault(AuthStatus())
        if (settings.current.serverUrl != url) {
            // Different server → local backup state refers to another library.
            backupDao.clear()
        }
        settings.setServerUrl(url)
        return url to status
    }

    suspend fun authStatus(): AuthStatus = apiClient.api.authStatus()

    suspend fun login(email: String, password: String) {
        val res = apiClient.api.login(LoginRequest(email.trim(), password, "android", deviceName()))
        onLoggedIn(res)
    }

    suspend fun oauthAuthorizeUrl(): String = apiClient.api.oauthAuthorize(REDIRECT_URI).url

    suspend fun completeOAuth(callbackUrl: String) {
        onLoggedIn(apiClient.api.oauthCallback(OAuthCallbackRequest(callbackUrl)))
    }

    private suspend fun onLoggedIn(res: LoginResponse) {
        settings.setSession(res.accessToken, res.user)
    }

    suspend fun refreshUser() {
        val me = apiClient.api.me()
        settings.setUser(me)
    }

    suspend fun logout(remote: Boolean = true) {
        if (remote) runCatching { apiClient.api.logout() }
        BackupScheduler.cancelAll(context)
        // Backup state belongs to this account – another user must not inherit it.
        backupDao.clear()
        settings.clearSession()
    }

    private fun deviceName() = "${Build.MANUFACTURER} ${Build.MODEL}".trim()

    companion object {
        const val REDIRECT_URI = "app.pixelhort:/oauth-callback"
    }
}
