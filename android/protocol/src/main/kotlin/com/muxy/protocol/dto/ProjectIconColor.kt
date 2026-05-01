package com.muxy.protocol.dto

object ProjectIconColor {
    data class Swatch(val id: String, val name: String, val hex: String) {
        val prefersDarkForeground: Boolean
            get() {
                val rgb = rgb(fromHex = hex) ?: return false
                val luminance = 0.2126 * rgb.first + 0.7152 * rgb.second + 0.0722 * rgb.third
                return luminance > 0.6
            }
    }

    val palette: List<Swatch> = listOf(
        Swatch("red", "Red", "#E5484D"),
        Swatch("orange", "Orange", "#F76B15"),
        Swatch("amber", "Amber", "#F5A623"),
        Swatch("yellow", "Yellow", "#EBCB00"),
        Swatch("lime", "Lime", "#9BCD1E"),
        Swatch("green", "Green", "#30A46C"),
        Swatch("teal", "Teal", "#12A594"),
        Swatch("cyan", "Cyan", "#05A2C2"),
        Swatch("blue", "Blue", "#3E63DD"),
        Swatch("indigo", "Indigo", "#5B5BD6"),
        Swatch("violet", "Violet", "#8E4EC6"),
        Swatch("pink", "Pink", "#D6409F"),
    )

    private val byID: Map<String, Swatch> = palette.associateBy { it.id }

    fun swatch(forIdentifier: String?): Swatch? {
        if (forIdentifier == null) return null
        byID[forIdentifier]?.let { return it }
        return palette.firstOrNull { it.hex.equals(forIdentifier, ignoreCase = true) }
    }

    fun rgb(fromHex: String): Triple<Double, Double, Double>? {
        var normalized = fromHex.trim()
        if (normalized.startsWith("#")) normalized = normalized.removePrefix("#")
        if (normalized.length != 6) return null
        val value = normalized.toLongOrNull(radix = 16) ?: return null
        val red = ((value shr 16) and 0xFF).toDouble() / 255.0
        val green = ((value shr 8) and 0xFF).toDouble() / 255.0
        val blue = (value and 0xFF).toDouble() / 255.0
        return Triple(red, green, blue)
    }
}
