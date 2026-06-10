package rs.koffi

object KoffiSerializer {
    private const val MAGIC: UShort = 0x4B46u
    private const val VERSION: UShort = 0x0001u

    fun <T> serialize(
        value: T,
        writer: KoffiWriter.(T) -> Unit,
        typeHash: ULong = 0uL,
    ): ByteArray {
        val w = KoffiWriter()
        w.writeUShort(MAGIC)        // 0x46, 0x4B (LE)
        w.writeUShort(VERSION)      // 0x01, 0x00 (LE)
        w.writeInt(0)               // 4 bytes padding
        w.writeULong(typeHash)      // 8 bytes, LE
        w.writer(value)
        return w.toByteArray()
    }

    fun <T> deserialize(
        bytes: ByteArray,
        expectedHash: ULong = 0uL,
        reader: KoffiReader.() -> T,
    ): T {
        val r = KoffiReader(bytes)
        val magic = r.readUShort()
        val version = r.readUShort()
        r.skipBytes(4)                 // padding
        val hash = r.readULong()

        check(magic == MAGIC) {
            "koffi: bad envelope magic 0x${magic.toString(16)}"
        }
        check(version == VERSION) {
            "koffi: unsupported envelope version $version"
        }
        if (expectedHash != 0uL && hash != expectedHash) {
            throw KoffiSchemaMismatch(expectedHash, hash)
        }
        return r.reader()
    }
}

class KoffiSchemaMismatch(expected: ULong, actual: ULong) : KoffiError() {
    override val message = "Schema mismatch: expected 0x${expected.toString(16)}, " +
            "got 0x${actual.toString(16)}. Regenerate bindings with koffi."
}

/**
 * A binary writer utilizing the Postcard binary format spec.
 */
class KoffiWriter(initialCapacity: Int = 128) {
    private var buffer = ByteArray(initialCapacity)
    var size = 0
        private set

    private fun ensureCapacity(extra: Int) {
        if (size + extra > buffer.size) {
            val newCap = (buffer.size * 2).coerceAtLeast(size + extra)
            buffer = buffer.copyOf(newCap)
        }
    }

    fun writeByte(v: Byte) {
        ensureCapacity(1)
        buffer[size++] = v
    }

    fun writeBytes(v: ByteArray) {
        ensureCapacity(v.size)
        v.copyInto(buffer, size)
        size += v.size
    }

    fun writeBool(v: Boolean) {
        writeByte(if (v) 1 else 0)
    }

    fun writeULong(v: ULong) {
        var value = v
        while (value >= 0x80u) {
            writeByte(((value and 0x7Fu) or 0x80u).toByte())
            value = value shr 7
        }
        writeByte((value and 0x7Fu).toByte())
    }

    fun writeLong(v: Long) {
        writeULong(((v shl 1) xor (v shr 63)).toULong())
    }

    fun writeUInt(v: UInt) = writeULong(v.toULong())
    fun writeInt(v: Int) = writeLong(v.toLong())
    fun writeUShort(v: UShort) = writeULong(v.toULong())
    fun writeShort(v: Short) = writeLong(v.toLong())
    fun writeUByte(v: UByte) = writeULong(v.toULong())
    fun writeByteVal(v: Byte) = writeLong(v.toLong())

    fun writeFloat(v: Float) {
        val bits = v.toRawBits()
        writeByte((bits and 0xFF).toByte())
        writeByte(((bits shr 8) and 0xFF).toByte())
        writeByte(((bits shr 16) and 0xFF).toByte())
        writeByte(((bits shr 24) and 0xFF).toByte())
    }

    fun writeDouble(v: Double) {
        val bits = v.toRawBits()
        for (i in 0..7) {
            writeByte(((bits shr (i * 8)) and 0xFF).toByte())
        }
    }

    fun writeString(v: String) {
        val bytes = v.encodeToByteArray()
        writeULong(bytes.size.toULong())
        writeBytes(bytes)
    }

    fun writeByteArray(v: ByteArray) {
        writeULong(v.size.toULong())
        writeBytes(v)
    }

    fun <T> writeOption(v: T?, writeItem: (T) -> Unit) {
        if (v == null) {
            writeByte(0)
        } else {
            writeByte(1)
            writeItem(v)
        }
    }

    fun <T> writeList(v: List<T>, writeItem: (T) -> Unit) {
        writeULong(v.size.toULong())
        for (item in v) {
            writeItem(item)
        }
    }

    fun toByteArray(): ByteArray = buffer.copyOf(size)
}

/**
 * A binary reader utilizing the Postcard binary format spec.
 */
class KoffiReader(private val buffer: ByteArray) {
    private var position = 0

    fun hasMore(): Boolean = position < buffer.size

    fun readByte(): Byte {
        if (position >= buffer.size) throw Exception("Unexpected EOF in KoffiReader")
        return buffer[position++]
    }

    fun readBytes(length: Int): ByteArray {
        if (position + length > buffer.size) throw Exception("Unexpected EOF in KoffiReader")
        val res = buffer.copyOfRange(position, position + length)
        position += length
        return res
    }

    fun readBool(): Boolean = readByte() != 0.toByte()

    fun readULong(): ULong {
        var result = 0uL
        var shift = 0
        while (true) {
            val b = readByte().toUByte().toULong()
            result = result or ((b and 0x7Fu) shl shift)
            if ((b and 0x80u) == 0uL) break
            shift += 7
            if (shift >= 64) throw Exception("Varint too long")
        }
        return result
    }

    fun readLong(): Long {
        val u = readULong().toLong()
        return (u ushr 1) xor -(u and 1)
    }

    fun readUInt(): UInt = readULong().toUInt()
    fun readInt(): Int = readLong().toInt()
    fun readUShort(): UShort = readULong().toUShort()
    fun readShort(): Short = readLong().toShort()
    fun readUByte(): UByte = readULong().toUByte()
    fun readByteVal(): Byte = readLong().toByte()

    fun readFloat(): Float {
        val b0 = readByte().toInt() and 0xFF
        val b1 = readByte().toInt() and 0xFF
        val b2 = readByte().toInt() and 0xFF
        val b3 = readByte().toInt() and 0xFF
        val bits = b0 or (b1 shl 8) or (b2 shl 16) or (b3 shl 24)
        return Float.fromBits(bits)
    }

    fun readDouble(): Double {
        var bits = 0L
        for (i in 0..7) {
            val b = readByte().toLong() and 0xFF
            bits = bits or (b shl (i * 8))
        }
        return Double.fromBits(bits)
    }

    fun readString(): String {
        val len = readULong().toInt()
        val bytes = readBytes(len)
        return bytes.decodeToString()
    }

    fun readByteArray(): ByteArray {
        val len = readULong().toInt()
        return readBytes(len)
    }

    fun <T> readOption(readItem: () -> T): T? {
        val tag = readByte()
        return if (tag == 0.toByte()) {
            null
        } else {
            readItem()
        }
    }

    fun <T> readList(readItem: () -> T): List<T> {
        val len = readULong().toInt()
        val list = ArrayList<T>(len)
        for (i in 0 until len) {
            list.add(readItem())
        }
        return list
    }

    fun skipBytes(count: Int) {
        if (position + count > buffer.size) throw Exception("Unexpected EOF in KoffiReader")
        position += count
    }
}
