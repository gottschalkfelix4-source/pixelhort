import Foundation
import Observation

/// Loads `/api/timeline/buckets` and lazily `/api/timeline/bucket` per month.
@MainActor
@Observable
final class TimelineViewModel {
    var filter: TimelineFilter
    private(set) var buckets: [TimeBucket] = []
    private(set) var days: [String: [DayGroup]] = [:]
    private(set) var isLoading = false
    private(set) var hasLoaded = false
    private(set) var generation = 0
    var errorMessage: String?

    var isSelecting = false
    var selection: Set<String> = []

    private var loadingBuckets: Set<String> = []

    init(filter: TimelineFilter) {
        self.filter = filter
    }

    var isDescending: Bool { filter.order != "asc" }

    var totalCount: Int { buckets.reduce(0) { $0 + $1.count } }

    func reload(api: APIClient) async {
        isLoading = true
        defer { isLoading = false }
        do {
            let loaded = try await api.timelineBuckets(filter)
            buckets = loaded
            days = [:]
            loadingBuckets = []
            generation += 1
            errorMessage = nil
        } catch is CancellationError {
            return
        } catch {
            errorMessage = error.localizedDescription
        }
        hasLoaded = true
    }

    func isLoaded(_ bucket: TimeBucket) -> Bool {
        days[bucket.id] != nil
    }

    func loadBucket(_ bucket: TimeBucket, api: APIClient) async {
        guard days[bucket.id] == nil, !loadingBuckets.contains(bucket.id) else { return }
        loadingBuckets.insert(bucket.id)
        defer { loadingBuckets.remove(bucket.id) }
        do {
            let assets = try await api.timelineBucket(bucket.timeBucket, filter: filter)
            days[bucket.id] = TimelineGrouping.groupByDay(assets, descending: isDescending)
        } catch {
            if !(error is CancellationError) {
                errorMessage = error.localizedDescription
            }
        }
    }

    /// All assets loaded so far in display order (used for paging in the viewer).
    var loadedAssets: [AssetDto] {
        buckets.flatMap { bucket in
            (days[bucket.id] ?? []).flatMap(\.assets)
        }
    }

    func assets(for ids: Set<String>) -> [AssetDto] {
        loadedAssets.filter { ids.contains($0.id) }
    }

    // MARK: Selection

    func toggleSelection(_ id: String) {
        if selection.contains(id) {
            selection.remove(id)
        } else {
            selection.insert(id)
        }
        if selection.isEmpty { isSelecting = false }
    }

    func beginSelection(with id: String) {
        isSelecting = true
        selection.insert(id)
    }

    func toggleDaySelection(_ day: DayGroup) {
        let ids = Set(day.assets.map(\.id))
        if ids.isSubset(of: selection) {
            selection.subtract(ids)
        } else {
            isSelecting = true
            selection.formUnion(ids)
        }
    }

    func endSelection() {
        isSelecting = false
        selection = []
    }

    // MARK: Local updates (optimistic)

    func removeLocally(_ ids: Set<String>) {
        for (key, groups) in days {
            days[key] = groups.compactMap { group in
                let remaining = group.assets.filter { !ids.contains($0.id) }
                return remaining.isEmpty ? nil : DayGroup(id: group.id, date: group.date, assets: remaining)
            }
        }
    }
}
