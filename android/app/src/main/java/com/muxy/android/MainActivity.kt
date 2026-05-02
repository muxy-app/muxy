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
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.lifecycle.lifecycleScope
import com.muxy.android.nav.MuxyNavHost
import com.muxy.android.ui.theme.MuxyTheme
import com.muxy.net.ConnectionState
import com.muxy.net.ConnectionTarget
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        installSplashScreen()
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        attemptColdStartRestore(savedInstanceState)
        setContent { MuxyRoot() }
    }

    private fun attemptColdStartRestore(savedInstanceState: Bundle?) {
        if (savedInstanceState != null) return
        val app = applicationContext as MuxyApp
        val client = app.container.muxyClient
        if (client.state.value !is ConnectionState.Idle) return

        lifecycleScope.launch {
            val last = app.container.lastSessionStore.flow.first() ?: return@launch
            if (client.state.value !is ConnectionState.Idle) return@launch
            client.connect(
                ConnectionTarget(host = last.host, port = last.port, deviceName = last.deviceName),
            )
        }
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
