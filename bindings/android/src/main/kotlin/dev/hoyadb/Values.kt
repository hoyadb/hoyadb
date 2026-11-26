// API-0011 / TC-0036 K01/K02: owned typed values; database semantics remain in Rust.
package dev.hoyadb

import java.nio.ByteBuffer
import java.nio.CharBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets
import java.util.Collections
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener

class HoyaException internal constructor(val kind: String) : Exception(kind) {
    override fun toString() = "HoyaException($kind)"
}

internal fun invalid(): Nothing = throw HoyaException("InvalidPayload")
internal fun invalidResponse(): Nothing = throw HoyaException("InvalidResponse")
internal fun limited(): Nothing = throw HoyaException("LimitExceeded")
internal fun <T> immutableList(values: List<T>): List<T> = Collections.unmodifiableList(ArrayList(values))
internal fun <T> immutableMap(values: Map<String, T>): Map<String, T> = Collections.unmodifiableMap(LinkedHashMap(values))

sealed class ProtocolValue {
    object Null : ProtocolValue()
    data class BooleanValue(val value: Boolean) : ProtocolValue()
    data class StringValue(val value: String) : ProtocolValue()
    class ArrayValue(values: List<ProtocolValue>) : ProtocolValue() {
        val value = immutableList(values)
        override fun equals(other: Any?) = other is ArrayValue && value == other.value
        override fun hashCode() = value.hashCode()
    }
    class ObjectValue(values: Map<String, ProtocolValue>) : ProtocolValue() {
        val value = immutableMap(values)
        override fun equals(other: Any?) = other is ObjectValue && value == other.value
        override fun hashCode() = value.hashCode()
    }
    final override fun toString() = "<ProtocolValue>"
}

internal fun obj(vararg fields: Pair<String, ProtocolValue>) = ProtocolValue.ObjectValue(fields.toMap())
internal fun str(value: String) = ProtocolValue.StringValue(value)
internal fun decimal(value: Long) = str(value.toString())
internal fun nullable(value: String?) = value?.let(::str) ?: ProtocolValue.Null
internal fun ProtocolValue.fields(): Map<String, ProtocolValue> = (this as? ProtocolValue.ObjectValue)?.value ?: invalidResponse()
internal fun ProtocolValue.items(): List<ProtocolValue> = (this as? ProtocolValue.ArrayValue)?.value ?: invalidResponse()
internal fun ProtocolValue.string(): String = (this as? ProtocolValue.StringValue)?.value ?: invalidResponse()
internal fun ProtocolValue.boolean(): Boolean = (this as? ProtocolValue.BooleanValue)?.value ?: invalidResponse()
internal fun ProtocolValue.exact(vararg names: String): Map<String, ProtocolValue> {
    val result = fields()
    if (result.keys != names.toSet()) invalidResponse()
    return result
}
internal fun ProtocolValue.signed(): Long {
    val text = string()
    val value = text.toLongOrNull() ?: invalidResponse()
    if (value.toString() != text) invalidResponse()
    return value
}
internal fun ProtocolValue.unsigned(): UnsignedDecimal = try { UnsignedDecimal.parse(string()) } catch (_: HoyaException) { invalidResponse() }
internal fun ProtocolValue.u32(): Long {
    val value = unsigned().value
    if (value.length > 10 || (value.length == 10 && value > "4294967295")) invalidResponse()
    return value.toLong()
}

class UnsignedDecimal private constructor(val value: String) {
    companion object {
        fun parse(value: String): UnsignedDecimal {
            if (value.isEmpty() || value.any { it !in '0'..'9' } || (value.length > 1 && value[0] == '0') ||
                value.length > 20 || (value.length == 20 && value > "18446744073709551615")) {
                throw HoyaException("InvalidRequest")
            }
            return UnsignedDecimal(value)
        }
        fun fromLong(value: Long): UnsignedDecimal {
            if (value < 0) throw HoyaException("InvalidRequest")
            return parse(value.toString())
        }
    }
    override fun equals(other: Any?) = other is UnsignedDecimal && value == other.value
    override fun hashCode() = value.hashCode()
    override fun toString() = "<UnsignedDecimal>"
}

sealed class Cell {
    object Null : Cell()
    data class BooleanValue(val value: Boolean) : Cell()
    data class StringValue(val value: String) : Cell()
    data class IntegerValue(val value: Long) : Cell()
    data class FloatValue(val value: Double) : Cell()
    data class TimestampValue(val value: Long) : Cell()
    data class ReferenceValue(val value: String) : Cell()
    final override fun toString() = "<Cell>"

    fun toProtocolValue(): ProtocolValue = when (this) {
        Null -> obj("kind" to str("null"))
        is BooleanValue -> tagged("boolean", ProtocolValue.BooleanValue(value))
        is StringValue -> tagged("string", str(value))
        is IntegerValue -> tagged("integer", decimal(value))
        is TimestampValue -> tagged("timestamp", decimal(value))
        is ReferenceValue -> tagged("reference", str(value))
        is FloatValue -> {
            if (!value.isFinite()) invalid()
            tagged("float", str(value.toString()))
        }
    }
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): Cell {
            val fields = wire.fields()
            val kind = fields["kind"]?.string() ?: invalidResponse()
            if (kind == "null") { wire.exact("kind"); return Null }
            wire.exact("kind", "value")
            val value = fields.getValue("value")
            return when (kind) {
                "boolean" -> BooleanValue(value.boolean())
                "string" -> StringValue(value.string())
                "integer" -> IntegerValue(value.signed())
                "timestamp" -> TimestampValue(value.signed())
                "reference" -> ReferenceValue(value.string())
                "float" -> {
                    val text = value.string()
                    if (!Regex("-?(0|[1-9][0-9]*)(\\.[0-9]+)?([eE][+-]?[0-9]+)?").matches(text)) invalidResponse()
                    val number = text.toDoubleOrNull() ?: invalidResponse()
                    if (!number.isFinite()) invalidResponse()
                    FloatValue(number)
                }
                else -> invalidResponse()
            }
        }
    }
}

internal fun tagged(kind: String, value: ProtocolValue) = obj("kind" to str(kind), "value" to value)
internal fun fieldsWire(fields: Map<String, Cell>) = ProtocolValue.ObjectValue(fields.mapValues { it.value.toProtocolValue() })
internal fun fieldsFrom(wire: ProtocolValue): Map<String, Cell> = immutableMap(wire.fields().mapValues { Cell.fromProtocolValue(it.value) })

sealed class JSONValue {
    object Null : JSONValue()
    data class BooleanValue(val value: Boolean) : JSONValue()
    data class StringValue(val value: String) : JSONValue()
    data class IntegerValue(val value: Long) : JSONValue()
    data class FloatValue(val value: Double) : JSONValue()
    class ArrayValue(values: List<JSONValue>) : JSONValue() {
        val value = immutableList(values)
        override fun equals(other: Any?) = other is ArrayValue && value == other.value
        override fun hashCode() = value.hashCode()
    }
    class ObjectValue(values: Map<String, JSONValue>) : JSONValue() {
        val value = immutableMap(values)
        override fun equals(other: Any?) = other is ObjectValue && value == other.value
        override fun hashCode() = value.hashCode()
    }
    final override fun toString() = "<JSONValue>"

    fun toProtocolValue(): ProtocolValue {
        var nodes = 0
        fun encode(value: JSONValue, depth: Int): ProtocolValue {
            if (depth > 64 || ++nodes > 100_000) limited()
            return when (value) {
                Null -> Cell.Null.toProtocolValue()
                is BooleanValue -> Cell.BooleanValue(value.value).toProtocolValue()
                is StringValue -> Cell.StringValue(value.value).toProtocolValue()
                is IntegerValue -> Cell.IntegerValue(value.value).toProtocolValue()
                is FloatValue -> Cell.FloatValue(value.value).toProtocolValue()
                is ArrayValue -> tagged("array", ProtocolValue.ArrayValue(value.value.map { encode(it, depth + 1) }))
                is ObjectValue -> tagged("object", ProtocolValue.ObjectValue(value.value.mapValues { encode(it.value, depth + 1) }))
            }
        }
        return encode(this, 0)
    }
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): JSONValue {
            val fields = wire.fields()
            return when (fields["kind"]?.string()) {
                "array" -> { wire.exact("kind", "value"); ArrayValue(fields.getValue("value").items().map(::fromProtocolValue)) }
                "object" -> { wire.exact("kind", "value"); ObjectValue(fields.getValue("value").fields().mapValues { fromProtocolValue(it.value) }) }
                else -> when (val cell = Cell.fromProtocolValue(wire)) {
                    Cell.Null -> Null
                    is Cell.BooleanValue -> BooleanValue(cell.value)
                    is Cell.StringValue -> StringValue(cell.value)
                    is Cell.IntegerValue -> IntegerValue(cell.value)
                    is Cell.FloatValue -> FloatValue(cell.value)
                    is Cell.TimestampValue, is Cell.ReferenceValue -> invalidResponse()
                }
            }
        }
    }
}

internal object WireCodec {
    private const val LIMIT = 1_048_576
    fun utf8(value: String): ByteArray {
        try {
            val bytes = StandardCharsets.UTF_8.newEncoder().onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT).encode(CharBuffer.wrap(value))
            return ByteArray(bytes.remaining()).also { bytes.get(it) }
        } catch (_: CharacterCodingException) { invalid() }
    }
    fun utf8(bytes: ByteArray): String = try {
        StandardCharsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(bytes)).toString()
    } catch (_: CharacterCodingException) { invalid() }

    fun encode(value: ProtocolValue): ByteArray {
        val output = StringBuilder()
        var nodes = 0
        fun quoted(value: String) {
            if (value.length > LIMIT) limited()
            utf8(value)
            output.append(JSONObject.quote(value))
        }
        fun write(value: ProtocolValue, depth: Int) {
            if (depth > 64 || ++nodes > 100_000) limited()
            when (value) {
                ProtocolValue.Null -> output.append("null")
                is ProtocolValue.BooleanValue -> output.append(if (value.value) "true" else "false")
                is ProtocolValue.StringValue -> quoted(value.value)
                is ProtocolValue.ArrayValue -> {
                    output.append('[')
                    value.value.forEachIndexed { index, child -> if (index > 0) output.append(','); write(child, depth + 1) }
                    output.append(']')
                }
                is ProtocolValue.ObjectValue -> {
                    output.append('{')
                    value.value.toSortedMap().entries.forEachIndexed { index, field ->
                        if (index > 0) output.append(',')
                        quoted(field.key); output.append(':'); write(field.value, depth + 1)
                    }
                    output.append('}')
                }
            }
            if (output.length > LIMIT) limited()
        }
        write(value, 0)
        return utf8(output.toString()).also { if (it.size > LIMIT) limited() }
    }

    // Only owned, trusted ABI replies reach Android's platform JSON parser.
    // Opaque server/business JSON is decoded by Rust's json_decode operation.
    fun decode(bytes: ByteArray): ProtocolValue {
        if (bytes.size > LIMIT) limited()
        val text = utf8(bytes)
        var depth = 0
        var quoted = false
        var escaped = false
        for (character in text) {
            if (quoted) {
                if (escaped) escaped = false else if (character == '\\') escaped = true else if (character == '"') quoted = false
            } else if (character == '"') quoted = true else if (character == '[' || character == '{') {
                if (++depth > 65) limited()
            } else if (character == ']' || character == '}') depth--
        }
        try {
            val parser = JSONTokener(text)
            val root = parser.nextValue()
            if (parser.nextClean() != '\u0000') invalid()
            var nodes = 0
            fun convert(value: Any?, depth: Int): ProtocolValue {
                if (depth > 64 || ++nodes > 100_000) limited()
                return when (value) {
                    null, JSONObject.NULL -> ProtocolValue.Null
                    is Boolean -> ProtocolValue.BooleanValue(value)
                    is String -> { utf8(value); str(value) }
                    is JSONArray -> ProtocolValue.ArrayValue((0 until value.length()).map { convert(value.get(it), depth + 1) })
                    is JSONObject -> {
                        val fields = LinkedHashMap<String, ProtocolValue>()
                        val keys = value.keys()
                        while (keys.hasNext()) { val key = keys.next(); utf8(key); fields[key] = convert(value.get(key), depth + 1) }
                        ProtocolValue.ObjectValue(fields)
                    }
                    else -> invalid()
                }
            }
            return convert(root, 0)
        } catch (error: HoyaException) { throw error }
        catch (_: Exception) { invalid() }
    }
}
