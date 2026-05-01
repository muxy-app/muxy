package com.muxy.android.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable

private val MuxyDarkColors = darkColorScheme(
    primary = MuxyAccent,
    onPrimary = MuxyOnAccent,
    background = MuxyDarkBackground,
    onBackground = MuxyTextPrimary,
    surface = MuxyDarkSurface,
    onSurface = MuxyTextPrimary,
    surfaceVariant = MuxyDarkSurfaceVariant,
    onSurfaceVariant = MuxyTextSecondary,
    error = MuxyError,
)

@Composable
fun MuxyTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = MuxyDarkColors,
        typography = MuxyTypography,
        content = content,
    )
}
