package app.pixelhort.ui.settings

import androidx.biometric.BiometricManager
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.Logout
import androidx.compose.material.icons.outlined.AccountCircle
import androidx.compose.material.icons.outlined.CloudUpload
import androidx.compose.material.icons.outlined.Dns
import androidx.compose.material.icons.outlined.Info
import androidx.compose.material.icons.outlined.Storage
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import app.pixelhort.BuildConfig
import app.pixelhort.R
import app.pixelhort.container
import app.pixelhort.data.api.ServerInfo
import app.pixelhort.ui.components.ConfirmDialog
import app.pixelhort.ui.components.Format
import app.pixelhort.ui.components.SettingSwitch
import app.pixelhort.ui.nav.LocalNav
import app.pixelhort.ui.nav.Routes
import kotlinx.coroutines.launch

@Composable
fun SettingsScreen(onLoggedOut: () -> Unit) {
    val ctx = LocalContext.current
    val c = ctx.container
    val nav = LocalNav.current
    val scope = rememberCoroutineScope()
    val settings by c.settings.state.collectAsState()
    var info by remember { mutableStateOf<ServerInfo?>(null) }
    var confirmLogout by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        info = runCatching { c.apiClient.api.serverInfo() }.getOrNull()
        runCatching { c.auth.refreshUser() }
    }
    val user = settings.user
    val biometricOk = remember {
        BiometricManager.from(ctx).canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_WEAK) == BiometricManager.BIOMETRIC_SUCCESS
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.settings)) },
                navigationIcon = { IconButton(onClick = { nav.popBackStack() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
            )
        },
    ) { pad ->
        LazyColumn(Modifier.fillMaxSize().padding(pad)) {
            item { Section(stringResource(R.string.account)) }
            item {
                ListItem(
                    leadingContent = { Icon(Icons.Outlined.AccountCircle, null) },
                    headlineContent = { Text(user?.name?.ifBlank { null } ?: user?.email ?: "–") },
                    supportingContent = { Text(user?.email.orEmpty() + if (user?.isAdmin == true) " · Admin" else "") },
                )
            }
            if (user != null) {
                item {
                    ListItem(
                        leadingContent = { Icon(Icons.Outlined.Storage, null) },
                        headlineContent = {
                            Text(
                                if (user.quotaBytes != null) stringResource(R.string.usage_quota, Format.bytes(user.usageBytes), Format.bytes(user.quotaBytes))
                                else stringResource(R.string.usage, Format.bytes(user.usageBytes))
                            )
                        },
                        supportingContent = user.quotaBytes?.takeIf { it > 0 }?.let { q ->
                            { LinearProgressIndicator(progress = { (user.usageBytes.toFloat() / q).coerceIn(0f, 1f) }, modifier = Modifier.fillMaxWidth().padding(top = 6.dp)) }
                        },
                    )
                }
            }
            item { HorizontalDivider() }
            item { Section(stringResource(R.string.server)) }
            item {
                ListItem(
                    leadingContent = { Icon(Icons.Outlined.Dns, null) },
                    headlineContent = { Text(settings.serverUrl.orEmpty()) },
                    supportingContent = { Text(info?.let { stringResource(R.string.server_version, it.version) } ?: stringResource(R.string.server_unreachable)) },
                )
            }
            item { HorizontalDivider() }
            item { Section(stringResource(R.string.backup)) }
            item {
                ListItem(
                    leadingContent = { Icon(Icons.Outlined.CloudUpload, null) },
                    headlineContent = { Text(stringResource(R.string.backup_settings)) },
                    supportingContent = {
                        Text(
                            if (settings.backup.enabled) stringResource(R.string.backup_on_n_albums, settings.backup.selectedBuckets.size)
                            else stringResource(R.string.backup_off)
                        )
                    },
                    modifier = Modifier.clickable { nav.navigate(Routes.BACKUP) },
                )
            }
            item { HorizontalDivider() }
            item { Section(stringResource(R.string.security)) }
            item {
                SettingSwitch(
                    stringResource(R.string.biometric_lock), stringResource(R.string.biometric_lock_sub),
                    settings.biometricLock, enabled = biometricOk,
                ) { v -> scope.launch { c.settings.setBiometricLock(v) } }
            }
            item { HorizontalDivider() }
            item {
                ListItem(
                    leadingContent = { Icon(Icons.Outlined.Info, null) },
                    headlineContent = { Text(stringResource(R.string.app_version, BuildConfig.VERSION_NAME)) },
                    supportingContent = { Text(stringResource(R.string.device_id, settings.deviceId)) },
                )
            }
            item {
                ListItem(
                    leadingContent = { Icon(Icons.AutoMirrored.Outlined.Logout, null, tint = MaterialTheme.colorScheme.error) },
                    headlineContent = { Text(stringResource(R.string.logout), color = MaterialTheme.colorScheme.error) },
                    modifier = Modifier.clickable { confirmLogout = true },
                )
            }
        }
    }
    if (confirmLogout) {
        ConfirmDialog(
            stringResource(R.string.logout), stringResource(R.string.confirm_logout), stringResource(R.string.logout),
            onConfirm = { scope.launch { c.auth.logout(); onLoggedOut() } },
            onDismiss = { confirmLogout = false },
        )
    }
}

@Composable
private fun Section(title: String) {
    Text(title, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(start = 16.dp, top = 16.dp, bottom = 4.dp))
}
