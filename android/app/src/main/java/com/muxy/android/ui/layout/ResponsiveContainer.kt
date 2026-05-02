package com.muxy.android.ui.layout

import android.app.Activity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.windowsizeclass.ExperimentalMaterial3WindowSizeClassApi
import androidx.compose.material3.windowsizeclass.WindowSizeClass
import androidx.compose.material3.windowsizeclass.WindowWidthSizeClass
import androidx.compose.material3.windowsizeclass.calculateWindowSizeClass
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp

@OptIn(ExperimentalMaterial3WindowSizeClassApi::class)
@Composable
fun rememberWindowSizeClass(): WindowSizeClass? {
    val activity = (LocalContext.current as? Activity) ?: return null
    return calculateWindowSizeClass(activity)
}

@Composable
fun ResponsiveContent(
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    val sizeClass = rememberWindowSizeClass()
    val maxWidth =
        when (sizeClass?.widthSizeClass) {
            WindowWidthSizeClass.Compact, null -> Modifier.fillMaxWidth()
            WindowWidthSizeClass.Medium -> Modifier.widthIn(max = 600.dp)
            else -> Modifier.widthIn(max = 760.dp)
        }
    Box(modifier = modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
        Box(modifier = maxWidth.fillMaxSize()) { content() }
    }
}
