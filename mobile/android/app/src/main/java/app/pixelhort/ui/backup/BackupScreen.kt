package app.pixelhort.ui.backup

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.CloudDone
import androidx.compose.material.icons.outlined.CloudUpload
import androidx.compose.material.icons.outlined.ErrorOutline
import androidx.compose.material.icons.outlined.HourglassEmpty
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.Checkbox
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.draw.clip
import androidx.work.WorkInfo
import androidx.work.WorkManager
import app.pixelhort.R
import app.pixelhort.backup.BackupScheduler
import app.pixelhort.backup.BackupWorker
import app.pixelhort.backup.DeviceAlbum
import app.pixelhort.backup.MediaStoreScanner
import app.pixelhort.container
import app.pixelhort.data.db.BackupStatus
import app.pixelhort.ui.components.Format
import app.pixelhort.ui.components.SettingSwitch
import app.pixelhort.ui.nav.LocalNav
import coil3.compose.AsyncImage
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

@Composable
fun BackupScreen() {
    val ctx = LocalContext.current
    val c = ctx.container
    val nav = LocalNav.current
    val scope = rememberCoroutineScope()
    val settings by c.settings.state.collectAsState()
    val opts = settings.backup
    var hasPermission by remember { mutableStateOf(c.scanner.hasMediaPermission()) }
    var albums by remember { mutableStateOf<List<DeviceAlbum>?>(null) }
    var showErrors by remember { mutableStateOf(false) }

    val permLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {
        hasPermission = c.scanner.hasMediaPermission()
    }

    LaunchedEffect(hasPermission) {
        if (hasPermission) albums = withContext(Dispatchers.IO) { c.scanner.albums() }
    }
    // Keep the Room index current so the counters are meaningful even before the first run.
    LaunchedEffect(hasPermission, opts.selectedBuckets, opts.includeVideos) {
        if (hasPermission) runCatching { c.backupEngine.syncIndex(opts) }
    }

    val bucketList = opts.selectedBuckets.toList()
    val countsFlow = remember(bucketList, opts.includeVideos) {
        if (bucketList.isEmpty()) flowOf(emptyList()) else c.db.backupDao().statusCounts(bucketList, opts.includeVideos)
    }
    val counts by countsFlow.collectAsState(initial = emptyList())
    val errors by remember { c.db.backupDao().errors() }.collectAsState(initial = emptyList())
    val works by remember { BackupScheduler.observe(ctx) }.collectAsState(initial = emptyList())
    val running = works.firstOrNull { it.state == WorkInfo.State.RUNNING }

    val backedUp = counts.filter { it.status == BackupStatus.UPLOADED || it.status == BackupStatus.DUPLICATE }.sumOf { it.count }
    val remaining = counts.filter { it.status == BackupStatus.PENDING || it.status == BackupStatus.ERROR }.sumOf { it.count }
    val failed = counts.filter { it.status == BackupStatus.ERROR }.sumOf { it.count }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.backup)) },
                navigationIcon = { IconButton(onClick = { nav.popBackStack() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
            )
        },
    ) { pad ->
        LazyColumn(Modifier.fillMaxSize().padding(pad)) {
            if (!hasPermission) {
                item {
                    Card(Modifier.fillMaxWidth().padding(16.dp)) {
                        Column(Modifier.padding(16.dp)) {
                            Text(stringResource(R.string.backup_permission_title), style = MaterialTheme.typography.titleMedium)
                            Spacer(Modifier.height(8.dp))
                            Text(stringResource(R.string.backup_permission_text))
                            Spacer(Modifier.height(12.dp))
                            Button(onClick = { permLauncher.launch(MediaStoreScanner.backupPermissions()) }) {
                                Text(stringResource(R.string.grant_permission))
                            }
                        }
                    }
                }
            }

            // status
            item {
                Card(Modifier.fillMaxWidth().padding(16.dp)) {
                    Column(Modifier.padding(16.dp)) {
                        Row(horizontalArrangement = Arrangement.SpaceEvenly, modifier = Modifier.fillMaxWidth()) {
                            Stat(Icons.Outlined.CloudDone, backedUp, stringResource(R.string.backed_up))
                            Stat(Icons.Outlined.HourglassEmpty, remaining, stringResource(R.string.remaining))
                            Stat(Icons.Outlined.ErrorOutline, failed, stringResource(R.string.errors))
                        }
                        Spacer(Modifier.height(12.dp))
                        if (running != null) {
                            val done = running.progress.getInt(BackupWorker.KEY_DONE, 0)
                            val total = running.progress.getInt(BackupWorker.KEY_TOTAL, 0)
                            val file = running.progress.getString(BackupWorker.KEY_FILE).orEmpty()
                            Text(
                                if (total > 0) stringResource(R.string.notif_backup_progress, done, total) else stringResource(R.string.notif_backup_preparing),
                                style = MaterialTheme.typography.bodyMedium,
                            )
                            if (file.isNotBlank()) Text(file, style = MaterialTheme.typography.bodySmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Spacer(Modifier.height(6.dp))
                            if (total > 0) LinearProgressIndicator(progress = { done / total.toFloat() }, modifier = Modifier.fillMaxWidth())
                            else LinearProgressIndicator(Modifier.fillMaxWidth())
                            Spacer(Modifier.height(8.dp))
                            OutlinedButton(onClick = { WorkManager.getInstance(ctx).cancelWorkById(running.id) }) { Text(stringResource(R.string.cancel)) }
                        } else {
                            Text(
                                if (settings.lastBackupAt > 0) stringResource(R.string.last_backup, Format.epochMillis(settings.lastBackupAt))
                                else stringResource(R.string.never_backed_up),
                                style = MaterialTheme.typography.bodySmall,
                            )
                            Spacer(Modifier.height(8.dp))
                            Button(
                                onClick = { BackupScheduler.backupNow(ctx) },
                                enabled = hasPermission && opts.selectedBuckets.isNotEmpty(),
                            ) {
                                Icon(Icons.Outlined.CloudUpload, null)
                                Spacer(Modifier.size(8.dp))
                                Text(stringResource(R.string.backup_now))
                            }
                        }
                    }
                }
            }

            // options
            item {
                SettingSwitch(
                    stringResource(R.string.backup_background), stringResource(R.string.backup_background_sub), opts.enabled,
                    enabled = opts.selectedBuckets.isNotEmpty(),
                ) { v -> scope.launch { c.settings.updateBackup { it.copy(enabled = v) } } }
            }
            item {
                SettingSwitch(stringResource(R.string.backup_wifi_only), null, opts.wifiOnly) { v ->
                    scope.launch { c.settings.updateBackup { it.copy(wifiOnly = v) } }
                }
            }
            item {
                SettingSwitch(stringResource(R.string.backup_charging_only), null, opts.chargingOnly) { v ->
                    scope.launch { c.settings.updateBackup { it.copy(chargingOnly = v) } }
                }
            }
            item {
                SettingSwitch(stringResource(R.string.backup_videos), null, opts.includeVideos) { v ->
                    scope.launch { c.settings.updateBackup { it.copy(includeVideos = v) } }
                }
            }
            item { HorizontalDivider() }

            // errors
            if (errors.isNotEmpty()) {
                item {
                    ListItem(
                        leadingContent = { Icon(Icons.Outlined.ErrorOutline, null, tint = MaterialTheme.colorScheme.error) },
                        headlineContent = { Text(stringResource(R.string.backup_errors_n, errors.size)) },
                        trailingContent = {
                            TextButton(onClick = { scope.launch { c.db.backupDao().resetErrors() } }) { Text(stringResource(R.string.retry)) }
                        },
                        modifier = Modifier.clickable { showErrors = !showErrors },
                    )
                }
                if (showErrors) {
                    items(errors, key = { "e" + it.localId }) { e ->
                        ListItem(
                            headlineContent = { Text(e.displayName, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                            supportingContent = { Text(e.error.orEmpty(), maxLines = 2) },
                        )
                    }
                }
                item { HorizontalDivider() }
            }

            // albums
            item {
                Text(
                    stringResource(R.string.backup_albums), style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.padding(start = 16.dp, top = 16.dp, end = 16.dp),
                )
                Text(
                    stringResource(R.string.backup_albums_sub), style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.outline,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                )
            }
            val list = albums
            if (hasPermission && list == null) item { LinearProgressIndicator(Modifier.fillMaxWidth().padding(16.dp)) }
            items(list.orEmpty(), key = { "a" + it.bucketId }) { album ->
                val checked = album.bucketId in opts.selectedBuckets
                ListItem(
                    leadingContent = {
                        AsyncImage(
                            album.coverUri, null, contentScale = ContentScale.Crop,
                            modifier = Modifier.size(48.dp).clip(RoundedCornerShape(8.dp)),
                        )
                    },
                    headlineContent = { Text(album.name) },
                    supportingContent = { Text(stringResource(R.string.n_items, album.count)) },
                    trailingContent = {
                        Checkbox(checked, { v -> toggleAlbum(scope, c, album.bucketId, v) })
                    },
                    modifier = Modifier.clickable { toggleAlbum(scope, c, album.bucketId, !checked) },
                )
            }
        }
    }
}

private fun toggleAlbum(scope: kotlinx.coroutines.CoroutineScope, c: app.pixelhort.AppContainer, bucketId: String, selected: Boolean) {
    scope.launch {
        c.settings.updateBackup { o ->
            val set = if (selected) o.selectedBuckets + bucketId else o.selectedBuckets - bucketId
            // Enable background backup automatically when the first album is chosen.
            o.copy(selectedBuckets = set, enabled = if (o.selectedBuckets.isEmpty() && set.isNotEmpty()) true else o.enabled && set.isNotEmpty())
        }
    }
}

@Composable
private fun Stat(icon: ImageVector, value: Int, label: String) {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Icon(icon, null, tint = MaterialTheme.colorScheme.primary)
        Text(value.toString(), style = MaterialTheme.typography.headlineSmall)
        Text(label, style = MaterialTheme.typography.bodySmall)
    }
}
