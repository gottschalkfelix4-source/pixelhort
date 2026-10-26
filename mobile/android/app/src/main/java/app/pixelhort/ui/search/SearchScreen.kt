package app.pixelhort.ui.search

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material3.AssistChip
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import app.pixelhort.AppContainer
import app.pixelhort.R
import app.pixelhort.container
import app.pixelhort.data.api.AssetDto
import app.pixelhort.data.api.ExploreResponse
import app.pixelhort.data.api.MetadataSearchRequest
import app.pixelhort.data.api.SearchResponse
import app.pixelhort.data.api.SmartSearchRequest
import app.pixelhort.data.api.userMessage
import app.pixelhort.ui.components.AssetThumb
import app.pixelhort.ui.components.EmptyBox
import app.pixelhort.ui.components.ErrorBox
import app.pixelhort.ui.components.LoadingBox
import app.pixelhort.ui.nav.LocalNav
import app.pixelhort.ui.nav.Routes
import app.pixelhort.ui.nav.openViewer
import coil3.compose.AsyncImage
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

enum class SearchMode { SMART, FILENAME, TEXT }

/** Static query used for place/object explore results. */
data class FixedQuery(val city: String? = null, val objects: List<String>? = null)

class SearchViewModel(private val c: AppContainer, private val fixed: FixedQuery?) : ViewModel() {
    var query by mutableStateOf("")
    var mode by mutableStateOf(SearchMode.SMART)
    var type by mutableStateOf<String?>(null)
    var favorites by mutableStateOf(false)
    var results by mutableStateOf<List<AssetDto>>(emptyList())
        private set
    var searched by mutableStateOf(false)
        private set
    var loading by mutableStateOf(false)
        private set
    var error by mutableStateOf<String?>(null)
        private set
    var explore by mutableStateOf<ExploreResponse?>(null)
        private set
    var smartAvailable by mutableStateOf(true)
        private set
    private var nextPage: Int? = null
    private var job: Job? = null

    init {
        viewModelScope.launch {
            runCatching { c.apiClient.api.serverInfo() }.getOrNull()?.let {
                smartAvailable = it.features.smartSearch
                if (!smartAvailable && mode == SearchMode.SMART) mode = SearchMode.FILENAME
            }
        }
        if (fixed != null) search() else loadExplore()
    }

    fun loadExplore() {
        viewModelScope.launch { explore = runCatching { c.apiClient.api.explore() }.getOrNull() }
    }

    fun clear() {
        job?.cancel(); query = ""; results = emptyList(); searched = false; error = null
    }

    fun search() {
        if (fixed == null && query.isBlank()) { clear(); return }
        job?.cancel()
        results = emptyList(); nextPage = 1; searched = true
        loadMore()
    }

    fun loadMore() {
        val page = nextPage ?: return
        if (loading) return
        job = viewModelScope.launch {
            loading = true; error = null
            try {
                val res = fetch(page)
                results = results + res.assets
                nextPage = res.nextPage
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                error = e.userMessage()
                nextPage = null
            } finally {
                loading = false
            }
        }
    }

    private suspend fun fetch(page: Int): SearchResponse {
        val api = c.apiClient.api
        val fav = if (favorites) true else null
        if (fixed != null) {
            return api.metadataSearch(MetadataSearchRequest(city = fixed.city, objects = fixed.objects, page = page, size = PAGE))
        }
        return when (mode) {
            SearchMode.SMART -> api.smartSearch(SmartSearchRequest(query = query.trim(), page = page, size = PAGE, type = type, isFavorite = fav))
            SearchMode.FILENAME -> api.metadataSearch(MetadataSearchRequest(originalFileName = query.trim(), type = type, isFavorite = fav, page = page, size = PAGE))
            SearchMode.TEXT -> api.metadataSearch(MetadataSearchRequest(text = query.trim(), type = type, isFavorite = fav, page = page, size = PAGE))
        }
    }

    companion object { const val PAGE = 100 }
}

@Composable
fun SearchScreen() {
    val c = LocalContext.current.container
    val vm: SearchViewModel = viewModel(key = "search") { SearchViewModel(c, null) }
    val focus = LocalFocusManager.current
    Scaffold(
        topBar = {
            Column(Modifier.background(MaterialTheme.colorScheme.surface).statusBarsPadding().padding(top = 8.dp)) {
                OutlinedTextField(
                    value = vm.query,
                    onValueChange = { vm.query = it },
                    placeholder = { Text(stringResource(if (vm.mode == SearchMode.SMART) R.string.search_smart_hint else R.string.search_hint)) },
                    leadingIcon = { Icon(Icons.Outlined.Search, null) },
                    trailingIcon = {
                        if (vm.query.isNotEmpty()) IconButton(onClick = { vm.clear() }) { Icon(Icons.Outlined.Close, stringResource(R.string.clear)) }
                    },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    keyboardActions = KeyboardActions(onSearch = { focus.clearFocus(); vm.search() }),
                    shape = RoundedCornerShape(28.dp),
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp),
                )
                Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp, vertical = 4.dp)) {
                    if (vm.smartAvailable) {
                        Chip(vm.mode == SearchMode.SMART, stringResource(R.string.search_mode_smart)) { vm.mode = SearchMode.SMART; vm.search() }
                    }
                    Chip(vm.mode == SearchMode.FILENAME, stringResource(R.string.search_mode_filename)) { vm.mode = SearchMode.FILENAME; vm.search() }
                    Chip(vm.mode == SearchMode.TEXT, stringResource(R.string.search_mode_text)) { vm.mode = SearchMode.TEXT; vm.search() }
                    Chip(vm.type == "IMAGE", stringResource(R.string.photos_only)) { vm.type = if (vm.type == "IMAGE") null else "IMAGE"; vm.search() }
                    Chip(vm.type == "VIDEO", stringResource(R.string.videos_only)) { vm.type = if (vm.type == "VIDEO") null else "VIDEO"; vm.search() }
                    Chip(vm.favorites, stringResource(R.string.favorites)) { vm.favorites = !vm.favorites; vm.search() }
                }
            }
        },
    ) { pad ->
        Box(Modifier.fillMaxSize().padding(pad)) {
            if (!vm.searched) ExploreContent(vm.explore) else SearchResults(vm)
        }
    }
}

@Composable
private fun Chip(selected: Boolean, label: String, onClick: () -> Unit) {
    FilterChip(selected = selected, onClick = onClick, label = { Text(label) }, modifier = Modifier.padding(end = 6.dp))
}

@Composable
fun SearchResults(vm: SearchViewModel) {
    val c = LocalContext.current.container
    val nav = LocalNav.current
    val err = vm.error
    when {
        vm.results.isEmpty() && vm.loading -> LoadingBox()
        vm.results.isEmpty() && err != null -> ErrorBox(err) { vm.search() }
        vm.results.isEmpty() -> EmptyBox(stringResource(R.string.no_results), icon = Icons.Outlined.Search)
        else -> LazyVerticalGrid(GridCells.Adaptive(110.dp), Modifier.fillMaxSize()) {
            itemsIndexed(vm.results, key = { _, a -> a.id }) { i, asset ->
                if (i >= vm.results.size - 20) LaunchedEffect(vm.results.size) { vm.loadMore() }
                AssetThumb(asset, false, false, onClick = { nav.openViewer(c, vm.results, i) }, onLongClick = {})
            }
            if (vm.loading) {
                item(span = { GridItemSpan(maxLineSpan) }) {
                    Box(Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
                }
            }
        }
    }
}

@Composable
private fun ExploreContent(explore: ExploreResponse?) {
    val c = LocalContext.current.container
    val nav = LocalNav.current
    if (explore == null) { LoadingBox(); return }
    LazyVerticalGrid(GridCells.Adaptive(110.dp), Modifier.fillMaxSize(), contentPadding = PaddingValues(8.dp)) {
        val people = explore.people.filter { it.name.isNotBlank() && !it.isHidden }
        if (people.isNotEmpty()) {
            item(span = { GridItemSpan(maxLineSpan) }) { Header(stringResource(R.string.people)) { nav.navigate(Routes.PEOPLE) } }
            item(span = { GridItemSpan(maxLineSpan) }) {
                LazyRow(horizontalArrangement = Arrangement.spacedBy(12.dp), contentPadding = PaddingValues(horizontal = 8.dp)) {
                    items(people, key = { it.id }) { p ->
                        Column(
                            Modifier.width(76.dp).clickable { nav.navigate(Routes.person(p.id)) },
                            horizontalAlignment = Alignment.CenterHorizontally,
                        ) {
                            AsyncImage(
                                c.apiClient.personThumbnailUrl(p.id), p.name, contentScale = ContentScale.Crop,
                                modifier = Modifier.size(72.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceVariant),
                            )
                            Text(p.name, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodySmall)
                        }
                    }
                }
            }
        }
        if (explore.places.isNotEmpty()) {
            item(span = { GridItemSpan(maxLineSpan) }) { Header(stringResource(R.string.places)) { nav.navigate(Routes.MAP) } }
            items(explore.places, key = { "p_" + it.value }) { place ->
                Tile(place.value, place.assetId?.let { c.apiClient.thumbnailUrl(it) }) { nav.navigate(Routes.place(place.value)) }
            }
        }
        if (explore.objects.isNotEmpty()) {
            item(span = { GridItemSpan(maxLineSpan) }) { Header(stringResource(R.string.things), null) }
            item(span = { GridItemSpan(maxLineSpan) }) {
                Row(Modifier.horizontalScroll(rememberScrollState())) {
                    explore.objects.forEach { o ->
                        AssistChip(onClick = { nav.navigate("object/${android.net.Uri.encode(o.value)}") }, label = { Text(o.value) }, modifier = Modifier.padding(end = 6.dp))
                    }
                }
            }
        }
        if (people.isEmpty() && explore.places.isEmpty() && explore.objects.isEmpty()) {
            item(span = { GridItemSpan(maxLineSpan) }) { EmptyBox(stringResource(R.string.explore_empty), Modifier.height(400.dp), Icons.Outlined.Search) }
        }
    }
}

@Composable
private fun Header(title: String, onMore: (() -> Unit)?) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
        if (onMore != null) Text(
            stringResource(R.string.show_all), color = MaterialTheme.colorScheme.primary,
            modifier = Modifier.clickable(onClick = onMore).padding(4.dp),
        )
    }
}

@Composable
private fun Tile(label: String, image: String?, onClick: () -> Unit) {
    Box(Modifier.padding(3.dp).aspectRatio(1f).clip(RoundedCornerShape(10.dp)).background(MaterialTheme.colorScheme.surfaceVariant).clickable(onClick = onClick)) {
        if (image != null) AsyncImage(image, null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
        Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0.5f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.6f))))
        Text(label, color = Color.White, style = MaterialTheme.typography.labelLarge, maxLines = 1, overflow = TextOverflow.Ellipsis,
            modifier = Modifier.align(Alignment.BottomStart).padding(6.dp))
    }
}

/** Results for a place or detected object from the explore page. */
@Composable
fun FixedSearchScreen(title: String, query: FixedQuery) {
    val c = LocalContext.current.container
    val nav = LocalNav.current
    val vm: SearchViewModel = viewModel(key = "fixed_$query") { SearchViewModel(c, query) }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(title) },
                navigationIcon = { IconButton(onClick = { nav.popBackStack() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
            )
        },
    ) { pad -> Box(Modifier.padding(pad)) { SearchResults(vm) } }
}
