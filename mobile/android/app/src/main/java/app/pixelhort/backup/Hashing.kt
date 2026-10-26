package app.pixelhort.backup

import java.io.InputStream
import java.security.MessageDigest

object Hashing {
    /** Streams [input] through SHA-1 and returns the lowercase hex digest. Closes the stream. */
    fun sha1Hex(input: InputStream, bufferSize: Int = 256 * 1024, onProgress: ((Long) -> Unit)? = null): String {
        val md = MessageDigest.getInstance("SHA-1")
        input.use { stream ->
            val buf = ByteArray(bufferSize)
            var total = 0L
            while (true) {
                val n = stream.read(buf)
                if (n < 0) break
                md.update(buf, 0, n)
                total += n
                onProgress?.invoke(total)
            }
        }
        return toHex(md.digest())
    }

    fun toHex(bytes: ByteArray): String {
        val sb = StringBuilder(bytes.size * 2)
        for (b in bytes) {
            val v = b.toInt() and 0xff
            sb.append(HEX[v ushr 4]).append(HEX[v and 0x0f])
        }
        return sb.toString()
    }

    private val HEX = "0123456789abcdef".toCharArray()
}
