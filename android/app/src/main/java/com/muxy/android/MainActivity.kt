package com.muxy.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import com.muxy.android.nav.MuxyNavHost
import com.muxy.android.ui.theme.MuxyTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent { MuxyRoot() }
    }
}

@Composable
private fun MuxyRoot() {
    val container = (LocalContext.current.applicationContext as MuxyApp).container
    CompositionLocalProvider(LocalAppContainer provides container) {
        MuxyTheme {
            Scaffold(modifier = Modifier.fillMaxSize()) { padding ->
                MuxyNavHost(modifier = Modifier.padding(padding))
            }
        }
    }
}
