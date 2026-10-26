package app.pixelhort.ui.onboarding

import android.net.Uri
import androidx.browser.customtabs.CustomTabsIntent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.PhotoCamera
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import app.pixelhort.AppEvents
import app.pixelhort.R
import app.pixelhort.container
import app.pixelhort.data.api.AuthStatus
import app.pixelhort.data.api.userMessage
import kotlinx.coroutines.launch

@Composable
private fun OnboardingFrame(content: @Composable () -> Unit) {
    Column(
        Modifier.fillMaxSize().systemBarsPadding().imePadding().verticalScroll(rememberScrollState()).padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Spacer(Modifier.height(48.dp))
        Icon(Icons.Outlined.PhotoCamera, null, Modifier.size(64.dp), tint = MaterialTheme.colorScheme.primary)
        Spacer(Modifier.height(8.dp))
        Text(stringResource(R.string.app_name), style = MaterialTheme.typography.headlineMedium)
        Spacer(Modifier.height(32.dp))
        content()
    }
}

@Composable
fun ServerScreen(onConnected: () -> Unit) {
    val c = LocalContext.current.container
    val scope = rememberCoroutineScope()
    var url by rememberSaveable { mutableStateOf(c.settings.current.serverUrl ?: "") }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }

    fun connect() {
        if (url.isBlank()) return
        busy = true; error = null
        scope.launch {
            try {
                c.auth.checkServer(url)
                onConnected()
            } catch (e: Exception) {
                error = e.userMessage()
            } finally {
                busy = false
            }
        }
    }

    OnboardingFrame {
        Text(stringResource(R.string.server_intro), style = MaterialTheme.typography.bodyLarge)
        Spacer(Modifier.height(16.dp))
        OutlinedTextField(
            value = url,
            onValueChange = { url = it },
            label = { Text(stringResource(R.string.server_url)) },
            placeholder = { Text("https://fotos.example.com:2342") },
            singleLine = true,
            isError = error != null,
            supportingText = { Text(error ?: stringResource(R.string.server_url_hint)) },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go),
            keyboardActions = KeyboardActions(onGo = { connect() }),
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(Modifier.height(16.dp))
        Button(onClick = { connect() }, enabled = !busy && url.isNotBlank(), modifier = Modifier.fillMaxWidth()) {
            if (busy) CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp) else Text(stringResource(R.string.connect))
        }
    }
}

@Composable
fun LoginScreen(onLoggedIn: () -> Unit, onChangeServer: () -> Unit) {
    val ctx = LocalContext.current
    val c = ctx.container
    val scope = rememberCoroutineScope()
    var email by rememberSaveable { mutableStateOf("") }
    var password by rememberSaveable { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var status by remember { mutableStateOf<AuthStatus?>(null) }

    LaunchedEffect(Unit) {
        status = runCatching { c.auth.authStatus() }.getOrNull() ?: AuthStatus()
    }
    // OAuth redirect arrives via MainActivity intent → AppEvents
    LaunchedEffect(Unit) {
        AppEvents.oauthCallbacks.collect { callback ->
            AppEvents.consumeOAuth()
            busy = true; error = null
            try {
                c.auth.completeOAuth(callback)
                onLoggedIn()
            } catch (e: Exception) {
                error = e.userMessage()
            } finally {
                busy = false
            }
        }
    }

    fun login() {
        busy = true; error = null
        scope.launch {
            try {
                c.auth.login(email, password)
                onLoggedIn()
            } catch (e: Exception) {
                error = if (e is retrofit2.HttpException && e.code() == 401) ctx.getString(R.string.login_failed) else e.userMessage()
            } finally {
                busy = false
            }
        }
    }

    OnboardingFrame {
        Text(c.settings.current.serverUrl.orEmpty(), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.outline)
        TextButton(onClick = onChangeServer) { Text(stringResource(R.string.change_server)) }
        Spacer(Modifier.height(8.dp))
        val s = status
        if (s?.setupRequired == true) {
            Text(stringResource(R.string.setup_required), color = MaterialTheme.colorScheme.error)
            Spacer(Modifier.height(8.dp))
        }
        if (s == null || s.passwordLoginEnabled) {
            OutlinedTextField(
                email, { email = it }, label = { Text(stringResource(R.string.email)) }, singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email, imeAction = ImeAction.Next),
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                password, { password = it }, label = { Text(stringResource(R.string.password)) }, singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { if (email.isNotBlank() && password.isNotBlank()) login() }),
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(Modifier.height(16.dp))
            Button(
                onClick = { login() },
                enabled = !busy && email.isNotBlank() && password.isNotBlank(),
                modifier = Modifier.fillMaxWidth(),
            ) {
                if (busy) CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp) else Text(stringResource(R.string.login))
            }
        }
        if (s?.oauthEnabled == true) {
            Spacer(Modifier.height(16.dp))
            HorizontalDivider()
            Spacer(Modifier.height(16.dp))
            OutlinedButton(
                onClick = {
                    busy = true; error = null
                    scope.launch {
                        try {
                            val url = c.auth.oauthAuthorizeUrl()
                            CustomTabsIntent.Builder().setShowTitle(true).build().launchUrl(ctx, Uri.parse(url))
                        } catch (e: Exception) {
                            error = e.userMessage()
                        } finally {
                            busy = false
                        }
                    }
                },
                enabled = !busy,
                modifier = Modifier.fillMaxWidth(),
            ) { Text(s.oauthButtonText?.takeIf { it.isNotBlank() } ?: stringResource(R.string.login_oauth)) }
        }
        error?.let {
            Spacer(Modifier.height(12.dp))
            Text(it, color = MaterialTheme.colorScheme.error)
        }
    }
}
