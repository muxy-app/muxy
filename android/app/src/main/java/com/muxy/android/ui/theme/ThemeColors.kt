package com.muxy.android.ui.theme

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.ui.graphics.Color
import com.muxy.net.DeviceTheme

@Stable
data class MuxyColors(
    val foreground: Color,
    val background: Color,
    val isDark: Boolean,
) {
    val cardBackground: Color get() = foreground.copy(alpha = 0.06f)
    val mutedForeground: Color get() = foreground.copy(alpha = 0.7f)
    val faintForeground: Color get() = foreground.copy(alpha = 0.5f)
    val outline: Color get() = foreground.copy(alpha = 0.4f)
}

@Composable
fun muxyColors(theme: DeviceTheme?, fallbackDark: Boolean = true): MuxyColors {
    if (theme == null) {
        return if (fallbackDark) {
            MuxyColors(foreground = Color.White, background = Color.Black, isDark = true)
        } else {
            MuxyColors(foreground = Color.Black, background = Color.White, isDark = false)
        }
    }
    return MuxyColors(
        foreground = rgbColor(theme.fg),
        background = rgbColor(theme.bg),
        isDark = theme.isDark,
    )
}

private fun rgbColor(rgb: UInt): Color {
    val r = ((rgb shr 16) and 0xFFu).toInt()
    val g = ((rgb shr 8) and 0xFFu).toInt()
    val b = (rgb and 0xFFu).toInt()
    return Color(red = r, green = g, blue = b)
}
