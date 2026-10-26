package app.pixelhort

import app.pixelhort.backup.Hashing
import app.pixelhort.data.api.AssetDto
import app.pixelhort.data.api.BulkCheckResponse
import app.pixelhort.data.api.PixelhortJson
import app.pixelhort.data.settings.SettingsStore
import app.pixelhort.ui.components.Format
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.Locale

class UtilTest {

    @Test
    fun sha1MatchesKnownVector() {
        assertEquals("a9993e364706816aba3e25717850c26c9cd0d89d", Hashing.sha1Hex("abc".byteInputStream()))
        assertEquals("da39a3ee5e6b4b0d3255bfef95601890afd80709", Hashing.sha1Hex(ByteArray(0).inputStream()))
    }

    @Test
    fun sha1IsStreamedAcrossBuffers() {
        val data = ByteArray(1_000_003) { (it % 251).toByte() }
        val small = Hashing.sha1Hex(data.inputStream(), bufferSize = 7)
        val big = Hashing.sha1Hex(data.inputStream())
        assertEquals(big, small)
    }

    @Test
    fun normalizesServerUrls() {
        assertEquals("https://photos.example.com", SettingsStore.normalizeServerUrl(" photos.example.com/ "))
        assertEquals("http://192.168.1.2:2342", SettingsStore.normalizeServerUrl("http://192.168.1.2:2342/api"))
        assertEquals("https://h/sub", SettingsStore.normalizeServerUrl("https://h/sub/"))
    }

    @Test
    fun formatsDurationsAndBytes() {
        assertEquals("1:05", Format.duration(65.4))
        assertEquals("1:01:01", Format.duration(3661.0))
        assertEquals("512 B", Format.bytes(512))
        assertTrue(Format.bytes(5L * 1024 * 1024).startsWith("5"))
    }

    @Test
    fun localDateTimeKeepsWallClock() {
        val ldt = Format.localDateTime("2024-05-01T23:30:00Z")!!
        assertEquals(23, ldt.hour)
        assertEquals("2024-05-01", Format.dayKey("2024-05-01T23:30:00Z"))
        assertEquals("Mai 2024", Format.monthTitle("2024-05-01", Locale.GERMAN))
    }

    @Test
    fun parsesAssetJsonLeniently() {
        val json = """{"id":"a","type":"VIDEO","duration":12.5,"unknownField":1,"exif":{"iso":200,"exposureTime":0.004,"fNumber":1.8},
            |"livePhotoVideoId":null,"people":[{"id":"p","name":"Anna"}]}""".trimMargin()
        val a = PixelhortJson.decodeFromString(AssetDto.serializer(), json)
        assertTrue(a.isVideo)
        assertEquals(200.0, a.exif!!.iso!!, 0.0)
        assertEquals("1/250 s", Format.exposure(a.exif!!.exposureTime))
        assertNull(a.livePhotoVideoId)
        assertEquals("Anna", a.people.single().name)
    }

    @Test
    fun parsesBulkCheck() {
        val r = PixelhortJson.decodeFromString(
            BulkCheckResponse.serializer(),
            """{"results":[{"id":"1","action":"reject","reason":"duplicate","assetId":"x"},{"id":"2","action":"accept"}]}""",
        )
        assertEquals(listOf("reject", "accept"), r.results.map { it.action })
    }
}
