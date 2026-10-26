import Foundation

/// A day section inside a month bucket of the timeline.
struct DayGroup: Identifiable, Hashable, Sendable {
    let id: String          // yyyy-MM-dd
    let date: Date
    let assets: [AssetDto]
}

enum TimelineGrouping {
    /// `localDateTime` is wall-clock time encoded as UTC, so all grouping uses a UTC calendar.
    static var utcCalendar: Calendar {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "UTC") ?? .current
        return calendar
    }

    static func groupByDay(_ assets: [AssetDto], descending: Bool = true) -> [DayGroup] {
        let calendar = utcCalendar
        var groups: [String: (Date, [AssetDto])] = [:]
        var order: [String] = []
        for asset in assets {
            let components = calendar.dateComponents([.year, .month, .day], from: asset.sortDate)
            let key = String(format: "%04d-%02d-%02d", components.year ?? 0, components.month ?? 0, components.day ?? 0)
            if groups[key] == nil {
                let day = calendar.date(from: components) ?? asset.sortDate
                groups[key] = (day, [])
                order.append(key)
            }
            groups[key]?.1.append(asset)
        }
        let result = order.compactMap { key -> DayGroup? in
            guard let entry = groups[key] else { return nil }
            let date = entry.0
            let sorted = entry.1.sorted { descending ? $0.sortDate > $1.sortDate : $0.sortDate < $1.sortDate }
            return DayGroup(id: key, date: date, assets: sorted)
        }
        return result.sorted { descending ? $0.date > $1.date : $0.date < $1.date }
    }

    /// Parses a bucket key like `2024-05-01` (UTC).
    static func bucketDate(_ timeBucket: String) -> Date? {
        JSONCoding.parseDate(timeBucket)
    }

    static func monthTitle(_ timeBucket: String, locale: Locale = .current) -> String {
        guard let date = bucketDate(timeBucket) else { return timeBucket }
        let formatter = DateFormatter()
        formatter.locale = locale
        formatter.timeZone = TimeZone(identifier: "UTC")
        formatter.setLocalizedDateFormatFromTemplate("MMMMyyyy")
        return formatter.string(from: date)
    }

    static func dayTitle(_ date: Date, locale: Locale = .current) -> String {
        let formatter = DateFormatter()
        formatter.locale = locale
        formatter.timeZone = TimeZone(identifier: "UTC")
        formatter.setLocalizedDateFormatFromTemplate("EEEEdMMMM")
        return formatter.string(from: date)
    }

    /// Formats `localDateTime` (wall clock as UTC) for display.
    static func wallClockString(_ date: Date, locale: Locale = .current) -> String {
        let formatter = DateFormatter()
        formatter.locale = locale
        formatter.timeZone = TimeZone(identifier: "UTC")
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        return formatter.string(from: date)
    }

    static func durationString(_ seconds: Double) -> String {
        let total = Int(seconds.rounded())
        let hours = total / 3600
        let minutes = (total % 3600) / 60
        let secs = total % 60
        if hours > 0 {
            return String(format: "%d:%02d:%02d", hours, minutes, secs)
        }
        return String(format: "%d:%02d", minutes, secs)
    }
}
