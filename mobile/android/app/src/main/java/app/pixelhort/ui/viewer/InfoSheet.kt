package app.pixelhort.ui.viewer

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.CalendarMonth
import androidx.compose.material.icons.outlined.Camera
import androidx.compose.material.icons.outlined.Image
import androidx.compose.material.icons.outlined.Place
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import app.pixelhort.R
import app.pixelhort.container
import app.pixelhort.data.api.AssetDto
import app.pixelhort.ui.components.Format
import app.pixelhort.ui.components.LoadingBox
import app.pixelhort.ui.components.ErrorBox
import app.pixelhort.ui.components.rememberRemote
import app.pixelhort.ui.nav.LocalNav
import app.pixelhort.ui.nav.Routes
import coil3.compose.AsyncImage
import java.util.Locale

@Composable
fun InfoSheet(assetId: String, onDismiss: () -> Unit) {
    val c = LocalContext.current.container
    val state = rememberRemote(assetId) { c.apiClient.api.asset(assetId) }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        val a = state.data
        when {
            a != null -> InfoContent(a, onDismiss)
            state.loading -> LoadingBox(Modifier.height(200.dp))
            else -> ErrorBox(state.error ?: "?", Modifier.height(200.dp)) { state.reload() }
        }
    }
}

@Composable
private fun InfoContent(a: AssetDto, onDismiss: () -> Unit) {
    val c = LocalContext.current.container
    val nav = LocalNav.current
    val e = a.exif
    Column(Modifier.verticalScroll(rememberScrollState()).padding(bottom = 32.dp)) {
        if (!a.description.isNullOrBlank()) {
            Text(a.description, Modifier.padding(horizontal = 16.dp, vertical = 8.dp), style = MaterialTheme.typography.bodyLarge)
        }
        ListItem(
            leadingContent = { Icon(Icons.Outlined.CalendarMonth, null) },
            headlineContent = { Text(Format.dateTime(a.localDateTime ?: e?.dateTimeOriginal ?: a.fileCreatedAt)) },
            supportingContent = e?.timeZone?.let { { Text(it) } },
        )
        ListItem(
            leadingContent = { Icon(Icons.Outlined.Image, null) },
            headlineContent = { Text(a.originalFileName) },
            supportingContent = {
                val parts = listOfNotNull(
                    if (a.width != null && a.height != null) "${a.width} × ${a.height}" else null,
                    a.fileSize?.let { Format.bytes(it) },
                    if (a.isVideo) Format.duration(a.duration) else null,
                    a.originalMimeType,
                )
                Text(parts.joinToString(" · "))
            },
        )
        if (e != null && (e.make != null || e.model != null)) {
            ListItem(
                leadingContent = { Icon(Icons.Outlined.Camera, null) },
                headlineContent = { Text(listOfNotNull(e.make, e.model).joinToString(" ")) },
                supportingContent = {
                    val parts = listOfNotNull(
                        e.lensModel,
                        e.fNumber?.let { "ƒ/" + String.format(Locale.ROOT, "%.1f", it) },
                        Format.exposure(e.exposureTime),
                        e.focalLength?.let { String.format(Locale.ROOT, "%.0f mm", it) },
                        e.iso?.let { "ISO ${it.toInt()}" },
                    )
                    Text(parts.joinToString(" · "))
                },
            )
        }
        if (e != null && (e.city != null || e.latitude != null)) {
            ListItem(
                leadingContent = { Icon(Icons.Outlined.Place, null) },
                headlineContent = { Text(listOfNotNull(e.city, e.state, e.country).joinToString(", ").ifBlank { stringResource(R.string.location) }) },
                supportingContent = if (e.latitude != null && e.longitude != null) {
                    { Text(String.format(Locale.ROOT, "%.5f, %.5f", e.latitude, e.longitude)) }
                } else null,
            )
        }
        val people = a.people.filter { it.name.isNotBlank() }
        if (people.isNotEmpty()) {
            Text(
                stringResource(R.string.people), style = MaterialTheme.typography.titleSmall,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
            Row(
                Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                people.forEach { p ->
                    AssistChip(
                        onClick = { onDismiss(); nav.navigate(Routes.person(p.id)) },
                        label = { Text(p.name) },
                        leadingIcon = {
                            AsyncImage(
                                c.apiClient.personThumbnailUrl(p.id), null, contentScale = ContentScale.Crop,
                                modifier = Modifier.size(24.dp).clip(CircleShape),
                            )
                        },
                    )
                }
            }
        }
        if (a.tags.isNotEmpty()) {
            Spacer(Modifier.height(8.dp))
            Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp)) {
                a.tags.forEach { t -> AssistChip(onClick = {}, label = { Text(t.value) }, modifier = Modifier.padding(end = 6.dp)) }
            }
        }
    }
}
