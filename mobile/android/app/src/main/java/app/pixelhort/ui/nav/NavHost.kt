package app.pixelhort.ui.nav

import android.net.Uri
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.PhotoLibrary
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.VideoLibrary
import androidx.compose.material.icons.outlined.CloudUpload
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import app.pixelhort.AppEvents
import app.pixelhort.R
import app.pixelhort.container
import app.pixelhort.ui.albums.AlbumDetailScreen
import app.pixelhort.ui.albums.AlbumsScreen
import app.pixelhort.ui.backup.BackupScreen
import app.pixelhort.ui.components.toast
import app.pixelhort.ui.library.LibraryScreen
import app.pixelhort.ui.locked.LockedFolderScreen
import app.pixelhort.ui.map.MapScreen
import app.pixelhort.ui.onboarding.LoginScreen
import app.pixelhort.ui.onboarding.ServerScreen
import app.pixelhort.ui.people.PeopleScreen
import app.pixelhort.ui.people.PersonScreen
import app.pixelhort.ui.search.FixedQuery
import app.pixelhort.ui.search.FixedSearchScreen
import app.pixelhort.ui.search.SearchScreen
import app.pixelhort.ui.settings.SettingsScreen
import app.pixelhort.ui.sharing.PartnersScreen
import app.pixelhort.ui.sharing.SharedLinksScreen
import app.pixelhort.ui.timeline.GridMode
import app.pixelhort.ui.timeline.TimelineFilter
import app.pixelhort.ui.timeline.TimelineScreen
import app.pixelhort.ui.viewer.ViewerScreen

private data class Tab(val route: String, val label: Int, val icon: ImageVector)

private val tabs = listOf(
    Tab(Routes.PHOTOS, R.string.photos, Icons.Filled.PhotoLibrary),
    Tab(Routes.SEARCH, R.string.search, Icons.Filled.Search),
    Tab(Routes.LIBRARY, R.string.library, Icons.Filled.VideoLibrary),
)

@Composable
fun PixelhortNavHost() {
    val ctx = LocalContext.current
    val c = ctx.container
    val nav = rememberNavController()
    val start = remember {
        val s = c.settings.current
        when {
            s.isLoggedIn -> Routes.PHOTOS
            !s.serverUrl.isNullOrBlank() -> Routes.LOGIN
            else -> Routes.SERVER
        }
    }

    // Token rejected by the server → back to login.
    LaunchedEffect(Unit) {
        c.apiClient.unauthorized.collect {
            if (c.settings.current.isLoggedIn) {
                c.auth.logout(remote = false)
                toast(ctx, ctx.getString(R.string.session_expired))
                nav.resetTo(Routes.LOGIN)
            }
        }
    }
    // Tapping a backup notification opens the backup status.
    LaunchedEffect(Unit) {
        AppEvents.openBackup.collect {
            AppEvents.consumeOpenBackup()
            if (c.settings.current.isLoggedIn) nav.navigate(Routes.BACKUP) { launchSingleTop = true }
        }
    }

    CompositionLocalProvider(LocalNav provides nav) {
        val entry by nav.currentBackStackEntryAsState()
        val route = entry?.destination?.route
        val showBottomBar = tabs.any { it.route == route }
        Scaffold(
            contentWindowInsets = WindowInsets(0),
            bottomBar = {
                if (showBottomBar) {
                    NavigationBar {
                        tabs.forEach { tab ->
                            NavigationBarItem(
                                selected = route == tab.route,
                                onClick = {
                                    nav.navigate(tab.route) {
                                        popUpTo(Routes.PHOTOS) { saveState = true }
                                        launchSingleTop = true
                                        restoreState = true
                                    }
                                },
                                icon = { Icon(tab.icon, null) },
                                label = { Text(stringResource(tab.label)) },
                            )
                        }
                    }
                }
            },
        ) { pad ->
            NavHost(nav, startDestination = start, modifier = Modifier.padding(pad).consumeWindowInsets(pad)) {
                composable(Routes.SERVER) { ServerScreen(onConnected = { nav.navigate(Routes.LOGIN) }) }
                composable(Routes.LOGIN) {
                    LoginScreen(
                        onLoggedIn = { nav.resetTo(Routes.PHOTOS) },
                        onChangeServer = { nav.resetTo(Routes.SERVER) },
                    )
                }
                composable(Routes.PHOTOS) {
                    TimelineScreen(
                        vmKey = "main",
                        filter = TimelineFilter(withPartners = true),
                        mode = GridMode.TIMELINE,
                        title = stringResource(R.string.app_name),
                        showBack = false,
                        emptyText = stringResource(R.string.timeline_empty),
                        withMemories = true,
                        topActions = {
                            IconButton(onClick = { nav.navigate(Routes.BACKUP) }) { Icon(Icons.Outlined.CloudUpload, stringResource(R.string.backup)) }
                            IconButton(onClick = { nav.navigate(Routes.SETTINGS) }) { Icon(Icons.Outlined.Settings, stringResource(R.string.settings)) }
                        },
                    )
                }
                composable(Routes.SEARCH) { SearchScreen() }
                composable(Routes.LIBRARY) { LibraryScreen() }
                composable(Routes.VIEWER) { e ->
                    ViewerScreen(e.arguments?.getString("index")?.toIntOrNull() ?: 0)
                }
                composable(Routes.ALBUMS) { AlbumsScreen() }
                composable(Routes.ALBUM) { e -> AlbumDetailScreen(e.arguments?.getString("id").orEmpty()) }
                composable(Routes.FAVORITES) {
                    TimelineScreen(
                        "favorites", TimelineFilter(isFavorite = true), GridMode.FAVORITES,
                        stringResource(R.string.favorites), true, stringResource(R.string.favorites_empty),
                    )
                }
                composable(Routes.ARCHIVE) {
                    TimelineScreen(
                        "archive", TimelineFilter(visibility = "archive"), GridMode.ARCHIVE,
                        stringResource(R.string.archive_title), true, stringResource(R.string.archive_empty),
                    )
                }
                composable(Routes.TRASH) {
                    TimelineScreen(
                        "trash", TimelineFilter(isTrashed = true), GridMode.TRASH,
                        stringResource(R.string.trash), true, stringResource(R.string.trash_empty),
                    )
                }
                composable(Routes.PEOPLE) { PeopleScreen() }
                composable(Routes.PERSON) { e -> PersonScreen(e.arguments?.getString("id").orEmpty()) }
                composable(Routes.PLACE) { e ->
                    val name = Uri.decode(e.arguments?.getString("name").orEmpty())
                    FixedSearchScreen(name, FixedQuery(city = name))
                }
                composable("object/{name}") { e ->
                    val name = Uri.decode(e.arguments?.getString("name").orEmpty())
                    FixedSearchScreen(name, FixedQuery(objects = listOf(name)))
                }
                composable(Routes.MAP) { MapScreen() }
                composable(Routes.SHARED_LINKS) { SharedLinksScreen() }
                composable(Routes.PARTNERS) { PartnersScreen() }
                composable(Routes.LOCKED) { LockedFolderScreen() }
                composable(Routes.BACKUP) { BackupScreen() }
                composable(Routes.SETTINGS) { SettingsScreen(onLoggedOut = { nav.resetTo(Routes.LOGIN) }) }
            }
        }
    }
}

/** Navigates to [route] and clears the whole back stack. */
private fun NavHostController.resetTo(route: String) {
    navigate(route) {
        popUpTo(graph.id) { inclusive = true }
        launchSingleTop = true
    }
}
