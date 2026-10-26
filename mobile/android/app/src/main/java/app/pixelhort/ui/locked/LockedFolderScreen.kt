package app.pixelhort.ui.locked

import android.content.Context
import android.content.ContextWrapper
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import app.pixelhort.AppContainer
import app.pixelhort.R
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewmodel.compose.viewModel
import app.pixelhort.container
import app.pixelhort.data.api.PinRequest
import app.pixelhort.data.api.SetPinRequest
import app.pixelhort.data.api.userMessage
import app.pixelhort.ui.components.ErrorBox
import app.pixelhort.ui.components.LoadingBox
import app.pixelhort.ui.components.TextInputDialog
import app.pixelhort.ui.nav.LocalNav
import app.pixelhort.ui.timeline.GridMode
import app.pixelhort.ui.timeline.TimelineFilter
import app.pixelhort.ui.timeline.TimelineScreen
import kotlinx.coroutines.launch
import java.time.Instant

private enum class LockState { LOADING, SETUP, LOCKED, UNLOCKED, ERROR }

@Composable
fun LockedFolderScreen() {
    val ctx = LocalContext.current
    val c = ctx.container
    val nav = LocalNav.current
    val scope = rememberCoroutineScope()
    var state by remember { mutableStateOf(LockState.LOADING) }
    var error by remember { mutableStateOf<String?>(null) }
    var askPin by remember { mutableStateOf(false) }
    var reload by remember { mutableStateOf(0) }

    LaunchedEffect(reload) {
        state = LockState.LOADING
        try {
            val s = c.apiClient.api.session()
            val unlocked = s.unlockedUntil?.let { runCatching { Instant.parse(it).isAfter(Instant.now()) }.getOrDefault(false) } ?: false
            state = when {
                !s.hasPin -> LockState.SETUP
                unlocked -> LockState.UNLOCKED
                else -> LockState.LOCKED
            }
        } catch (e: Exception) {
            error = e.userMessage(); state = LockState.ERROR
        }
    }

    // Leaving the locked folder (popping it off the back stack) re-locks the session server-side.
    // A ViewModel is used because the composable is also disposed when the viewer opens on top.
    viewModel(key = "lock_guard") { LockGuard(c) }

    fun startUnlock() {
        error = null
        val activity = ctx.findActivity()
        if (c.settings.current.biometricLock && activity != null && biometricAvailable(ctx)) {
            val prompt = BiometricPrompt(activity, ContextCompat.getMainExecutor(ctx), object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) { askPin = true }
                override fun onAuthenticationError(code: Int, msg: CharSequence) { error = msg.toString() }
            })
            prompt.authenticate(
                BiometricPrompt.PromptInfo.Builder()
                    .setTitle(ctx.getString(R.string.locked_folder))
                    .setSubtitle(ctx.getString(R.string.biometric_subtitle))
                    .setNegativeButtonText(ctx.getString(R.string.cancel))
                    .setAllowedAuthenticators(BiometricManager.Authenticators.BIOMETRIC_WEAK)
                    .build()
            )
        } else {
            askPin = true
        }
    }

    LaunchedEffect(state) { if (state == LockState.LOCKED) startUnlock() }

    if (state == LockState.UNLOCKED) {
        TimelineScreen(
            vmKey = "locked",
            filter = TimelineFilter(visibility = "locked"),
            mode = GridMode.LOCKED,
            title = stringResource(R.string.locked_folder),
            showBack = true,
            emptyText = stringResource(R.string.locked_empty),
            topActions = {
                IconButton(onClick = {
                    scope.launch { runCatching { c.apiClient.api.lockSession() }; nav.popBackStack() }
                }) { Icon(Icons.Outlined.Lock, stringResource(R.string.lock)) }
            },
        )
        return
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.locked_folder)) },
                navigationIcon = { IconButton(onClick = { nav.popBackStack() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
            )
        },
    ) { pad ->
        when (state) {
            LockState.LOADING -> LoadingBox(Modifier.padding(pad))
            LockState.ERROR -> ErrorBox(error ?: "?", Modifier.padding(pad)) { reload++ }
            else -> Column(
                Modifier.fillMaxSize().padding(pad).padding(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                Icon(Icons.Outlined.Lock, null, Modifier.size(72.dp), tint = MaterialTheme.colorScheme.primary)
                Spacer(Modifier.height(16.dp))
                Text(
                    stringResource(if (state == LockState.SETUP) R.string.locked_setup_text else R.string.locked_text),
                    textAlign = TextAlign.Center,
                )
                Spacer(Modifier.height(24.dp))
                Button(onClick = { if (state == LockState.SETUP) askPin = true else startUnlock() }) {
                    Text(stringResource(if (state == LockState.SETUP) R.string.set_pin else R.string.unlock))
                }
                error?.let {
                    Spacer(Modifier.height(12.dp))
                    Text(it, color = MaterialTheme.colorScheme.error)
                }
            }
        }
    }

    if (askPin) {
        val setup = state == LockState.SETUP
        TextInputDialog(
            title = stringResource(if (setup) R.string.set_pin else R.string.enter_pin),
            label = stringResource(R.string.pin_hint),
            keyboardNumber = true,
            password = true,
            confirm = stringResource(if (setup) R.string.save else R.string.unlock),
            onConfirm = { pin ->
                scope.launch {
                    try {
                        if (pin.length !in 4..8 || !pin.all { it.isDigit() }) throw IllegalArgumentException(ctx.getString(R.string.pin_invalid))
                        if (setup) c.apiClient.api.setPin(SetPinRequest(pin))
                        c.apiClient.api.unlockSession(PinRequest(pin))
                        error = null
                        state = LockState.UNLOCKED
                    } catch (e: retrofit2.HttpException) {
                        error = if (e.code() == 401 || e.code() == 403 || e.code() == 400) ctx.getString(R.string.pin_wrong) else e.userMessage()
                    } catch (e: Exception) {
                        error = e.userMessage()
                    }
                }
            },
            onDismiss = { askPin = false },
        )
    }
}

private class LockGuard(private val c: AppContainer) : ViewModel() {
    override fun onCleared() {
        c.appScope.launch { runCatching { c.apiClient.api.lockSession() } }
    }
}

private fun biometricAvailable(ctx: Context): Boolean =
    BiometricManager.from(ctx).canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_WEAK) == BiometricManager.BIOMETRIC_SUCCESS

fun Context.findActivity(): FragmentActivity? {
    var c: Context? = this
    while (c is ContextWrapper) {
        if (c is FragmentActivity) return c
        c = c.baseContext
    }
    return null
}
