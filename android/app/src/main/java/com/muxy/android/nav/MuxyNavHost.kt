package com.muxy.android.nav

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.muxy.android.connect.ConnectScreen

internal object MuxyRoutes {
    const val CONNECT = "connect"
}

@Composable
fun MuxyNavHost(modifier: Modifier = Modifier) {
    val navController = rememberNavController()
    NavHost(
        navController = navController,
        startDestination = MuxyRoutes.CONNECT,
        modifier = modifier,
    ) {
        composable(MuxyRoutes.CONNECT) { ConnectScreen() }
    }
}
