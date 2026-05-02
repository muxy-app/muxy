package com.muxy.terminal

enum class ArmedModifier(val displayName: String, val glyph: String) {
    CTRL("control", "⌃"),
    SHIFT("shift", "⇧"),
    ALT("option", "⌥"),
    CMD("command", "⌘"),
}

object ModifierTransform {
    private const val ESC: String = "\u001B"
    private const val NUL: String = "\u0000"

    fun transform(
        text: String,
        modifier: ArmedModifier,
    ): String? =
        when (modifier) {
            ArmedModifier.CTRL -> ctrlTransform(text)
            ArmedModifier.SHIFT -> text.uppercase()
            ArmedModifier.ALT -> ESC + text
            ArmedModifier.CMD -> text
        }

    private fun ctrlTransform(text: String): String? {
        if (text.length != 1) return null
        val value = text[0].code
        return when (value) {
            in 0x40..0x5F -> (value - 0x40).toChar().toString()
            in 0x61..0x7A -> (value - 0x60).toChar().toString()
            0x20 -> NUL
            else -> null
        }
    }
}
