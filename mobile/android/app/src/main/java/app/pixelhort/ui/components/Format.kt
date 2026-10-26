package app.pixelhort.ui.components

import java.time.Instant
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.ZoneId
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.util.Locale

object Format {
    fun bytes(b: Long?): String {
        if (b == null) return "–"
        if (b < 1024) return "$b B"
        val units = listOf("KB", "MB", "GB", "TB")
        var v = b.toDouble() / 1024
        var i = 0
        while (v >= 1024 && i < units.lastIndex) { v /= 1024; i++ }
        return String.format(Locale.getDefault(), "%.1f %s", v, units[i])
    }

    /** Seconds → "m:ss" / "h:mm:ss". */
    fun duration(seconds: Double?): String {
        if (seconds == null) return ""
        val s = seconds.toLong()
        val h = s / 3600
        val m = (s % 3600) / 60
        val sec = s % 60
        return if (h > 0) String.format(Locale.ROOT, "%d:%02d:%02d", h, m, sec) else String.format(Locale.ROOT, "%d:%02d", m, sec)
    }

    /**
     * `localDateTime` is wall clock time encoded as UTC; parse it as UTC and keep the wall
     * clock values (no zone conversion).
     */
    fun localDateTime(iso: String?): LocalDateTime? = iso?.let {
        runCatching { LocalDateTime.ofInstant(Instant.parse(it), ZoneOffset.UTC) }.getOrNull()
            ?: runCatching { LocalDateTime.parse(it.removeSuffix("Z")) }.getOrNull()
            ?: runCatching { LocalDate.parse(it.take(10)).atStartOfDay() }.getOrNull()
    }

    fun dayKey(iso: String?): String = iso?.take(10) ?: ""

    fun monthTitle(timeBucket: String, locale: Locale = Locale.getDefault()): String = runCatching {
        LocalDate.parse(timeBucket.take(10)).format(DateTimeFormatter.ofPattern("LLLL yyyy", locale))
            .replaceFirstChar { it.titlecase(locale) }
    }.getOrDefault(timeBucket)

    fun dayTitle(day: String, locale: Locale = Locale.getDefault()): String = runCatching {
        LocalDate.parse(day).format(DateTimeFormatter.ofPattern("EEE, d. MMM yyyy", locale))
    }.getOrDefault(day)

    fun dateTime(iso: String?, locale: Locale = Locale.getDefault()): String {
        val ldt = localDateTime(iso) ?: return "–"
        return ldt.format(DateTimeFormatter.ofLocalizedDateTime(FormatStyle.MEDIUM).withLocale(locale))
    }

    /** RFC 3339 timestamp → localized date-time in device time zone. */
    fun instant(iso: String?, locale: Locale = Locale.getDefault()): String = iso?.let {
        runCatching {
            Instant.parse(it).atZone(ZoneId.systemDefault())
                .format(DateTimeFormatter.ofLocalizedDateTime(FormatStyle.MEDIUM).withLocale(locale))
        }.getOrNull()
    } ?: "–"

    fun epochMillis(ms: Long, locale: Locale = Locale.getDefault()): String =
        Instant.ofEpochMilli(ms).atZone(ZoneId.systemDefault())
            .format(DateTimeFormatter.ofLocalizedDateTime(FormatStyle.MEDIUM).withLocale(locale))

    fun exposure(raw: String?): String? {
        if (raw.isNullOrBlank()) return null
        val d = raw.toDoubleOrNull() ?: return "$raw s"
        return if (d in 0.0..0.5 && d > 0) "1/${Math.round(1 / d)} s" else "${d} s"
    }
}
