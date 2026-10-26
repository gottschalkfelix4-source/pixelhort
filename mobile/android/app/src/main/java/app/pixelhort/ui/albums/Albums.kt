package app.pixelhort.ui.albums

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Edit
import androidx.compose.material.icons.outlined.Link
import androidx.compose.material.icons.outlined.MoreVert
import androidx.compose.material.icons.outlined.PersonAdd
import androidx.compose.material.icons.outlined.PhotoAlbum
import androidx.compose.material.icons.outlined.RemoveCircleOutline
import androidx.compose.material.icons.outlined.FavoriteBorder
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.pixelhort.R
import app.pixelhort.container
import app.pixelhort.data.api.AlbumDto
import app.pixelhort.data.api.AlbumUserRole
import app.pixelhort.data.api.AlbumUsersRequest
import app.pixelhort.data.api.AssetBulkUpdateRequest
import app.pixelhort.data.api.CreateAlbumRequest
import app.pixelhort.data.api.IdsRequest
import app.pixelhort.data.api.UpdateAlbumRequest
import app.pixelhort.data.api.UserSummary
import app.pixelhort.data.api.userMessage
import app.pixelhort.ui.components.AlbumCover
import app.pixelhort.ui.components.AssetThumb
import app.pixelhort.ui.components.ConfirmDialog
import app.pixelhort.ui.components.CreateSharedLinkDialog
import app.pixelhort.ui.components.EmptyBox
import app.pixelhort.ui.components.RemoteContent
import app.pixelhort.ui.components.TextInputDialog
import app.pixelhort.ui.components.rememberRemote
import app.pixelhort.ui.components.toast
import app.pixelhort.ui.nav.LocalNav
import app.pixelhort.ui.nav.Routes
import app.pixelhort.ui.nav.openViewer
import kotlinx.coroutines.launch

@Composable
fun AlbumsScreen() {
    val c = LocalContext.current.container
    val ctx = LocalContext.current
    val nav = LocalNav.current
    val scope = rememberCoroutineScope()
    var sharedOnly by rememberSaveable { mutableStateOf(false) }
    val albums = rememberRemote(sharedOnly) { c.apiClient.api.albums(shared = if (sharedOnly) true else null) }
    var create by remember { mutableStateOf(false) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.albums)) },
                navigationIcon = {
                    IconButton(onClick = { nav.popBackStack() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) }
                },
            )
        },
        floatingActionButton = {
            ExtendedFloatingActionButton(
                onClick = { create = true },
                icon = { Icon(Icons.Outlined.Add, null) },
                text = { Text(stringResource(R.string.album_new)) },
            )
        },
    ) { pad ->
        PullToRefreshBox(isRefreshing = albums.loading && albums.data != null, onRefresh = { albums.reload() }, modifier = Modifier.padding(pad)) {
            RemoteContent(albums) { list ->
                LazyVerticalGrid(
                    columns = GridCells.Adaptive(160.dp),
                    contentPadding = PaddingValues(12.dp),
                    modifier = Modifier.fillMaxSize(),
                ) {
                    item(span = { GridItemSpan(maxLineSpan) }) {
                        androidx.compose.foundation.layout.Row {
                            FilterChip(!sharedOnly, { sharedOnly = false }, { Text(stringResource(R.string.all)) }, Modifier.padding(end = 8.dp))
                            FilterChip(sharedOnly, { sharedOnly = true }, { Text(stringResource(R.string.shared_albums)) })
                        }
                    }
                    if (list.isEmpty()) {
                        item(span = { GridItemSpan(maxLineSpan) }) { EmptyBox(stringResource(R.string.no_albums), Modifier.padding(top = 80.dp), Icons.Outlined.PhotoAlbum) }
                    }
                    items(list, key = { it.id }) { album -> AlbumCard(album) { nav.navigate(Routes.album(album.id)) } }
                }
            }
        }
    }
    if (create) {
        TextInputDialog(
            title = stringResource(R.string.album_new), label = stringResource(R.string.album_name), confirm = stringResource(R.string.create),
            onConfirm = { name ->
                scope.launch {
                    try {
                        val a = c.apiClient.api.createAlbum(CreateAlbumRequest(albumName = name))
                        nav.navigate(Routes.album(a.id))
                    } catch (e: Exception) { toast(ctx, e.userMessage()) }
                }
            },
            onDismiss = { create = false },
        )
    }
}

@Composable
private fun AlbumCard(album: AlbumDto, onClick: () -> Unit) {
    Column(Modifier.padding(6.dp).clickable(onClick = onClick)) {
        AlbumCover(album, Modifier.fillMaxWidth())
        Text(album.albumName, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 6.dp))
        val sub = buildString {
            append(LocalContext.current.getString(R.string.n_items, album.assetCount))
            if (album.shared) append(" · ").append(LocalContext.current.getString(R.string.shared))
        }
        Text(sub, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline)
    }
}

private enum class AlbumDialog { RENAME, DELETE, LINK, USERS, REMOVE }

@Composable
fun AlbumDetailScreen(albumId: String) {
    val ctx = LocalContext.current
    val c = ctx.container
    val nav = LocalNav.current
    val scope = rememberCoroutineScope()
    val album = rememberRemote(albumId) { c.apiClient.api.album(albumId) }
    var selection by remember { mutableStateOf<Set<String>>(emptySet()) }
    var dialog by remember { mutableStateOf<AlbumDialog?>(null) }
    var menu by remember { mutableStateOf(false) }
    val me = c.settings.current.user?.id
    val a = album.data
    val isOwner = a != null && (me == null || a.ownerId == me)
    val canEdit = isOwner || (a?.albumUsers?.any { it.user.id == me && it.role == "editor" } == true)

    BackHandler(selection.isNotEmpty()) { selection = emptySet() }

    fun act(block: suspend () -> Unit) = scope.launch {
        try { block(); selection = emptySet(); album.reload(); c.markDataChanged() } catch (e: Exception) { toast(ctx, e.userMessage()) }
    }

    Scaffold(
        topBar = {
            if (selection.isNotEmpty()) {
                TopAppBar(
                    navigationIcon = { IconButton(onClick = { selection = emptySet() }) { Icon(Icons.Outlined.Close, null) } },
                    title = { Text(stringResource(R.string.n_selected, selection.size)) },
                    actions = {
                        IconButton(onClick = { act { c.apiClient.api.updateAssets(AssetBulkUpdateRequest(selection.toList(), isFavorite = true)) } }) {
                            Icon(Icons.Outlined.FavoriteBorder, stringResource(R.string.favorite))
                        }
                        IconButton(onClick = { dialog = AlbumDialog.LINK }) { Icon(Icons.Outlined.Link, stringResource(R.string.share_link)) }
                        if (canEdit) {
                            IconButton(onClick = { dialog = AlbumDialog.REMOVE }) {
                                Icon(Icons.Outlined.RemoveCircleOutline, stringResource(R.string.remove_from_album))
                            }
                        }
                    },
                )
            } else {
                TopAppBar(
                    title = { Text(a?.albumName ?: "", maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    navigationIcon = {
                        IconButton(onClick = { nav.popBackStack() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) }
                    },
                    actions = {
                        IconButton(onClick = { dialog = AlbumDialog.LINK }, enabled = a != null) { Icon(Icons.Outlined.Link, stringResource(R.string.share_link)) }
                        if (isOwner) {
                            IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.MoreVert, null) }
                            DropdownMenu(menu, { menu = false }) {
                                DropdownMenuItem({ Text(stringResource(R.string.share_with_users)) }, { menu = false; dialog = AlbumDialog.USERS }, leadingIcon = { Icon(Icons.Outlined.PersonAdd, null) })
                                DropdownMenuItem({ Text(stringResource(R.string.rename)) }, { menu = false; dialog = AlbumDialog.RENAME }, leadingIcon = { Icon(Icons.Outlined.Edit, null) })
                                DropdownMenuItem({ Text(stringResource(R.string.delete_album)) }, { menu = false; dialog = AlbumDialog.DELETE }, leadingIcon = { Icon(Icons.Outlined.Delete, null) })
                            }
                        }
                    },
                )
            }
        },
    ) { pad ->
        PullToRefreshBox(isRefreshing = album.loading && album.data != null, onRefresh = { album.reload() }, modifier = Modifier.padding(pad)) {
            RemoteContent(album) { al ->
                LazyVerticalGrid(GridCells.Adaptive(110.dp), Modifier.fillMaxSize()) {
                    item(span = { GridItemSpan(maxLineSpan) }) {
                        Column(Modifier.padding(12.dp)) {
                            if (!al.description.isNullOrBlank()) Text(al.description)
                            val shared = al.albumUsers.joinToString { it.user.name.ifBlank { it.user.email } }
                            if (shared.isNotBlank()) Text(stringResource(R.string.shared_with, shared), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline)
                            Text(stringResource(R.string.n_items, al.assets.size), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline)
                        }
                    }
                    if (al.assets.isEmpty()) {
                        item(span = { GridItemSpan(maxLineSpan) }) { EmptyBox(stringResource(R.string.album_empty), Modifier.padding(top = 60.dp)) }
                    }
                    items(al.assets, key = { it.id }) { asset ->
                        AssetThumb(
                            asset = asset,
                            selected = asset.id in selection,
                            selectionMode = selection.isNotEmpty(),
                            onClick = {
                                if (selection.isNotEmpty()) selection = if (asset.id in selection) selection - asset.id else selection + asset.id
                                else nav.openViewer(c, al.assets, al.assets.indexOf(asset))
                            },
                            onLongClick = { selection = if (asset.id in selection) selection - asset.id else selection + asset.id },
                        )
                    }
                }
            }
        }
    }

    when (dialog) {
        AlbumDialog.RENAME -> TextInputDialog(
            title = stringResource(R.string.rename), initial = a?.albumName ?: "", label = stringResource(R.string.album_name),
            onConfirm = { name -> act { c.apiClient.api.updateAlbum(albumId, UpdateAlbumRequest(albumName = name)) } },
            onDismiss = { dialog = null },
        )
        AlbumDialog.DELETE -> ConfirmDialog(
            stringResource(R.string.delete_album), stringResource(R.string.confirm_delete_album), stringResource(R.string.delete),
            onConfirm = {
                scope.launch {
                    try { c.apiClient.api.deleteAlbum(albumId); nav.popBackStack() } catch (e: Exception) { toast(ctx, e.userMessage()) }
                }
            },
            onDismiss = { dialog = null },
        )
        AlbumDialog.REMOVE -> ConfirmDialog(
            stringResource(R.string.remove_from_album), stringResource(R.string.confirm_remove_from_album, selection.size), stringResource(R.string.remove),
            onConfirm = { val ids = selection.toList(); act { c.apiClient.api.removeAssetsFromAlbum(albumId, IdsRequest(ids)) } },
            onDismiss = { dialog = null },
        )
        AlbumDialog.LINK -> CreateSharedLinkDialog(
            assetIds = if (selection.isNotEmpty()) selection.toList() else null,
            albumId = if (selection.isEmpty()) albumId else null,
        ) { dialog = null; selection = emptySet() }
        AlbumDialog.USERS -> UserPickerDialog(
            exclude = (a?.albumUsers?.map { it.user.id } ?: emptyList()) + listOfNotNull(me),
            onPicked = { users, editor ->
                act { c.apiClient.api.addAlbumUsers(albumId, AlbumUsersRequest(users.map { AlbumUserRole(it.id, if (editor) "editor" else "viewer") })) }
            },
            onDismiss = { dialog = null },
        )
        null -> Unit
    }
}

/** Multi-select list of server users (for album sharing). */
@Composable
fun UserPickerDialog(
    exclude: List<String>,
    showRole: Boolean = true,
    onPicked: (List<UserSummary>, editor: Boolean) -> Unit,
    onDismiss: () -> Unit,
) {
    val c = LocalContext.current.container
    val users = rememberRemote { c.apiClient.api.users().filter { it.id !in exclude } }
    var picked by remember { mutableStateOf<Set<String>>(emptySet()) }
    var editor by remember { mutableStateOf(true) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.choose_users)) },
        text = {
            RemoteContent(users, Modifier.padding(16.dp)) { list ->
                Column {
                    if (list.isEmpty()) Text(stringResource(R.string.no_other_users))
                    LazyColumn(Modifier.fillMaxWidth().padding(bottom = 8.dp)) {
                        items(list, key = { it.id }) { u ->
                            ListItem(
                                headlineContent = { Text(u.name.ifBlank { u.email }) },
                                supportingContent = { Text(u.email) },
                                trailingContent = { Checkbox(u.id in picked, { picked = if (it) picked + u.id else picked - u.id }) },
                                modifier = Modifier.clickable { picked = if (u.id in picked) picked - u.id else picked + u.id },
                            )
                        }
                    }
                    if (showRole) {
                        androidx.compose.foundation.layout.Row {
                            FilterChip(editor, { editor = true }, { Text(stringResource(R.string.role_editor)) }, Modifier.padding(end = 8.dp))
                            FilterChip(!editor, { editor = false }, { Text(stringResource(R.string.role_viewer)) })
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(enabled = picked.isNotEmpty(), onClick = {
                onPicked(users.data.orEmpty().filter { it.id in picked }, editor); onDismiss()
            }) { Text(stringResource(R.string.ok)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
