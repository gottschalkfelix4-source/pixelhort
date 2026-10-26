package app.pixelhort.ui.nav

import android.net.Uri
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.navigation.NavHostController
import app.pixelhort.AppContainer
import app.pixelhort.data.api.AssetDto

object Routes {
    const val SERVER = "server"
    const val LOGIN = "login"
    const val PHOTOS = "photos"
    const val SEARCH = "search"
    const val LIBRARY = "library"
    const val VIEWER = "viewer/{index}"
    const val ALBUMS = "albums"
    const val ALBUM = "album/{id}"
    const val FAVORITES = "favorites"
    const val ARCHIVE = "archive"
    const val TRASH = "trash"
    const val PEOPLE = "people"
    const val PERSON = "person/{id}"
    const val PLACE = "place/{name}"
    const val MAP = "map"
    const val SHARED_LINKS = "shared-links"
    const val PARTNERS = "partners"
    const val LOCKED = "locked"
    const val BACKUP = "backup"
    const val SETTINGS = "settings"

    fun viewer(index: Int) = "viewer/$index"
    fun album(id: String) = "album/$id"
    fun person(id: String) = "person/$id"
    fun place(name: String) = "place/${Uri.encode(name)}"
}

val LocalNav = staticCompositionLocalOf<NavHostController> { error("no nav controller") }

fun NavHostController.openViewer(container: AppContainer, assets: List<AssetDto>, index: Int) {
    if (assets.isEmpty()) return
    container.viewerAssets = assets
    navigate(Routes.viewer(index.coerceIn(0, assets.lastIndex)))
}
