package app.pixelhort.ui.viewer

import android.Manifest
import android.app.DownloadManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.view.ContextThemeWrapper
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.MotionPhotosOn
import androidx.compose.material.icons.filled.PlayCircle
import androidx.compose.material.icons.outlined.Archive
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Download
import androidx.compose.material.icons.outlined.FavoriteBorder
import androidx.compose.material.icons.outlined.Info
import androidx.compose.material.icons.outlined.Link
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.MoreVert
import androidx.compose.material.icons.outlined.PhotoAlbum
import androidx.compose.material.icons.outlined.Share
import androidx.compose.material.icons.outlined.Unarchive
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import androidx.mediarouter.app.MediaRouteButton
import app.pixelhort.R
import app.pixelhort.cast.CastHelper
import app.pixelhort.container
import app.pixelhort.data.api.AssetDeleteRequest
import app.pixelhort.data.api.AssetDto
import app.pixelhort.data.api.AssetBulkUpdateRequest
import app.pixelhort.data.api.AssetUpdateRequest
import app.pixelhort.data.api.userMessage
import app.pixelhort.ui.components.AddToAlbumDialog
import app.pixelhort.ui.components.ConfirmDialog
import app.pixelhort.ui.components.CreateSharedLinkDialog
import app.pixelhort.ui.components.Format
import app.pixelhort.ui.components.toast
import app.pixelhort.ui.nav.LocalNav
import coil3.compose.AsyncImage
import coil3.compose.SubcomposeAsyncImage
import com.google.android.gms.cast.framework.CastButtonFactory
import com.google.android.gms.cast.framework.CastSession
import com.google.android.gms.cast.framework.SessionManagerListener
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import okhttp3.Request
import java.io.File
import java.io.IOException

private enum class ViewerDialog { DELETE, ALBUM, LINK, INFO }

@Composable
fun ViewerScreen(startIndex: Int) {
    val ctx = LocalContext.current
    val c = ctx.container
    val nav = LocalNav.current
    val scope = rememberCoroutineScope()
    val assets = remember { mutableStateListOf<AssetDto>().apply { addAll(c.viewerAssets) } }
    if (assets.isEmpty()) {
        LaunchedEffect(Unit) { nav.popBackStack() }
        return
    }
    val pager = rememberPagerState(initialPage = startIndex.coerceIn(0, assets.lastIndex)) { assets.size }
    var chrome by remember { mutableStateOf(true) }
    var zoomed by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    var dialog by remember { mutableStateOf<ViewerDialog?>(null) }
    val current = assets.getOrNull(pager.currentPage.coerceIn(0, assets.lastIndex)) ?: return
    val me = c.settings.current.user?.id
    val isOwn = me == null || current.ownerId.isEmpty() || current.ownerId == me
    val castConnected = rememberCastConnected()

    BackHandler(dialog != null) { dialog = null }

    // Cast: whenever a session is active, show the current asset on the TV.
    LaunchedEffect(pager.settledPage, castConnected) {
        if (!castConnected) return@LaunchedEffect
        val a = assets.getOrNull(pager.settledPage) ?: return@LaunchedEffect
        try {
            val res = c.apiClient.api.castUrl(a.id)
            CastHelper.load(ctx, a, res)
        } catch (e: Exception) {
            toast(ctx, e.userMessage())
        }
    }

    fun replace(a: AssetDto) {
        val i = assets.indexOfFirst { it.id == a.id }
        if (i >= 0) assets[i] = a
    }

    fun removeCurrent() {
        val i = assets.indexOfFirst { it.id == current.id }
        if (i >= 0) assets.removeAt(i)
        c.markDataChanged()
        if (assets.isEmpty()) nav.popBackStack()
    }

    fun act(block: suspend () -> Unit) {
        scope.launch {
            try { block() } catch (e: Exception) { toast(ctx, e.userMessage()) }
        }
    }

    Box(Modifier.fillMaxSize().background(Color.Black)) {
        HorizontalPager(
            state = pager,
            userScrollEnabled = !zoomed,
            key = { assets.getOrNull(it)?.id ?: it },
            beyondViewportPageCount = 1,
        ) { page ->
            val asset = assets.getOrNull(page) ?: return@HorizontalPager
            AssetPage(asset, isCurrent = page == pager.settledPage, onTap = { chrome = !chrome }, onZoom = { zoomed = it })
        }

        // top bar
        AnimatedVisibility(chrome, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.align(Alignment.TopCenter)) {
            Row(
                Modifier.fillMaxWidth()
                    .background(Brush.verticalGradient(listOf(Color.Black.copy(alpha = 0.7f), Color.Transparent)))
                    .statusBarsPadding()
                    .padding(horizontal = 4.dp, vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(onClick = { nav.popBackStack() }) {
                    Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back), tint = Color.White)
                }
                Column(Modifier.weight(1f)) {
                    Text(
                        Format.dateTime(current.localDateTime ?: current.fileCreatedAt),
                        color = Color.White, style = MaterialTheme.typography.titleSmall,
                        maxLines = 1, overflow = TextOverflow.Ellipsis,
                    )
                    current.exif?.city?.let {
                        Text(it, color = Color.White.copy(alpha = 0.8f), style = MaterialTheme.typography.bodySmall)
                    }
                }
                if (current.hasLiveVideo) {
                    Icon(Icons.Filled.MotionPhotosOn, stringResource(R.string.live_photo_hint), tint = Color.White, modifier = Modifier.padding(8.dp))
                }
                CastButton()
                Box {
                    IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.MoreVert, null, tint = Color.White) }
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.download)) },
                            leadingIcon = { Icon(Icons.Outlined.Download, null) },
                            onClick = { menu = false; download(ctx, current) },
                        )
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.share_link)) },
                            leadingIcon = { Icon(Icons.Outlined.Link, null) },
                            onClick = { menu = false; dialog = ViewerDialog.LINK },
                        )
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.add_to_album)) },
                            leadingIcon = { Icon(Icons.Outlined.PhotoAlbum, null) },
                            onClick = { menu = false; dialog = ViewerDialog.ALBUM },
                        )
                        if (isOwn) {
                            val archived = current.visibility == "archive"
                            DropdownMenuItem(
                                text = { Text(stringResource(if (archived) R.string.unarchive else R.string.archive)) },
                                leadingIcon = { Icon(if (archived) Icons.Outlined.Unarchive else Icons.Outlined.Archive, null) },
                                onClick = {
                                    menu = false
                                    act {
                                        c.apiClient.api.updateAsset(current.id, AssetUpdateRequest(visibility = if (archived) "timeline" else "archive"))
                                        removeCurrent()
                                    }
                                },
                            )
                            if (current.visibility != "locked") {
                                DropdownMenuItem(
                                    text = { Text(stringResource(R.string.move_to_locked)) },
                                    leadingIcon = { Icon(Icons.Outlined.Lock, null) },
                                    onClick = {
                                        menu = false
                                        act {
                                            c.apiClient.api.updateAssets(AssetBulkUpdateRequest(listOf(current.id), visibility = "locked"))
                                            removeCurrent()
                                        }
                                    },
                                )
                            }
                        }
                    }
                }
            }
        }

        // bottom actions
        AnimatedVisibility(chrome, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.align(Alignment.BottomCenter)) {
            Row(
                Modifier.fillMaxWidth()
                    .background(Brush.verticalGradient(listOf(Color.Transparent, Color.Black.copy(alpha = 0.7f))))
                    .navigationBarsPadding()
                    .padding(8.dp),
                horizontalArrangement = Arrangement.SpaceEvenly,
            ) {
                IconButton(onClick = { act { shareFile(ctx, current) } }) {
                    Icon(Icons.Outlined.Share, stringResource(R.string.share), tint = Color.White)
                }
                if (isOwn) {
                    IconButton(onClick = {
                        act {
                            val updated = c.apiClient.api.updateAsset(current.id, AssetUpdateRequest(isFavorite = !current.isFavorite))
                            replace(updated)
                            c.markDataChanged()
                        }
                    }) {
                        Icon(
                            if (current.isFavorite) Icons.Filled.Favorite else Icons.Outlined.FavoriteBorder,
                            stringResource(R.string.favorite), tint = Color.White,
                        )
                    }
                }
                IconButton(onClick = { dialog = ViewerDialog.INFO }) {
                    Icon(Icons.Outlined.Info, stringResource(R.string.info), tint = Color.White)
                }
                if (isOwn) {
                    IconButton(onClick = { dialog = ViewerDialog.DELETE }) {
                        Icon(Icons.Outlined.Delete, stringResource(R.string.delete), tint = Color.White)
                    }
                }
            }
        }
    }

    when (dialog) {
        ViewerDialog.DELETE -> {
            val forever = current.isTrashed || current.visibility == "locked"
            ConfirmDialog(
                stringResource(if (forever) R.string.delete_forever else R.string.delete),
                if (forever) stringResource(R.string.confirm_delete_forever, 1) else stringResource(R.string.confirm_trash, 1),
                stringResource(R.string.delete),
                onConfirm = {
                    act {
                        c.apiClient.api.deleteAssets(AssetDeleteRequest(listOf(current.id), force = forever))
                        removeCurrent()
                    }
                },
                onDismiss = { dialog = null },
            )
        }
        ViewerDialog.ALBUM -> AddToAlbumDialog(listOf(current.id)) { dialog = null }
        ViewerDialog.LINK -> CreateSharedLinkDialog(assetIds = listOf(current.id)) { dialog = null }
        ViewerDialog.INFO -> InfoSheet(current.id, onDismiss = { dialog = null })
        null -> Unit
    }
}

@Composable
private fun AssetPage(asset: AssetDto, isCurrent: Boolean, onTap: () -> Unit, onZoom: (Boolean) -> Unit) {
    val api = LocalContext.current.container.apiClient
    var playLive by remember(asset.id) { mutableStateOf(false) }
    var playVideo by remember(asset.id) { mutableStateOf(false) }
    LaunchedEffect(isCurrent) { if (!isCurrent) { playVideo = false; playLive = false } else if (asset.isVideo) playVideo = true }

    if (asset.isVideo && playVideo && isCurrent) {
        VideoPlayer(api.videoUrl(asset.id), Modifier.fillMaxSize(), onTap = onTap)
        return
    }
    ZoomableBox(
        resetKey = if (isCurrent) asset.id else null,
        onTap = onTap,
        onZoomChanged = { if (isCurrent) onZoom(it) },
        onLongPressStart = if (asset.hasLiveVideo) ({ playLive = true }) else null,
        onLongPressEnd = if (asset.hasLiveVideo) ({ playLive = false }) else null,
    ) {
        SubcomposeAsyncImage(
            model = api.thumbnailUrl(asset.id, preview = true),
            contentDescription = asset.originalFileName,
            contentScale = ContentScale.Fit,
            modifier = Modifier.fillMaxSize(),
            loading = {
                // low-res thumbnail (usually already cached by the grid) while the preview loads
                AsyncImage(api.thumbnailUrl(asset.id), null, contentScale = ContentScale.Fit, modifier = Modifier.fillMaxSize())
            },
        )
        if (asset.isVideo) {
            Icon(
                Icons.Filled.PlayCircle, null, tint = Color.White,
                modifier = Modifier.align(Alignment.Center).size(72.dp),
            )
        }
        if (playLive && asset.livePhotoVideoId != null) {
            VideoPlayer(api.videoUrl(asset.livePhotoVideoId), Modifier.fillMaxSize(), showControls = false, loop = true)
        }
    }
}

@Composable
private fun CastButton() {
    val ctx = LocalContext.current
    if (CastHelper.context(ctx) == null) return
    AndroidView(
        factory = { c ->
            MediaRouteButton(ContextThemeWrapper(c, androidx.appcompat.R.style.ThemeOverlay_AppCompat_Dark)).also {
                runCatching { CastButtonFactory.setUpMediaRouteButton(c.applicationContext, it) }
            }
        },
        modifier = Modifier.size(48.dp),
    )
}

@Composable
private fun rememberCastConnected(): Boolean {
    val ctx = LocalContext.current
    var connected by remember { mutableStateOf(CastHelper.isConnected(ctx)) }
    DisposableEffect(Unit) {
        val sm = CastHelper.context(ctx)?.sessionManager
        val listener = object : SessionManagerListener<CastSession> {
            override fun onSessionStarted(s: CastSession, id: String) { connected = true }
            override fun onSessionResumed(s: CastSession, wasSuspended: Boolean) { connected = true }
            override fun onSessionEnded(s: CastSession, error: Int) { connected = false }
            override fun onSessionSuspended(s: CastSession, reason: Int) { connected = false }
            override fun onSessionStarting(s: CastSession) {}
            override fun onSessionStartFailed(s: CastSession, error: Int) {}
            override fun onSessionEnding(s: CastSession) {}
            override fun onSessionResuming(s: CastSession, id: String) {}
            override fun onSessionResumeFailed(s: CastSession, error: Int) {}
        }
        sm?.addSessionManagerListener(listener, CastSession::class.java)
        onDispose { sm?.removeSessionManagerListener(listener, CastSession::class.java) }
    }
    return connected
}

/** Downloads the original into the cache and opens the Android share sheet. */
private suspend fun shareFile(context: Context, asset: AssetDto) {
    toast(context, context.getString(R.string.preparing_share))
    val c = context.container
    val file = withContext(Dispatchers.IO) {
        val dir = File(context.cacheDir, "share").apply { mkdirs() }
        dir.listFiles()?.forEach { if (System.currentTimeMillis() - it.lastModified() > 3_600_000) it.delete() }
        val out = File(dir, asset.originalFileName.ifBlank { "${asset.id}.jpg" }.replace('/', '_'))
        c.apiClient.okHttp.newCall(Request.Builder().url(c.apiClient.originalUrl(asset.id)).build()).execute().use { resp ->
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}")
            resp.body!!.byteStream().use { input -> out.outputStream().use { input.copyTo(it) } }
        }
        out
    }
    val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", file)
    val send = Intent(Intent.ACTION_SEND)
        .setType(asset.originalMimeType ?: if (asset.isVideo) "video/*" else "image/*")
        .putExtra(Intent.EXTRA_STREAM, uri)
        .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    context.startActivity(Intent.createChooser(send, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
}

/** Saves the original via DownloadManager (Pictures/Pixelhort). */
private fun download(context: Context, asset: AssetDto) {
    val api = context.container.apiClient
    val name = asset.originalFileName.ifBlank { "${asset.id}.jpg" }
    val req = DownloadManager.Request(Uri.parse(api.originalUrl(asset.id)))
        .setTitle(name)
        .setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED)
    api.authHeader()?.let { req.addRequestHeader("Authorization", it) }
    val canPublic = Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q ||
        ContextCompat.checkSelfPermission(context, Manifest.permission.WRITE_EXTERNAL_STORAGE) == PackageManager.PERMISSION_GRANTED
    if (canPublic) {
        req.setDestinationInExternalPublicDir(Environment.DIRECTORY_PICTURES, "Pixelhort/$name")
    } else {
        req.setDestinationInExternalFilesDir(context, Environment.DIRECTORY_PICTURES, name)
    }
    try {
        context.getSystemService(DownloadManager::class.java).enqueue(req)
        toast(context, context.getString(R.string.download_started))
    } catch (e: Exception) {
        toast(context, e.userMessage())
    }
}
