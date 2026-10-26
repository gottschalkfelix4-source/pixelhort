package app.pixelhort.ui.people

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.Edit
import androidx.compose.material.icons.outlined.People
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.pixelhort.R
import app.pixelhort.container
import app.pixelhort.data.api.UpdatePersonRequest
import app.pixelhort.data.api.userMessage
import app.pixelhort.ui.components.EmptyBox
import app.pixelhort.ui.components.RemoteContent
import app.pixelhort.ui.components.TextInputDialog
import app.pixelhort.ui.components.rememberRemote
import app.pixelhort.ui.components.toast
import app.pixelhort.ui.nav.LocalNav
import app.pixelhort.ui.nav.Routes
import app.pixelhort.ui.timeline.GridMode
import app.pixelhort.ui.timeline.TimelineFilter
import app.pixelhort.ui.timeline.TimelineScreen
import coil3.compose.AsyncImage
import kotlinx.coroutines.launch

@Composable
fun PeopleScreen() {
    val c = LocalContext.current.container
    val nav = LocalNav.current
    val people = rememberRemote { c.apiClient.api.people(withHidden = false) }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.people)) },
                navigationIcon = { IconButton(onClick = { nav.popBackStack() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
            )
        },
    ) { pad ->
        RemoteContent(people, Modifier.padding(pad)) { res ->
            if (res.people.isEmpty()) {
                EmptyBox(stringResource(R.string.no_people), Modifier.padding(pad), Icons.Outlined.People)
            } else {
                LazyVerticalGrid(GridCells.Adaptive(96.dp), Modifier.fillMaxSize().padding(pad), contentPadding = PaddingValues(8.dp)) {
                    items(res.people, key = { it.id }) { p ->
                        Column(
                            Modifier.padding(6.dp).clickable { nav.navigate(Routes.person(p.id)) },
                            horizontalAlignment = Alignment.CenterHorizontally,
                        ) {
                            AsyncImage(
                                c.apiClient.personThumbnailUrl(p.id), p.name, contentScale = ContentScale.Crop,
                                modifier = Modifier.fillMaxWidth().aspectRatio(1f).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceVariant),
                            )
                            Text(
                                p.name.ifBlank { stringResource(R.string.unnamed) },
                                maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodyMedium,
                                color = if (p.name.isBlank()) MaterialTheme.colorScheme.outline else MaterialTheme.colorScheme.onSurface,
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun PersonScreen(personId: String) {
    val ctx = LocalContext.current
    val c = ctx.container
    val scope = rememberCoroutineScope()
    val person = rememberRemote(personId) { c.apiClient.api.person(personId) }
    var rename by remember { mutableStateOf(false) }
    val name = person.data?.name.orEmpty()
    TimelineScreen(
        vmKey = "person_$personId",
        filter = TimelineFilter(personId = personId),
        mode = GridMode.PERSON,
        title = name.ifBlank { stringResource(R.string.unnamed) },
        showBack = true,
        emptyText = stringResource(R.string.no_photos),
        topActions = {
            IconButton(onClick = { rename = true }) { Icon(Icons.Outlined.Edit, stringResource(R.string.rename)) }
        },
    )
    if (rename) {
        TextInputDialog(
            title = stringResource(R.string.rename_person), initial = name, label = stringResource(R.string.name),
            onConfirm = { n ->
                scope.launch {
                    try {
                        person.data = c.apiClient.api.updatePerson(personId, UpdatePersonRequest(name = n))
                    } catch (e: Exception) { toast(ctx, e.userMessage()) }
                }
            },
            onDismiss = { rename = false },
        )
    }
}
