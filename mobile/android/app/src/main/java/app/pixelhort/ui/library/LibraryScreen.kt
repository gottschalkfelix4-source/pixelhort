package app.pixelhort.ui.library

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Archive
import androidx.compose.material.icons.outlined.CloudUpload
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.FavoriteBorder
import androidx.compose.material.icons.outlined.Group
import androidx.compose.material.icons.outlined.Link
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.Map
import androidx.compose.material.icons.outlined.People
import androidx.compose.material.icons.outlined.PhotoAlbum
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import app.pixelhort.R
import app.pixelhort.ui.nav.LocalNav
import app.pixelhort.ui.nav.Routes

@Composable
fun LibraryScreen() {
    val nav = LocalNav.current
    Scaffold(topBar = { TopAppBar(title = { Text(stringResource(R.string.library)) }) }) { pad ->
        LazyColumn(Modifier.fillMaxSize().padding(pad)) {
            item { Entry(Icons.Outlined.PhotoAlbum, stringResource(R.string.albums)) { nav.navigate(Routes.ALBUMS) } }
            item { Entry(Icons.Outlined.FavoriteBorder, stringResource(R.string.favorites)) { nav.navigate(Routes.FAVORITES) } }
            item { Entry(Icons.Outlined.People, stringResource(R.string.people)) { nav.navigate(Routes.PEOPLE) } }
            item { Entry(Icons.Outlined.Map, stringResource(R.string.map)) { nav.navigate(Routes.MAP) } }
            item { HorizontalDivider() }
            item { Entry(Icons.Outlined.Link, stringResource(R.string.shared_links)) { nav.navigate(Routes.SHARED_LINKS) } }
            item { Entry(Icons.Outlined.Group, stringResource(R.string.partner_sharing)) { nav.navigate(Routes.PARTNERS) } }
            item { HorizontalDivider() }
            item { Entry(Icons.Outlined.Archive, stringResource(R.string.archive_title)) { nav.navigate(Routes.ARCHIVE) } }
            item { Entry(Icons.Outlined.Lock, stringResource(R.string.locked_folder)) { nav.navigate(Routes.LOCKED) } }
            item { Entry(Icons.Outlined.Delete, stringResource(R.string.trash)) { nav.navigate(Routes.TRASH) } }
            item { HorizontalDivider() }
            item { Entry(Icons.Outlined.CloudUpload, stringResource(R.string.backup)) { nav.navigate(Routes.BACKUP) } }
            item { Entry(Icons.Outlined.Settings, stringResource(R.string.settings)) { nav.navigate(Routes.SETTINGS) } }
        }
    }
}

@Composable
private fun Entry(icon: ImageVector, title: String, onClick: () -> Unit) {
    ListItem(
        leadingContent = { Icon(icon, null) },
        headlineContent = { Text(title) },
        modifier = Modifier.clickable(onClick = onClick),
    )
}
