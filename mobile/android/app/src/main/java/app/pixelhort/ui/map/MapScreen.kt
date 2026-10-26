package app.pixelhort.ui.map

import android.graphics.Color as AColor
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.drawable.BitmapDrawable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.outlined.FavoriteBorder
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import app.pixelhort.R
import app.pixelhort.container
import app.pixelhort.data.api.MapMarkerDto
import app.pixelhort.data.api.userMessage
import app.pixelhort.ui.components.RemoteContent
import app.pixelhort.ui.components.rememberRemote
import app.pixelhort.ui.components.toast
import app.pixelhort.ui.nav.LocalNav
import app.pixelhort.ui.nav.openViewer
import kotlinx.coroutines.launch
import org.osmdroid.tileprovider.tilesource.TileSourceFactory
import org.osmdroid.util.BoundingBox
import org.osmdroid.util.GeoPoint
import org.osmdroid.views.MapView
import org.osmdroid.views.overlay.FolderOverlay
import org.osmdroid.views.overlay.Marker

/** OpenStreetMap (osmdroid) with one point per geotagged asset from `/api/map/markers`. */
@Composable
fun MapScreen() {
    val ctx = LocalContext.current
    val c = ctx.container
    val nav = LocalNav.current
    val scope = rememberCoroutineScope()
    var favoritesOnly by rememberSaveable { mutableStateOf(false) }
    val markers = rememberRemote(favoritesOnly) {
        c.apiClient.api.mapMarkers(isFavorite = if (favoritesOnly) true else null, withPartners = true)
    }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.map)) },
                navigationIcon = { IconButton(onClick = { nav.popBackStack() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                actions = {
                    IconButton(onClick = { favoritesOnly = !favoritesOnly }) {
                        Icon(if (favoritesOnly) Icons.Filled.Favorite else Icons.Outlined.FavoriteBorder, stringResource(R.string.favorites))
                    }
                },
            )
        },
    ) { pad ->
        Box(Modifier.fillMaxSize().padding(pad)) {
            RemoteContent(markers) { list ->
                OsmMap(list) { id ->
                    scope.launch {
                        try {
                            val asset = c.apiClient.api.asset(id)
                            nav.openViewer(c, listOf(asset), 0)
                        } catch (e: Exception) { toast(ctx, e.userMessage()) }
                    }
                }
            }
        }
    }
}

@Composable
private fun OsmMap(markers: List<MapMarkerDto>, onClick: (String) -> Unit) {
    val ctx = LocalContext.current
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    val mapView = remember {
        MapView(ctx).apply {
            setTileSource(TileSourceFactory.MAPNIK)
            setMultiTouchControls(true)
            zoomController.setVisibility(org.osmdroid.views.CustomZoomButtonsController.Visibility.SHOW_AND_FADEOUT)
            minZoomLevel = 2.0
            controller.setZoom(3.0)
            controller.setCenter(GeoPoint(50.0, 10.0))
        }
    }
    DisposableEffect(lifecycle) {
        val obs = LifecycleEventObserver { _, e ->
            when (e) {
                Lifecycle.Event.ON_RESUME -> mapView.onResume()
                Lifecycle.Event.ON_PAUSE -> mapView.onPause()
                else -> Unit
            }
        }
        lifecycle.addObserver(obs)
        onDispose {
            lifecycle.removeObserver(obs)
            mapView.onDetach()
        }
    }
    AndroidView(
        factory = { mapView },
        modifier = Modifier.fillMaxSize(),
        update = { map ->
            map.overlays.removeAll { it is FolderOverlay }
            if (markers.isNotEmpty()) {
                val icon = dotIcon(ctx)
                val folder = FolderOverlay()
                // Plain markers render reliably; cap the count to keep the map responsive.
                markers.take(MAX_MARKERS).forEach { m ->
                    folder.add(Marker(map).apply {
                        position = GeoPoint(m.lat, m.lon)
                        setAnchor(Marker.ANCHOR_CENTER, Marker.ANCHOR_CENTER)
                        this.icon = icon
                        title = listOfNotNull(m.city, m.country).joinToString(", ")
                        setOnMarkerClickListener { _, _ -> onClick(m.id); true }
                    })
                }
                map.overlays.add(folder)
                val points = markers.map { GeoPoint(it.lat, it.lon) }
                if (markers.size > 1) {
                    val box = BoundingBox.fromGeoPointsSafe(points)
                    map.post { runCatching { map.zoomToBoundingBox(box.increaseByScale(1.2f), false, 64) } }
                } else {
                    map.controller.setZoom(12.0)
                    map.controller.setCenter(points[0])
                }
            }
            map.invalidate()
        },
    )
}

private const val MAX_MARKERS = 3000

/** Blue dot with a white ring as marker icon. */
private fun dotIcon(ctx: android.content.Context): BitmapDrawable {
    val size = (22 * ctx.resources.displayMetrics.density).toInt()
    val bmp = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
    val canvas = Canvas(bmp)
    val r = size / 2f
    canvas.drawCircle(r, r, r, Paint(Paint.ANTI_ALIAS_FLAG).apply { color = AColor.WHITE })
    canvas.drawCircle(r, r, r * 0.72f, Paint(Paint.ANTI_ALIAS_FLAG).apply { color = AColor.parseColor("#3F6CF0") })
    return BitmapDrawable(ctx.resources, bmp)
}
