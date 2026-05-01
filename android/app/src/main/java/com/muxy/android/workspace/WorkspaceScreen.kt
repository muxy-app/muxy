package com.muxy.android.workspace

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.outlined.AccountTree
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material.icons.outlined.Notifications
import androidx.compose.material.icons.outlined.Refresh
import androidx.compose.material.icons.outlined.Terminal
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.muxy.android.LocalAppContainer
import com.muxy.android.ui.theme.muxyColors
import com.muxy.android.vcs.VCSSheet
import com.muxy.protocol.dto.SplitNodeDTO
import com.muxy.protocol.dto.TabAreaDTO
import com.muxy.protocol.dto.TabDTO
import com.muxy.protocol.dto.TabKindDTO
import com.muxy.protocol.dto.WorkspaceDTO
import com.muxy.terminal.MuxyTerminalView
import kotlinx.coroutines.launch
import java.util.UUID

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WorkspaceScreen(
    projectID: UUID,
    onBack: () -> Unit,
    onOpenNotifications: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val container = LocalAppContainer.current
    val client = container.muxyClient
    val scope = rememberCoroutineScope()

    val workspace by client.workspace.collectAsStateWithLifecycle()
    val activeProjectID by client.activeProjectID.collectAsStateWithLifecycle()
    val projects by client.projects.collectAsStateWithLifecycle()
    val theme by client.deviceTheme.collectAsStateWithLifecycle()
    val notifications by client.notifications.collectAsStateWithLifecycle()
    val unreadCount = notifications.count { !it.isRead }
    val colors = muxyColors(theme)

    var showVCS by remember { mutableStateOf(false) }
    var showTabPicker by remember { mutableStateOf(false) }

    LaunchedEffect(projectID, activeProjectID) {
        if (activeProjectID != projectID) {
            client.selectProject(projectID)
        } else if (workspace == null) {
            client.refreshWorkspace(projectID)
        }
    }

    val activeProject = remember(projects, projectID) { projects.firstOrNull { it.id == projectID } }
    val tabsList = remember(workspace) { workspace?.let { collectTabsByArea(it) } ?: emptyList() }
    val active = remember(workspace) { workspace?.let { activeAreaTab(it) } }

    Scaffold(
        modifier = modifier.fillMaxSize(),
        containerColor = colors.background,
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        text = activeProject?.name ?: "Workspace",
                        style = MaterialTheme.typography.titleMedium,
                        color = colors.foreground,
                    )
                },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            Icons.AutoMirrored.Outlined.ArrowBack,
                            contentDescription = "Back",
                            tint = colors.foreground,
                        )
                    }
                },
                actions = {
                    IconButton(onClick = onOpenNotifications) {
                        BadgedBox(
                            badge = {
                                if (unreadCount > 0) {
                                    Badge { Text(text = if (unreadCount > 99) "99+" else "$unreadCount") }
                                }
                            },
                        ) {
                            Icon(
                                Icons.Outlined.Notifications,
                                contentDescription = "Notifications",
                                tint = colors.foreground,
                            )
                        }
                    }
                    IconButton(
                        onClick = { showVCS = true },
                    ) {
                        Icon(
                            Icons.Outlined.AccountTree,
                            contentDescription = "Source Control",
                            tint = colors.foreground,
                        )
                    }
                    IconButton(
                        onClick = { scope.launch { client.refreshWorkspace(projectID) } },
                    ) {
                        Icon(
                            Icons.Outlined.Refresh,
                            contentDescription = "Refresh",
                            tint = colors.foreground,
                        )
                    }
                    Box {
                        IconButton(onClick = { showTabPicker = true }) {
                            Icon(
                                Icons.Outlined.Terminal,
                                contentDescription = "Tabs",
                                tint = colors.foreground,
                            )
                        }
                        TabPickerMenu(
                            expanded = showTabPicker,
                            onDismiss = { showTabPicker = false },
                            entries = tabsList,
                            activeTabID = active?.tab?.id,
                            onTabSelected = { entry ->
                                showTabPicker = false
                                scope.launch {
                                    client.selectTab(
                                        projectID = projectID,
                                        areaID = entry.area.id,
                                        tabID = entry.tab.id,
                                    )
                                }
                            },
                            onNewTerminal = {
                                showTabPicker = false
                                scope.launch { client.createTab(projectID = projectID) }
                            },
                        )
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = colors.background),
            )
        },
    ) { padding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .background(colors.background),
        ) {
            when {
                workspace == null -> WorkspaceLoading(modifier = Modifier.fillMaxSize())
                active == null -> EmptyTabsState(
                    foreground = colors.foreground,
                    onCreate = { scope.launch { client.createTab(projectID = projectID) } },
                )
                else -> ActiveTabContent(
                    tab = active.tab,
                    foreground = colors.foreground,
                    background = colors.background,
                )
            }
        }
    }

    if (showVCS) {
        VCSSheet(
            projectID = projectID,
            onDismiss = { showVCS = false },
        )
    }
}

private data class AreaTab(val area: TabAreaDTO, val tab: TabDTO)

private fun collectTabsByArea(workspace: WorkspaceDTO): List<AreaTab> =
    collectAreas(workspace.root).flatMap { area ->
        area.tabs.map { AreaTab(area = area, tab = it) }
    }

private fun activeAreaTab(workspace: WorkspaceDTO): AreaTab? {
    val areas = collectAreas(workspace.root)
    val focused = areas.firstOrNull { it.id == workspace.focusedAreaID } ?: areas.firstOrNull() ?: return null
    val activeID = focused.activeTabID ?: return null
    val tab = focused.tabs.firstOrNull { it.id == activeID } ?: return null
    return AreaTab(area = focused, tab = tab)
}

private fun collectAreas(node: SplitNodeDTO): List<TabAreaDTO> = when (node) {
    is SplitNodeDTO.TabArea -> listOf(node.tabArea)
    is SplitNodeDTO.Split -> collectAreas(node.split.first) + collectAreas(node.split.second)
}

@Composable
private fun TabPickerMenu(
    expanded: Boolean,
    onDismiss: () -> Unit,
    entries: List<AreaTab>,
    activeTabID: UUID?,
    onTabSelected: (AreaTab) -> Unit,
    onNewTerminal: () -> Unit,
) {
    DropdownMenu(expanded = expanded, onDismissRequest = onDismiss) {
        entries.forEach { entry ->
            DropdownMenuItem(
                text = { Text(shortTitle(entry.tab.title)) },
                leadingIcon = {
                    if (entry.tab.id == activeTabID) {
                        Icon(Icons.Outlined.Check, contentDescription = null)
                    } else {
                        Spacer(Modifier.size(24.dp))
                    }
                },
                onClick = { onTabSelected(entry) },
            )
        }
        if (entries.isNotEmpty()) HorizontalDivider()
        DropdownMenuItem(
            text = { Text("New Terminal") },
            leadingIcon = { Icon(Icons.Outlined.Add, contentDescription = null) },
            onClick = onNewTerminal,
        )
    }
}

private fun shortTitle(title: String): String {
    val parts = title.split('/').filter { it.isNotEmpty() }
    return parts.lastOrNull() ?: title
}

@Composable
private fun ActiveTabContent(
    tab: TabDTO,
    foreground: androidx.compose.ui.graphics.Color,
    background: androidx.compose.ui.graphics.Color,
) {
    val container = LocalAppContainer.current
    when (tab.kind) {
        TabKindDTO.TERMINAL -> {
            val paneID = tab.paneID
            if (paneID == null) {
                NonTerminalPlaceholder(title = "No pane available", foreground = foreground, background = background)
            } else {
                MuxyTerminalView(
                    client = container.muxyClient,
                    paneID = paneID,
                    modifier = Modifier.fillMaxSize(),
                )
            }
        }
        TabKindDTO.VCS -> NonTerminalPlaceholder(
            title = "Source Control — open on desktop",
            foreground = foreground,
            background = background,
        )
        TabKindDTO.EDITOR -> NonTerminalPlaceholder(
            title = tab.title.ifBlank { "Editor — open on desktop" },
            foreground = foreground,
            background = background,
        )
        TabKindDTO.DIFF_VIEWER -> NonTerminalPlaceholder(
            title = tab.title.ifBlank { "Diff — open on desktop" },
            foreground = foreground,
            background = background,
        )
    }
}

@Composable
private fun NonTerminalPlaceholder(
    title: String,
    foreground: androidx.compose.ui.graphics.Color,
    background: androidx.compose.ui.graphics.Color,
) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(background),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = title,
            color = foreground.copy(alpha = 0.7f),
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(24.dp),
        )
    }
}

@Composable
private fun EmptyTabsState(
    foreground: androidx.compose.ui.graphics.Color,
    onCreate: () -> Unit,
) {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
            modifier = Modifier.padding(horizontal = 24.dp),
        ) {
            Icon(
                imageVector = Icons.Outlined.Terminal,
                contentDescription = null,
                tint = foreground.copy(alpha = 0.4f),
                modifier = Modifier.size(48.dp),
            )
            Spacer(Modifier.height(12.dp))
            Text(
                text = "No tabs",
                color = foreground,
                style = MaterialTheme.typography.titleMedium,
                textAlign = TextAlign.Center,
            )
            Spacer(Modifier.height(4.dp))
            Text(
                text = "Create a new terminal to get started.",
                color = foreground.copy(alpha = 0.7f),
                textAlign = TextAlign.Center,
            )
            Spacer(Modifier.height(16.dp))
            androidx.compose.material3.OutlinedButton(onClick = onCreate) {
                Text("New Terminal")
            }
        }
    }
}

@Composable
private fun WorkspaceLoading(modifier: Modifier = Modifier) {
    Box(modifier = modifier, contentAlignment = Alignment.Center) {
        CircularProgressIndicator()
    }
}
