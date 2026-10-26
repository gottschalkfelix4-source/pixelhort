package app.pixelhort

import android.content.Intent
import android.os.Bundle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.fragment.app.FragmentActivity
import app.pixelhort.cast.CastHelper
import app.pixelhort.ui.nav.PixelhortNavHost
import app.pixelhort.ui.theme.PixelhortTheme
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow

/** Events crossing the Activity ↔ Compose boundary. */
object AppEvents {
    private val _oauthCallbacks = MutableSharedFlow<String>(replay = 1, onBufferOverflow = BufferOverflow.DROP_OLDEST)
    /** Full redirect URLs (`app.pixelhort:/oauth-callback?code=..&state=..`). */
    val oauthCallbacks: SharedFlow<String> = _oauthCallbacks

    private val _openBackup = MutableSharedFlow<Unit>(replay = 1, onBufferOverflow = BufferOverflow.DROP_OLDEST)
    val openBackup: SharedFlow<Unit> = _openBackup

    fun emitOAuth(url: String) { _oauthCallbacks.tryEmit(url) }
    fun emitOpenBackup() { _openBackup.tryEmit(Unit) }
    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    fun consumeOAuth() = _oauthCallbacks.resetReplayCache()
    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    fun consumeOpenBackup() = _openBackup.resetReplayCache()
}

/**
 * Single activity. Extends FragmentActivity because BiometricPrompt and the Cast
 * MediaRouteButton dialogs need a FragmentManager.
 */
class MainActivity : FragmentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // Initialise Cast early so the route button discovers devices.
        CastHelper.context(this)
        handleIntent(intent)
        setContent {
            PixelhortTheme {
                PixelhortNavHost()
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    private fun handleIntent(intent: Intent?) {
        val data = intent?.data
        if (data != null && data.scheme == "app.pixelhort" && data.toString().contains("oauth-callback")) {
            AppEvents.emitOAuth(data.toString())
        }
        if (intent?.getBooleanExtra(EXTRA_OPEN_BACKUP, false) == true) AppEvents.emitOpenBackup()
    }

    companion object {
        const val EXTRA_OPEN_BACKUP = "open_backup"
    }
}
