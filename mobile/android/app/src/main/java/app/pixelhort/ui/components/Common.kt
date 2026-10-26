package app.pixelhort.ui.components

import android.content.Context
import android.content.Intent
import android.widget.Toast
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.MotionPhotosOn
import androidx.compose.material.icons.filled.PlayCircle
import androidx.compose.material.icons.outlined.Circle
import androidx.compose.material.icons.outlined.CloudOff
import androidx.compose.material.icons.outlined.PhotoLibrary
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.pixelhort.R
import app.pixelhort.container
import app.pixelhort.data.api.AlbumDto
import app.pixelhort.data.api.AssetDto
import app.pixelhort.data.api.CreateAlbumRequest
import app.pixelhort.data.api.CreateSharedLinkRequest
import app.pixelhort.data.api.IdsRequest
import app.pixelhort.data.api.userMessage
import coil3.compose.AsyncImage
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import java.time.Instant
import java.time.temporal.ChronoUnit

// ---------------------------------------------------------------- remote loading

@Stable
class RemoteState<T> {
    var data by mutableStateOf<T?>(null)
    var error by mutableStateOf<String?>(null)
    var loading by mutableStateOf(true)
    internal var version by mutableIntStateOf(0)
    fun reload() { version++ }
}

/** Small helper for screens that just display the result of one API call. */
@Composable
fun <T> rememberRemote(vararg keys: Any?, load: suspend () -> T): RemoteState<T> {
    val state = remember(*keys) { RemoteState<T>() }
    LaunchedEffect(state, state.version) {
        state.loading = true
        state.error = null
        try {
            state.data = load()
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            state.error = e.userMessage()
        } finally {
            state.loading = false
        }
    }
    return state
}

@Composable
fun <T> RemoteContent(state: RemoteState<T>, modifier: Modifier = Modifier, content: @Composable (T) -> Unit) {
    val d = state.data
    when {
        d != null -> content(d)
        state.loading -> LoadingBox(modifier)
        else -> ErrorBox(state.error ?: "?", modifier) { state.reload() }
    }
}

@Composable
fun LoadingBox(modifier: Modifier = Modifier) {
    Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
}

@Composable
fun ErrorBox(message: String, modifier: Modifier = Modifier, onRetry: (() -> Unit)? = null) {
    Column(
        modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(Icons.Outlined.CloudOff, null, Modifier.size(48.dp), tint = MaterialTheme.colorScheme.error)
        Spacer(Modifier.height(12.dp))
        Text(message, textAlign = TextAlign.Center)
        if (onRetry != null) {
            Spacer(Modifier.height(12.dp))
            Button(onClick = onRetry) { Text(stringResource(R.string.retry)) }
        }
    }
}

@Composable
fun EmptyBox(text: String, modifier: Modifier = Modifier, icon: ImageVector = Icons.Outlined.PhotoLibrary) {
    Column(
        modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(icon, null, Modifier.size(56.dp), tint = MaterialTheme.colorScheme.outline)
        Spacer(Modifier.height(12.dp))
        Text(text, textAlign = TextAlign.Center, color = MaterialTheme.colorScheme.outline)
    }
}

// ---------------------------------------------------------------- thumbnails

@Composable
fun AssetThumb(
    asset: AssetDto,
    selected: Boolean,
    selectionMode: Boolean,
    onClick: () -> Unit,
    onLongClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val api = LocalContext.current.container.apiClient
    Box(
        modifier
            .aspectRatio(1f)
            .padding(1.dp)
            .background(MaterialTheme.colorScheme.surfaceVariant)
            .combinedClickable(onClick = onClick, onLongClick = onLongClick)
    ) {
        AsyncImage(
            model = api.thumbnailUrl(asset.id),
            contentDescription = asset.originalFileName,
            contentScale = ContentScale.Crop,
            modifier = Modifier.fillMaxSize().then(if (selected) Modifier.padding(10.dp).clip(RoundedCornerShape(8.dp)) else Modifier),
        )
        if (asset.isVideo) {
            Row(Modifier.align(Alignment.TopEnd).padding(4.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(Format.duration(asset.duration), color = Color.White, style = MaterialTheme.typography.labelSmall)
                Spacer(Modifier.size(2.dp))
                Icon(Icons.Filled.PlayCircle, null, tint = Color.White, modifier = Modifier.size(16.dp))
            }
        } else if (asset.hasLiveVideo || asset.isMotionPhoto) {
            Icon(Icons.Filled.MotionPhotosOn, null, tint = Color.White, modifier = Modifier.align(Alignment.TopEnd).padding(4.dp).size(16.dp))
        }
        if (asset.isFavorite) {
            Icon(Icons.Filled.Favorite, null, tint = Color.White, modifier = Modifier.align(Alignment.BottomStart).padding(4.dp).size(16.dp))
        }
        if (selectionMode) {
            Icon(
                if (selected) Icons.Filled.CheckCircle else Icons.Outlined.Circle,
                null,
                tint = if (selected) MaterialTheme.colorScheme.primary else Color.White,
                modifier = Modifier.align(Alignment.TopStart).padding(4.dp).size(22.dp)
                    .then(if (selected) Modifier.background(Color.White, CircleShape) else Modifier),
            )
        }
    }
}

@Composable
fun AlbumCover(album: AlbumDto, modifier: Modifier = Modifier) {
    val api = LocalContext.current.container.apiClient
    Box(modifier.aspectRatio(1f).clip(RoundedCornerShape(12.dp)).background(MaterialTheme.colorScheme.surfaceVariant)) {
        val thumb = album.albumThumbnailAssetId
        if (thumb != null) {
            AsyncImage(api.thumbnailUrl(thumb), null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
        } else {
            Icon(Icons.Outlined.PhotoLibrary, null, Modifier.align(Alignment.Center).size(40.dp), tint = MaterialTheme.colorScheme.outline)
        }
    }
}

// ---------------------------------------------------------------- dialogs

@Composable
fun ConfirmDialog(title: String, text: String, confirm: String, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = { Text(text) },
        confirmButton = { TextButton(onClick = { onConfirm(); onDismiss() }) { Text(confirm) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
fun TextInputDialog(
    title: String,
    initial: String = "",
    label: String = "",
    confirm: String = stringResource(R.string.ok),
    keyboardNumber: Boolean = false,
    password: Boolean = false,
    onConfirm: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var value by remember { mutableStateOf(initial) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            OutlinedTextField(
                value = value, onValueChange = { value = it }, label = { Text(label) }, singleLine = true,
                keyboardOptions = if (keyboardNumber) androidx.compose.foundation.text.KeyboardOptions(
                    keyboardType = androidx.compose.ui.text.input.KeyboardType.NumberPassword
                ) else androidx.compose.foundation.text.KeyboardOptions.Default,
                visualTransformation = if (password) androidx.compose.ui.text.input.PasswordVisualTransformation()
                else androidx.compose.ui.text.input.VisualTransformation.None,
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = {
            TextButton(enabled = value.isNotBlank(), onClick = { onConfirm(value.trim()); onDismiss() }) { Text(confirm) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

/** Lets the user pick an existing album (or create a new one) and adds [assetIds] to it. */
@Composable
fun AddToAlbumDialog(assetIds: List<String>, onDone: () -> Unit) {
    val ctx = LocalContext.current
    val api = ctx.container.apiClient
    val scope = rememberCoroutineScope()
    val albums = rememberRemote { api.api.albums().filter { it.ownerId == ctx.container.settings.current.user?.id || it.albumUsers.any { u -> u.user.id == ctx.container.settings.current.user?.id && u.role == "editor" } } }
    var creating by remember { mutableStateOf(false) }
    if (creating) {
        TextInputDialog(
            title = stringResource(R.string.album_new),
            label = stringResource(R.string.album_name),
            confirm = stringResource(R.string.create),
            onConfirm = { name ->
                scope.launch {
                    try {
                        api.api.createAlbum(CreateAlbumRequest(albumName = name, assetIds = assetIds))
                        toast(ctx, ctx.getString(R.string.added_to_album, name))
                    } catch (e: Exception) {
                        toast(ctx, e.userMessage())
                    }
                    onDone()
                }
            },
            onDismiss = { creating = false },
        )
        return
    }
    AlertDialog(
        onDismissRequest = onDone,
        title = { Text(stringResource(R.string.add_to_album)) },
        text = {
            RemoteContent(albums, Modifier.height(200.dp)) { list ->
                LazyColumn(Modifier.height(360.dp)) {
                    item {
                        ListItem(
                            headlineContent = { Text(stringResource(R.string.album_new)) },
                            modifier = Modifier.combinedClickable(onClick = { creating = true }),
                        )
                    }
                    items(list, key = { it.id }) { album ->
                        ListItem(
                            leadingContent = { AlbumCover(album, Modifier.size(48.dp)) },
                            headlineContent = { Text(album.albumName, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                            supportingContent = { Text(stringResource(R.string.n_items, album.assetCount)) },
                            modifier = Modifier.combinedClickable(onClick = {
                                scope.launch {
                                    try {
                                        val res = api.api.addAssetsToAlbum(album.id, IdsRequest(assetIds))
                                        val added = res.count { it.success }
                                        toast(ctx, ctx.getString(R.string.added_n_to_album, added, album.albumName))
                                    } catch (e: Exception) {
                                        toast(ctx, e.userMessage())
                                    }
                                    onDone()
                                }
                            }),
                        )
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(onClick = onDone) { Text(stringResource(R.string.cancel)) } },
    )
}

/** Creates a shared link (for assets or an album) and opens the Android share sheet. */
@Composable
fun CreateSharedLinkDialog(assetIds: List<String>? = null, albumId: String? = null, onDone: () -> Unit) {
    val ctx = LocalContext.current
    val api = ctx.container.apiClient
    val scope = rememberCoroutineScope()
    var description by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var allowDownload by remember { mutableStateOf(true) }
    var expiryDays by remember { mutableIntStateOf(0) }
    var busy by remember { mutableStateOf(false) }
    val expiryOptions = listOf(0, 1, 7, 30, 365)
    AlertDialog(
        onDismissRequest = onDone,
        title = { Text(stringResource(R.string.shared_link_create)) },
        text = {
            Column {
                OutlinedTextField(description, { description = it }, label = { Text(stringResource(R.string.description)) }, singleLine = true)
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    password, { password = it }, label = { Text(stringResource(R.string.password_optional)) }, singleLine = true,
                    visualTransformation = androidx.compose.ui.text.input.PasswordVisualTransformation(),
                )
                Spacer(Modifier.height(8.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(stringResource(R.string.allow_download), Modifier.weight(1f))
                    Switch(allowDownload, { allowDownload = it })
                }
                Text(stringResource(R.string.expires), style = MaterialTheme.typography.labelLarge)
                Row(Modifier.horizontalScroll(androidx.compose.foundation.rememberScrollState())) {
                    expiryOptions.forEach { d ->
                        androidx.compose.material3.FilterChip(
                            selected = expiryDays == d,
                            onClick = { expiryDays = d },
                            label = { Text(if (d == 0) stringResource(R.string.never) else stringResource(R.string.n_days, d)) },
                            modifier = Modifier.padding(end = 4.dp),
                        )
                    }
                }
            }
        },
        confirmButton = {
            TextButton(enabled = !busy, onClick = {
                busy = true
                scope.launch {
                    try {
                        val link = api.api.createSharedLink(
                            CreateSharedLinkRequest(
                                type = if (albumId != null) "ALBUM" else "INDIVIDUAL",
                                albumId = albumId,
                                assetIds = assetIds,
                                description = description.ifBlank { null },
                                password = password.ifBlank { null },
                                expiresAt = if (expiryDays > 0) Instant.now().plus(expiryDays.toLong(), ChronoUnit.DAYS).toString() else null,
                                allowDownload = allowDownload,
                                showMetadata = true,
                            )
                        )
                        shareText(ctx, link.url)
                    } catch (e: Exception) {
                        toast(ctx, e.userMessage())
                    }
                    onDone()
                }
            }) { Text(stringResource(R.string.share)) }
        },
        dismissButton = { TextButton(onClick = onDone) { Text(stringResource(R.string.cancel)) } },
    )
}

fun shareText(context: Context, text: String) {
    val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, text)
    context.startActivity(Intent.createChooser(send, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
}

fun toast(context: Context, msg: String) = Toast.makeText(context, msg, Toast.LENGTH_SHORT).show()

@Composable
fun SettingSwitch(title: String, subtitle: String? = null, checked: Boolean, enabled: Boolean = true, onChange: (Boolean) -> Unit) {
    ListItem(
        headlineContent = { Text(title) },
        supportingContent = subtitle?.let { { Text(it) } },
        trailingContent = { Switch(checked = checked, onCheckedChange = onChange, enabled = enabled) },
        modifier = Modifier.combinedClickable(enabled = enabled, onClick = { onChange(!checked) }),
    )
}
