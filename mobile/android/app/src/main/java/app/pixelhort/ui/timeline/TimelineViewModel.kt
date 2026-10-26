package app.pixelhort.ui.timeline

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.pixelhort.AppContainer
import app.pixelhort.data.api.AssetBulkUpdateRequest
import app.pixelhort.data.api.AssetDeleteRequest
import app.pixelhort.data.api.AssetDto
import app.pixelhort.data.api.MemoryDto
import app.pixelhort.data.api.PinRequest
import app.pixelhort.data.api.RestoreRequest
import app.pixelhort.data.api.TimeBucketDto
import app.pixelhort.data.api.userMessage
import app.pixelhort.ui.components.Format
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

data class TimelineFilter(
    val visibility: String? = null,
    val isFavorite: Boolean? = null,
    val albumId: String? = null,
    val personId: String? = null,
    val withPartners: Boolean? = null,
    val isTrashed: Boolean? = null,
)

/** Which actions the multi-select bar offers. */
enum class GridMode { TIMELINE, FAVORITES, ARCHIVE, TRASH, LOCKED, PERSON }

/**
 * Remote timeline: month buckets from `/api/timeline/buckets`; bucket contents are fetched
 * lazily from `/api/timeline/bucket` when they scroll into view.
 */
class TimelineViewModel(private val c: AppContainer, val filter: TimelineFilter, private val withMemories: Boolean) : ViewModel() {

    var buckets by mutableStateOf<List<TimeBucketDto>>(emptyList())
        private set
    val bucketAssets = mutableStateMapOf<String, List<AssetDto>>()
    /** Bucket contents grouped by day (yyyy-MM-dd → assets), newest first. */
    val bucketDays = mutableStateMapOf<String, List<Pair<String, List<AssetDto>>>>()
    var memories by mutableStateOf<List<MemoryDto>>(emptyList())
        private set
    var initialLoading by mutableStateOf(true)
        private set
    var refreshing by mutableStateOf(false)
        private set
    var error by mutableStateOf<String?>(null)
        private set
    var selection by mutableStateOf<Set<String>>(emptySet())
        private set

    private val inFlight = HashSet<String>()
    private var generation = 0

    init { refresh() }

    fun refresh() {
        val gen = ++generation
        viewModelScope.launch {
            refreshing = true
            try {
                val api = c.apiClient.api
                val b = api.timeBuckets(
                    visibility = filter.visibility, isFavorite = filter.isFavorite, albumId = filter.albumId,
                    personId = filter.personId, withPartners = filter.withPartners, isTrashed = filter.isTrashed,
                )
                if (gen != generation) return@launch
                bucketAssets.clear()
                bucketDays.clear()
                inFlight.clear()
                buckets = b
                error = null
                if (withMemories) {
                    memories = runCatching { api.memories() }.getOrDefault(emptyList()).filter { it.assets.isNotEmpty() }
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                error = e.userMessage()
            } finally {
                refreshing = false
                initialLoading = false
            }
        }
    }

    fun loadBucket(timeBucket: String) {
        if (bucketAssets.containsKey(timeBucket) || !inFlight.add(timeBucket)) return
        val gen = generation
        viewModelScope.launch {
            try {
                val list = c.apiClient.api.timeBucket(
                    timeBucket = timeBucket, visibility = filter.visibility, isFavorite = filter.isFavorite,
                    albumId = filter.albumId, personId = filter.personId, withPartners = filter.withPartners,
                    isTrashed = filter.isTrashed,
                )
                if (gen == generation) {
                    val sorted = list.sortedByDescending { it.localDateTime ?: it.fileCreatedAt }
                    bucketDays[timeBucket] = sorted.groupBy { Format.dayKey(it.localDateTime ?: it.fileCreatedAt) }.toList()
                    bucketAssets[timeBucket] = sorted
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                error = e.userMessage()
            } finally {
                inFlight.remove(timeBucket)
            }
        }
    }

    /** Flat list of all currently loaded assets in display order (for the viewer pager). */
    fun loadedAssets(): List<AssetDto> = buckets.flatMap { bucketAssets[it.timeBucket].orEmpty() }

    fun toggle(id: String) {
        selection = if (id in selection) selection - id else selection + id
    }

    fun select(ids: Collection<String>, selected: Boolean) {
        selection = if (selected) selection + ids else selection - ids.toSet()
    }

    fun clearSelection() { selection = emptySet() }

    fun selectedAssets(): List<AssetDto> = loadedAssets().filter { it.id in selection }

    /** Ids of selected assets owned by the current user (partner assets are read-only). */
    fun ownSelectedIds(): List<String> {
        val me = c.settings.current.user?.id
        return selectedAssets().filter { me == null || it.ownerId.isEmpty() || it.ownerId == me }.map { it.id }
    }

    // ---------------------------------------------------------------- actions

    fun runAction(onError: (String) -> Unit, onDone: () -> Unit = {}, block: suspend () -> Unit) {
        viewModelScope.launch {
            try {
                block()
                clearSelection()
                refresh()
                onDone()
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                onError(e.userMessage())
            }
        }
    }

    suspend fun setFavorite(ids: List<String>, fav: Boolean) =
        c.apiClient.api.updateAssets(AssetBulkUpdateRequest(ids = ids, isFavorite = fav))

    suspend fun setVisibility(ids: List<String>, visibility: String) =
        c.apiClient.api.updateAssets(AssetBulkUpdateRequest(ids = ids, visibility = visibility))

    /** Unlocks the session with the PIN, moves the assets and locks again. */
    suspend fun moveToLocked(ids: List<String>, pin: String) {
        c.apiClient.api.unlockSession(PinRequest(pin))
        try {
            setVisibility(ids, "locked")
        } finally {
            runCatching { c.apiClient.api.lockSession() }
        }
    }

    suspend fun delete(ids: List<String>, force: Boolean) =
        c.apiClient.api.deleteAssets(AssetDeleteRequest(ids, force))

    suspend fun restore(ids: List<String>?) = c.apiClient.api.restoreTrash(RestoreRequest(ids))

    suspend fun emptyTrash() = c.apiClient.api.emptyTrash()
}
