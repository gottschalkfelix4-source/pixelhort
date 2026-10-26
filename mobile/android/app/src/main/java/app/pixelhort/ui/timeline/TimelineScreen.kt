package app.pixelhort.ui.timeline

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.outlined.Archive
import androidx.compose.material.icons.outlined.Circle
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.DeleteForever
import androidx.compose.material.icons.outlined.FavoriteBorder
import androidx.compose.material.icons.outlined.Link
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.LockOpen
import androidx.compose.material.icons.outlined.MoreVert
import androidx.compose.material.icons.outlined.PhotoAlbum
import androidx.compose.material.icons.outlined.RestoreFromTrash
import androidx.compose.material.icons.outlined.Unarchive
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.viewmodel.compose.viewModel
import app.pixelhort.R
import app.pixelhort.container
import app.pixelhort.data.api.MemoryDto
import app.pixelhort.ui.components.AddToAlbumDialog
import app.pixelhort.ui.components.AssetThumb
import app.pixelhort.ui.components.ConfirmDialog
import app.pixelhort.ui.components.CreateSharedLinkDialog
import app.pixelhort.ui.components.EmptyBox
import app.pixelhort.ui.components.ErrorBox
import app.pixelhort.ui.components.Format
import app.pixelhort.ui.components.LoadingBox
import app.pixelhort.ui.components.TextInputDialog
import app.pixelhort.ui.components.toast
import app.pixelhort.ui.nav.LocalNav
import app.pixelhort.ui.nav.openViewer
import coil3.compose.AsyncImage
import kotlin.math.ceil
import kotlin.math.max

private enum class PendingDialog { ALBUM, LINK, DELETE, DELETE_FOREVER, EMPTY_TRASH, LOCK }

/**
 * Generic month/day grouped asset grid backed by the timeline endpoints. Used for the main
 * photo timeline as well as favorites, archive, trash, locked folder and people.
 */
@Composable
fun TimelineScreen(
    vmKey: String,
    filter: TimelineFilter,
    mode: GridMode,
    title: String,
    showBack: Boolean,
    emptyText: String,
    withMemories: Boolean = false,
    topActions: @Composable RowScope.() -> Unit = {},
) {
    val ctx = LocalContext.current
    val c = ctx.container
    val nav = LocalNav.current
    val vm: TimelineViewModel = viewModel(key = vmKey) { TimelineViewModel(c, filter, withMemories) }
    var dialog by remember { mutableStateOf<PendingDialog?>(null) }
    var dialogIds by remember { mutableStateOf<List<String>>(emptyList()) }
    val dataVersion by c.dataVersion.collectAsState()
    var seenVersion by remember { mutableIntStateOf(dataVersion) }

    LifecycleResumeEffect(dataVersion) {
        if (dataVersion != seenVersion) {
            seenVersion = dataVersion
            vm.refresh()
        }
        onPauseOrDispose { }
    }
    BackHandler(vm.selection.isNotEmpty()) { vm.clearSelection() }
    val onError: (String) -> Unit = { toast(ctx, it) }

    Scaffold(
        topBar = {
            if (vm.selection.isNotEmpty()) {
                SelectionTopBar(vm, mode, onDialog = { d, ids -> dialogIds = ids; dialog = d }, onError = onError)
            } else {
                TopAppBar(
                    title = { Text(title) },
                    navigationIcon = {
                        if (showBack) IconButton(onClick = { nav.popBackStack() }) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back))
                        }
                    },
                    actions = {
                        if (mode == GridMode.TRASH) {
                            IconButton(onClick = { vm.runAction(onError) { vm.restore(null) } }) {
                                Icon(Icons.Outlined.RestoreFromTrash, stringResource(R.string.restore_all))
                            }
                            IconButton(onClick = { dialog = PendingDialog.EMPTY_TRASH }) {
                                Icon(Icons.Outlined.DeleteForever, stringResource(R.string.empty_trash))
                            }
                        }
                        topActions()
                    },
                )
            }
        },
    ) { pad ->
        PullToRefreshBox(
            isRefreshing = vm.refreshing && !vm.initialLoading,
            onRefresh = { vm.refresh() },
            modifier = Modifier.fillMaxSize().padding(pad),
        ) {
            val err = vm.error
            when {
                vm.initialLoading -> LoadingBox()
                vm.buckets.isEmpty() && err != null -> Column(Modifier.verticalScroll(rememberScrollState())) {
                    ErrorBox(err, Modifier.height(400.dp)) { vm.refresh() }
                }
                vm.buckets.isEmpty() && vm.memories.isEmpty() -> Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                    EmptyBox(emptyText, Modifier.height(400.dp))
                }
                else -> TimelineGrid(vm, onOpen = { id ->
                    val all = vm.loadedAssets()
                    nav.openViewer(c, all, all.indexOfFirst { it.id == id })
                }, onOpenMemory = { m -> nav.openViewer(c, m.assets, 0) })
            }
        }
    }

    when (dialog) {
        PendingDialog.ALBUM -> AddToAlbumDialog(dialogIds) { dialog = null; vm.clearSelection() }
        PendingDialog.LINK -> CreateSharedLinkDialog(assetIds = dialogIds) { dialog = null; vm.clearSelection() }
        PendingDialog.DELETE -> ConfirmDialog(
            stringResource(R.string.delete), stringResource(R.string.confirm_trash, dialogIds.size), stringResource(R.string.delete),
            onConfirm = { val ids = dialogIds; vm.runAction(onError) { vm.delete(ids, force = false) } },
            onDismiss = { dialog = null },
        )
        PendingDialog.DELETE_FOREVER -> ConfirmDialog(
            stringResource(R.string.delete_forever), stringResource(R.string.confirm_delete_forever, dialogIds.size),
            stringResource(R.string.delete_forever),
            onConfirm = { val ids = dialogIds; vm.runAction(onError) { vm.delete(ids, force = true) } },
            onDismiss = { dialog = null },
        )
        PendingDialog.EMPTY_TRASH -> ConfirmDialog(
            stringResource(R.string.empty_trash), stringResource(R.string.confirm_empty_trash), stringResource(R.string.empty_trash),
            onConfirm = { vm.runAction(onError) { vm.emptyTrash() } },
            onDismiss = { dialog = null },
        )
        // Moving into the locked folder needs an unlocked session: ask for the PIN first.
        PendingDialog.LOCK -> TextInputDialog(
            title = stringResource(R.string.enter_pin),
            label = stringResource(R.string.pin_hint),
            confirm = stringResource(R.string.move_to_locked),
            keyboardNumber = true,
            password = true,
            onConfirm = { pin -> val ids = dialogIds; dialog = null; vm.runAction(onError) { vm.moveToLocked(ids, pin) } },
            onDismiss = { dialog = null },
        )
        null -> Unit
    }
}

@Composable
private fun SelectionTopBar(
    vm: TimelineViewModel,
    mode: GridMode,
    onDialog: (PendingDialog, List<String>) -> Unit,
    onError: (String) -> Unit,
) {
    var menu by remember { mutableStateOf(false) }
    val own = vm.ownSelectedIds()
    val all = vm.selection.toList()
    TopAppBar(
        navigationIcon = {
            IconButton(onClick = { vm.clearSelection() }) { Icon(Icons.Outlined.Close, stringResource(R.string.cancel)) }
        },
        title = { Text(stringResource(R.string.n_selected, vm.selection.size)) },
        actions = {
            when (mode) {
                GridMode.TRASH -> {
                    IconButton(onClick = { vm.runAction(onError) { vm.restore(own) } }) {
                        Icon(Icons.Outlined.RestoreFromTrash, stringResource(R.string.restore))
                    }
                    IconButton(onClick = { onDialog(PendingDialog.DELETE_FOREVER, own) }) {
                        Icon(Icons.Outlined.DeleteForever, stringResource(R.string.delete_forever))
                    }
                }
                GridMode.LOCKED -> {
                    IconButton(onClick = { vm.runAction(onError) { vm.setVisibility(own, "timeline") } }) {
                        Icon(Icons.Outlined.LockOpen, stringResource(R.string.move_out_of_locked))
                    }
                    IconButton(onClick = { onDialog(PendingDialog.DELETE_FOREVER, own) }) {
                        Icon(Icons.Outlined.DeleteForever, stringResource(R.string.delete_forever))
                    }
                }
                else -> {
                    IconButton(onClick = { onDialog(PendingDialog.LINK, all) }) {
                        Icon(Icons.Outlined.Link, stringResource(R.string.share_link))
                    }
                    val allFav = vm.selectedAssets().all { it.isFavorite }
                    IconButton(onClick = { vm.runAction(onError) { vm.setFavorite(own, !allFav) } }) {
                        Icon(if (allFav) Icons.Filled.Favorite else Icons.Outlined.FavoriteBorder, stringResource(R.string.favorite))
                    }
                    IconButton(onClick = { onDialog(PendingDialog.DELETE, own) }) {
                        Icon(Icons.Outlined.Delete, stringResource(R.string.delete))
                    }
                    IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.MoreVert, null) }
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.add_to_album)) },
                            leadingIcon = { Icon(Icons.Outlined.PhotoAlbum, null) },
                            onClick = { menu = false; onDialog(PendingDialog.ALBUM, all) },
                        )
                        if (mode == GridMode.ARCHIVE) {
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.unarchive)) },
                                leadingIcon = { Icon(Icons.Outlined.Unarchive, null) },
                                onClick = { menu = false; vm.runAction(onError) { vm.setVisibility(own, "timeline") } },
                            )
                        } else {
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.archive)) },
                                leadingIcon = { Icon(Icons.Outlined.Archive, null) },
                                onClick = { menu = false; vm.runAction(onError) { vm.setVisibility(own, "archive") } },
                            )
                        }
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.move_to_locked)) },
                            leadingIcon = { Icon(Icons.Outlined.Lock, null) },
                            onClick = { menu = false; onDialog(PendingDialog.LOCK, own) },
                        )
                    }
                }
            }
        },
    )
}

@Composable
private fun TimelineGrid(vm: TimelineViewModel, onOpen: (String) -> Unit, onOpenMemory: (MemoryDto) -> Unit) {
    val gridState = rememberLazyGridState()
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val cols = max(3, (maxWidth / 110.dp).toInt())
        val cell = maxWidth / cols
        val selectionMode = vm.selection.isNotEmpty()
        LazyVerticalGrid(
            columns = GridCells.Fixed(cols),
            state = gridState,
            contentPadding = PaddingValues(bottom = 24.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            if (vm.memories.isNotEmpty()) {
                item(key = "memories", span = { GridItemSpan(maxLineSpan) }) {
                    MemoriesRow(vm.memories, onOpenMemory)
                }
            }
            vm.buckets.forEach { bucket ->
                val tb = bucket.timeBucket
                item(key = "m_$tb", span = { GridItemSpan(maxLineSpan) }) {
                    Text(
                        Format.monthTitle(tb),
                        style = MaterialTheme.typography.titleLarge,
                        modifier = Modifier.padding(start = 12.dp, top = 20.dp, bottom = 4.dp),
                    )
                }
                val days = vm.bucketDays[tb]
                if (days == null) {
                    item(key = "p_$tb", span = { GridItemSpan(maxLineSpan) }) {
                        LaunchedEffect(tb) { vm.loadBucket(tb) }
                        val rows = ceil(bucket.count / cols.toDouble()).toInt().coerceAtLeast(1)
                        Box(
                            Modifier.fillMaxWidth().height(cell * rows)
                                .padding(1.dp)
                                .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f))
                        )
                    }
                } else {
                    days.forEach { (day, assets) ->
                        item(key = "d_${tb}_$day", span = { GridItemSpan(maxLineSpan) }) {
                            val allSelected = assets.all { it.id in vm.selection }
                            Row(
                                Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Text(Format.dayTitle(day), style = MaterialTheme.typography.labelLarge, modifier = Modifier.weight(1f))
                                if (selectionMode) {
                                    Icon(
                                        if (allSelected) Icons.Filled.CheckCircle else Icons.Outlined.Circle,
                                        null,
                                        tint = MaterialTheme.colorScheme.primary,
                                        modifier = Modifier.size(22.dp).clickable { vm.select(assets.map { it.id }, !allSelected) },
                                    )
                                }
                            }
                        }
                        items(assets, key = { it.id }) { asset ->
                            AssetThumb(
                                asset = asset,
                                selected = asset.id in vm.selection,
                                selectionMode = selectionMode,
                                onClick = { if (selectionMode) vm.toggle(asset.id) else onOpen(asset.id) },
                                onLongClick = { vm.toggle(asset.id) },
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun MemoriesRow(memories: List<MemoryDto>, onOpen: (MemoryDto) -> Unit) {
    val api = LocalContext.current.container.apiClient
    LazyRow(
        contentPadding = PaddingValues(horizontal = 12.dp, vertical = 8.dp),
        horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(8.dp),
    ) {
        items(memories, key = { it.id }) { m ->
            Box(
                Modifier.width(110.dp).height(150.dp).clip(RoundedCornerShape(12.dp))
                    .background(MaterialTheme.colorScheme.surfaceVariant)
                    .clickable { onOpen(m) }
            ) {
                m.assets.firstOrNull()?.let {
                    AsyncImage(api.thumbnailUrl(it.id), null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
                }
                Box(
                    Modifier.fillMaxSize().background(
                        Brush.verticalGradient(0.5f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.7f))
                    )
                )
                Text(
                    m.yearsAgo?.let { pluralStringResource(R.plurals.memory_years_ago, it, it) }
                        ?: m.title.ifBlank { m.year?.toString() ?: "" },
                    color = Color.White,
                    style = MaterialTheme.typography.labelLarge,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.align(Alignment.BottomStart).padding(8.dp),
                )
            }
        }
    }
}
