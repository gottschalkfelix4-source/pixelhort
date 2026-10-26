package app.pixelhort.ui.sharing

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Group
import androidx.compose.material.icons.outlined.Link
import androidx.compose.material.icons.outlined.PersonAdd
import androidx.compose.material.icons.outlined.Share
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.pixelhort.R
import app.pixelhort.container
import app.pixelhort.data.api.PartnerUpdateRequest
import app.pixelhort.data.api.SharedLinkDto
import app.pixelhort.data.api.userMessage
import app.pixelhort.ui.albums.UserPickerDialog
import app.pixelhort.ui.components.ConfirmDialog
import app.pixelhort.ui.components.EmptyBox
import app.pixelhort.ui.components.Format
import app.pixelhort.ui.components.RemoteContent
import app.pixelhort.ui.components.rememberRemote
import app.pixelhort.ui.components.shareText
import app.pixelhort.ui.components.toast
import app.pixelhort.ui.nav.LocalNav
import coil3.compose.AsyncImage
import kotlinx.coroutines.launch

@Composable
private fun BackTopBar(title: String, actions: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit = {}) {
    val nav = LocalNav.current
    TopAppBar(
        title = { Text(title) },
        navigationIcon = { IconButton(onClick = { nav.popBackStack() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
        actions = actions,
    )
}

@Composable
fun SharedLinksScreen() {
    val ctx = LocalContext.current
    val c = ctx.container
    val scope = rememberCoroutineScope()
    val links = rememberRemote { c.apiClient.api.sharedLinks() }
    var toDelete by remember { mutableStateOf<SharedLinkDto?>(null) }
    Scaffold(topBar = { BackTopBar(stringResource(R.string.shared_links)) }) { pad ->
        RemoteContent(links, Modifier.padding(pad)) { list ->
            if (list.isEmpty()) EmptyBox(stringResource(R.string.no_shared_links), Modifier.padding(pad), Icons.Outlined.Link)
            else LazyColumn(Modifier.fillMaxSize().padding(pad)) {
                items(list, key = { it.id }) { link ->
                    val thumb = link.album?.albumThumbnailAssetId ?: link.assetIds.firstOrNull()
                    ListItem(
                        leadingContent = {
                            if (thumb != null) AsyncImage(c.apiClient.thumbnailUrl(thumb), null, contentScale = ContentScale.Crop, modifier = Modifier.size(48.dp))
                            else Icon(Icons.Outlined.Link, null)
                        },
                        headlineContent = {
                            Text(
                                link.description?.takeIf { it.isNotBlank() } ?: link.album?.albumName
                                ?: stringResource(R.string.n_items, link.assetIds.size),
                                maxLines = 1, overflow = TextOverflow.Ellipsis,
                            )
                        },
                        supportingContent = {
                            val parts = listOfNotNull(
                                link.url,
                                if (link.hasPassword) stringResource(R.string.password_protected) else null,
                                link.expiresAt?.let { stringResource(R.string.expires_at, Format.instant(it)) },
                            )
                            Text(parts.joinToString("\n"), maxLines = 3, overflow = TextOverflow.Ellipsis)
                        },
                        trailingContent = {
                            androidx.compose.foundation.layout.Row {
                                IconButton(onClick = { copy(ctx, link.url) }) { Icon(Icons.Outlined.ContentCopy, stringResource(R.string.copy)) }
                                IconButton(onClick = { shareText(ctx, link.url) }) { Icon(Icons.Outlined.Share, stringResource(R.string.share)) }
                                IconButton(onClick = { toDelete = link }) { Icon(Icons.Outlined.Delete, stringResource(R.string.delete)) }
                            }
                        },
                    )
                    HorizontalDivider()
                }
            }
        }
    }
    toDelete?.let { link ->
        ConfirmDialog(
            stringResource(R.string.delete), stringResource(R.string.confirm_delete_link), stringResource(R.string.delete),
            onConfirm = {
                scope.launch {
                    try { c.apiClient.api.deleteSharedLink(link.id); links.reload() } catch (e: Exception) { toast(ctx, e.userMessage()) }
                }
            },
            onDismiss = { toDelete = null },
        )
    }
}

private fun copy(ctx: Context, text: String) {
    ctx.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("link", text))
    toast(ctx, ctx.getString(R.string.copied))
}

@Composable
fun PartnersScreen() {
    val ctx = LocalContext.current
    val c = ctx.container
    val scope = rememberCoroutineScope()
    val sharedBy = rememberRemote { c.apiClient.api.partners("shared-by") }
    val sharedWith = rememberRemote { c.apiClient.api.partners("shared-with") }
    var pick by remember { mutableStateOf(false) }

    fun act(block: suspend () -> Unit) = scope.launch {
        try { block(); sharedBy.reload(); sharedWith.reload(); c.markDataChanged() } catch (e: Exception) { toast(ctx, e.userMessage()) }
    }

    Scaffold(
        topBar = {
            BackTopBar(stringResource(R.string.partner_sharing)) {
                IconButton(onClick = { pick = true }) { Icon(Icons.Outlined.PersonAdd, stringResource(R.string.add_partner)) }
            }
        },
    ) { pad ->
        LazyColumn(Modifier.fillMaxSize().padding(pad)) {
            item {
                Text(stringResource(R.string.partners_shared_by), style = MaterialTheme.typography.titleSmall, modifier = Modifier.padding(16.dp))
            }
            val by = sharedBy.data.orEmpty()
            if (sharedBy.data != null && by.isEmpty()) item { Text(stringResource(R.string.partners_none_by), Modifier.padding(horizontal = 16.dp), color = MaterialTheme.colorScheme.outline) }
            items(by, key = { "by_" + it.user.id }) { p ->
                ListItem(
                    leadingContent = { Icon(Icons.Outlined.Group, null) },
                    headlineContent = { Text(p.user.name.ifBlank { p.user.email }) },
                    supportingContent = { Text(p.user.email) },
                    trailingContent = {
                        IconButton(onClick = { act { c.apiClient.api.removePartner(p.user.id) } }) { Icon(Icons.Outlined.Delete, stringResource(R.string.remove)) }
                    },
                )
            }
            item { HorizontalDivider(Modifier.padding(top = 8.dp)) }
            item {
                Text(stringResource(R.string.partners_shared_with), style = MaterialTheme.typography.titleSmall, modifier = Modifier.padding(16.dp))
            }
            val with = sharedWith.data.orEmpty()
            if (sharedWith.data != null && with.isEmpty()) item { Text(stringResource(R.string.partners_none_with), Modifier.padding(horizontal = 16.dp), color = MaterialTheme.colorScheme.outline) }
            items(with, key = { "with_" + it.user.id }) { p ->
                ListItem(
                    leadingContent = { Icon(Icons.Outlined.Group, null) },
                    headlineContent = { Text(p.user.name.ifBlank { p.user.email }) },
                    supportingContent = { Text(stringResource(R.string.show_in_timeline)) },
                    trailingContent = {
                        Switch(p.inTimeline, { v -> act { c.apiClient.api.updatePartner(p.user.id, PartnerUpdateRequest(v)) } })
                    },
                )
            }
            sharedBy.error?.let { item { Text(it, Modifier.padding(16.dp), color = MaterialTheme.colorScheme.error) } }
        }
    }
    if (pick) {
        UserPickerDialog(
            exclude = sharedBy.data.orEmpty().map { it.user.id } + listOfNotNull(c.settings.current.user?.id),
            showRole = false,
            onPicked = { users, _ -> act { users.forEach { c.apiClient.api.addPartner(it.id) } } },
            onDismiss = { pick = false },
        )
    }
}
