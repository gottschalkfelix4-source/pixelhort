package app.pixelhort.ui.viewer

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateCentroid
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.PointerInputScope
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.platform.LocalDensity
import kotlinx.coroutines.withTimeoutOrNull

/**
 * Pinch-to-zoom / pan / double-tap container. Single-finger drags are only consumed while
 * zoomed in, so the surrounding pager can still swipe between assets at scale 1.
 */
@Composable
fun ZoomableBox(
    modifier: Modifier = Modifier,
    resetKey: Any? = null,
    onTap: () -> Unit = {},
    onZoomChanged: (Boolean) -> Unit = {},
    onLongPressStart: (() -> Unit)? = null,
    onLongPressEnd: (() -> Unit)? = null,
    content: @Composable BoxScope.() -> Unit,
) {
    var scale by remember { mutableFloatStateOf(1f) }
    var offset by remember { mutableStateOf(Offset.Zero) }
    LaunchedEffect(resetKey) { scale = 1f; offset = Offset.Zero; onZoomChanged(false) }

    BoxWithConstraints(modifier.fillMaxSize()) {
        val density = LocalDensity.current
        val w = with(density) { maxWidth.toPx() }
        val h = with(density) { maxHeight.toPx() }

        fun clamp(o: Offset, s: Float): Offset {
            val maxX = (w * (s - 1f)) / 2f
            val maxY = (h * (s - 1f)) / 2f
            return Offset(o.x.coerceIn(-maxX, maxX), o.y.coerceIn(-maxY, maxY))
        }

        Box(
            Modifier
                .fillMaxSize()
                .pointerInput(onLongPressStart) {
                    detectTapGestures(
                        onTap = { onTap() },
                        onDoubleTap = { tap ->
                            if (scale > 1f) {
                                scale = 1f; offset = Offset.Zero; onZoomChanged(false)
                            } else {
                                scale = 2.5f
                                val center = Offset(w / 2f, h / 2f)
                                offset = clamp((center - tap) * (scale - 1f), scale)
                                onZoomChanged(true)
                            }
                        },
                        onPress = {
                            if (onLongPressStart != null) {
                                val released = withTimeoutOrNull(350) { tryAwaitRelease() }
                                if (released == null) {
                                    onLongPressStart()
                                    tryAwaitRelease()
                                    onLongPressEnd?.invoke()
                                }
                            }
                        },
                    )
                }
                .pointerInput(Unit) { detectZoom(::clamp, { scale }, { s, o -> scale = s; offset = o; onZoomChanged(s > 1.01f) }, { offset }) }
                .graphicsLayer {
                    scaleX = scale; scaleY = scale
                    translationX = offset.x; translationY = offset.y
                }
        ) { content() }
    }
}

private suspend fun PointerInputScope.detectZoom(
    clamp: (Offset, Float) -> Offset,
    scale: () -> Float,
    update: (Float, Offset) -> Unit,
    offset: () -> Offset,
) {
    awaitEachGesture {
        awaitFirstDown(requireUnconsumed = false)
        do {
            val event = awaitPointerEvent()
            val multi = event.changes.count { it.pressed } > 1
            if (multi || scale() > 1.01f) {
                val zoom = event.calculateZoom()
                val pan = event.calculatePan()
                val newScale = (scale() * zoom).coerceIn(1f, 6f)
                val centroid = event.calculateCentroid(useCurrent = false)
                var newOffset = offset() + pan
                if (zoom != 1f && centroid != Offset.Unspecified) {
                    val c = Offset(size.width / 2f, size.height / 2f)
                    newOffset = (newOffset - (centroid - c)) * (newScale / scale()) + (centroid - c)
                }
                update(newScale, if (newScale <= 1f) Offset.Zero else clamp(newOffset, newScale))
                event.changes.forEach { if (it.positionChanged()) it.consume() }
            }
        } while (event.changes.any { it.pressed })
    }
}
