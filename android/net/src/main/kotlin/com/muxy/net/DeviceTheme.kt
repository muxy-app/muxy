package com.muxy.net

data class DeviceTheme(
    val fg: UInt,
    val bg: UInt,
    val palette: List<UInt>,
) {
    val isDark: Boolean
        get() {
            val r = ((bg shr 16) and 0xFFu).toDouble() / 255.0
            val g = ((bg shr 8) and 0xFFu).toDouble() / 255.0
            val b = (bg and 0xFFu).toDouble() / 255.0
            val luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b
            return luminance < 0.5
        }
}
