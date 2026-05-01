package com.muxy.protocol.dto

import com.muxy.protocol.codec.Base64ByteArraySerializer
import kotlinx.serialization.Contextual
import kotlinx.serialization.EncodeDefault
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.Serializable
import java.util.UUID

@OptIn(ExperimentalSerializationApi::class)
@Serializable
data class TerminalContentDTO(
    val paneID: @Contextual UUID,
    val content: String,
    val cols: UInt,
    val rows: UInt,
)

@Serializable
data class TerminalCellDTO(
    val codepoint: UInt,
    val fg: UInt,
    val bg: UInt,
    val flags: UShort,
)

@OptIn(ExperimentalSerializationApi::class)
@Serializable
data class TerminalCellsDTO(
    val paneID: @Contextual UUID,
    val cols: UInt,
    val rows: UInt,
    val cursorX: UInt,
    val cursorY: UInt,
    val cursorVisible: Boolean,
    val defaultFg: UInt,
    val defaultBg: UInt,
    val cells: List<TerminalCellDTO>,
    @EncodeDefault(EncodeDefault.Mode.ALWAYS) val altScreen: Boolean = false,
    @EncodeDefault(EncodeDefault.Mode.ALWAYS) val cursorKeys: Boolean = false,
    @EncodeDefault(EncodeDefault.Mode.ALWAYS) val bracketedPaste: Boolean = false,
    @EncodeDefault(EncodeDefault.Mode.ALWAYS) val focusEvent: Boolean = false,
    @EncodeDefault(EncodeDefault.Mode.ALWAYS) val mouseEvent: UShort = 0u,
    @EncodeDefault(EncodeDefault.Mode.ALWAYS) val mouseFormat: UShort = 0u,
)

object TerminalCellFlag {
    val BOLD: UShort = 1u.toUShort()
    val ITALIC: UShort = 2u.toUShort()
    val FAINT: UShort = 4u.toUShort()
    val BLINK: UShort = 8u.toUShort()
    val INVERSE: UShort = 16u.toUShort()
    val INVISIBLE: UShort = 32u.toUShort()
    val STRIKE: UShort = 64u.toUShort()
    val UNDERLINE: UShort = 128u.toUShort()
    val OVERLINE: UShort = 256u.toUShort()
    val WIDE: UShort = 512u.toUShort()
    val SPACER: UShort = 1024u.toUShort()
}

@Serializable
data class TerminalOutputEventDTO(
    val paneID: @Contextual UUID,
    val bytes: @Serializable(with = Base64ByteArraySerializer::class) ByteArray,
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is TerminalOutputEventDTO) return false
        return paneID == other.paneID && bytes.contentEquals(other.bytes)
    }

    override fun hashCode(): Int {
        var result = paneID.hashCode()
        result = 31 * result + bytes.contentHashCode()
        return result
    }
}
