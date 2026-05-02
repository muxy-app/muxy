package com.muxy.android.nav

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.muxy.android.LocalAppContainer
import com.muxy.android.connect.ConnectScreen
import com.muxy.android.connect.ConnectingView
import com.muxy.android.connect.ConnectionFailedView
import com.muxy.android.connect.PairingPendingView
import com.muxy.android.notifications.NotificationsScreen
import com.muxy.android.projects.ProjectListScreen
import com.muxy.android.settings.SettingsScreen
import com.muxy.android.workspace.WorkspaceScreen
import com.muxy.net.ConnectionState
import java.util.UUID

internal object MuxyRoutes {
    const val CONNECT = "connect"
    const val SETTINGS = "settings"
    const val PROJECTS = "projects"
    const val WORKSPACE = "workspace/{projectID}"
    const val WORKSPACE_ARG = "projectID"
    const val NOTIFICATIONS = "notifications"

    fun workspace(projectID: UUID): String = "workspace/$projectID"
}

@Composable
fun MuxyNavHost(modifier: Modifier = Modifier) {
    val container = LocalAppContainer.current
    val state by container.muxyClient.state.collectAsStateWithLifecycle()

    when (val current = state) {
        is ConnectionState.Idle -> ConnectFlow(modifier = modifier)
        is ConnectionState.Connecting ->
            ConnectingView(
                deviceName = current.target.deviceName,
                host = current.target.host,
                port = current.target.port,
                onCancel = container.muxyClient::disconnect,
                modifier = modifier,
            )
        is ConnectionState.Authenticating ->
            ConnectingView(
                deviceName = current.target.deviceName,
                host = current.target.host,
                port = current.target.port,
                onCancel = container.muxyClient::disconnect,
                modifier = modifier,
            )
        is ConnectionState.AwaitingApproval ->
            PairingPendingView(
                deviceName = current.target.deviceName,
                onCancel = container.muxyClient::disconnect,
                modifier = modifier,
            )
        is ConnectionState.Connected, is ConnectionState.Reconnecting -> ConnectedFlow(modifier = modifier)
        is ConnectionState.Failed ->
            ConnectionFailedView(
                issue = current.issue,
                onRetry = {
                    val target = current.target
                    if (target != null) container.muxyClient.connect(target)
                },
                onDisconnect = container.muxyClient::disconnect,
                modifier = modifier,
            )
    }
}

@Composable
private fun ConnectFlow(modifier: Modifier = Modifier) {
    val navController = rememberNavController()
    NavHost(
        navController = navController,
        startDestination = MuxyRoutes.CONNECT,
        modifier = modifier,
    ) {
        composable(MuxyRoutes.CONNECT) {
            ConnectScreen(
                onOpenSettings = { navController.navigate(MuxyRoutes.SETTINGS) },
            )
        }
        composable(MuxyRoutes.SETTINGS) {
            SettingsScreen(onBack = { navController.popBackStack() })
        }
    }
}

@Composable
private fun ConnectedFlow(modifier: Modifier = Modifier) {
    val container = LocalAppContainer.current
    val navController = rememberNavController()
    NavHost(
        navController = navController,
        startDestination = MuxyRoutes.PROJECTS,
        modifier = modifier,
    ) {
        composable(MuxyRoutes.PROJECTS) {
            ProjectListScreen(
                onProjectSelected = { id -> navController.navigate(MuxyRoutes.workspace(id)) },
                onOpenNotifications = { navController.navigate(MuxyRoutes.NOTIFICATIONS) },
                onDisconnect = container.muxyClient::disconnect,
            )
        }
        composable(MuxyRoutes.WORKSPACE) { backStack ->
            val raw = backStack.arguments?.getString(MuxyRoutes.WORKSPACE_ARG)
            val projectID = runCatching { UUID.fromString(raw) }.getOrNull()
            if (projectID == null) {
                navController.popBackStack()
                return@composable
            }
            WorkspaceScreen(
                projectID = projectID,
                onBack = { navController.popBackStack() },
                onOpenNotifications = { navController.navigate(MuxyRoutes.NOTIFICATIONS) },
            )
        }
        composable(MuxyRoutes.NOTIFICATIONS) {
            NotificationsScreen(
                onBack = { navController.popBackStack() },
                onNavigateToProject = { id ->
                    navController.navigate(MuxyRoutes.workspace(id)) {
                        popUpTo(MuxyRoutes.PROJECTS)
                    }
                },
            )
        }
    }
}
