package com.muxy.android.vcs

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.OpenInNew
import androidx.compose.material.icons.outlined.AccountTree
import androidx.compose.material.icons.outlined.ArrowDownward
import androidx.compose.material.icons.outlined.ArrowUpward
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material.icons.outlined.CheckCircle
import androidx.compose.material.icons.outlined.MoreVert
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.muxy.android.LocalAppContainer
import com.muxy.android.ui.theme.MuxyColors
import com.muxy.android.ui.theme.muxyColors
import com.muxy.net.VCSClientError
import com.muxy.net.discardFiles
import com.muxy.net.fetchVCSStatus
import com.muxy.net.stageFiles
import com.muxy.net.unstageFiles
import com.muxy.net.vcsCommit
import com.muxy.net.vcsPull
import com.muxy.net.vcsPush
import com.muxy.protocol.dto.GitFileDTO
import com.muxy.protocol.dto.GitFileStatusDTO
import com.muxy.protocol.dto.VCSStatusDTO
import kotlinx.coroutines.launch
import java.util.UUID

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun VCSSheet(
    projectID: UUID,
    onDismiss: () -> Unit,
) {
    val container = LocalAppContainer.current
    val client = container.muxyClient
    val theme by client.deviceTheme.collectAsStateWithLifecycle()
    val colors = muxyColors(theme)
    val scope = rememberCoroutineScope()
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    var status by remember { mutableStateOf<VCSStatusDTO?>(null) }
    var isLoading by remember { mutableStateOf(false) }
    var errorMessage by remember { mutableStateOf<String?>(null) }
    var commitMessage by remember { mutableStateOf("") }
    val inFlight = remember { mutableStateMapOf<String, Boolean>() }
    var showBranches by remember { mutableStateOf(false) }
    var showWorktrees by remember { mutableStateOf(false) }
    var showCreatePR by remember { mutableStateOf(false) }
    var menuExpanded by remember { mutableStateOf(false) }

    suspend fun refresh() {
        isLoading = true
        errorMessage = null
        val fresh = client.fetchVCSStatus(projectID)
        status = fresh
        isLoading = false
        if (fresh == null) {
            errorMessage =
                "Could not read repository status. This project may not be a Git repository, or the Mac is unreachable."
        }
    }

    LaunchedEffect(projectID) { refresh() }

    suspend fun run(
        key: String,
        op: suspend () -> Unit,
    ) {
        inFlight[key] = true
        try {
            op()
            errorMessage = null
            refresh()
        } catch (t: Throwable) {
            errorMessage = errorMessageOf(t)
        } finally {
            inFlight.remove(key)
        }
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = colors.background,
    ) {
        Column(modifier = Modifier.fillMaxWidth()) {
            VCSHeader(
                colors = colors,
                onDismiss = onDismiss,
                hasPullRequest = status?.pullRequest != null,
                onShowMenu = { menuExpanded = true },
                menuExpanded = menuExpanded,
                onMenuDismiss = { menuExpanded = false },
                onBranches = {
                    menuExpanded = false
                    showBranches = true
                },
                onWorktrees = {
                    menuExpanded = false
                    showWorktrees = true
                },
                onCreatePR = {
                    menuExpanded = false
                    showCreatePR = true
                },
            )
            HorizontalDivider(color = colors.outline)
            when {
                isLoading && status == null ->
                    Box(
                        modifier =
                            Modifier
                                .fillMaxSize()
                                .padding(48.dp),
                        contentAlignment = Alignment.Center,
                    ) { CircularProgressIndicator(color = colors.foreground) }
                status == null ->
                    StatusUnavailable(
                        colors = colors,
                        errorMessage = errorMessage,
                        onRetry = { scope.launch { refresh() } },
                    )
                else ->
                    StatusContent(
                        status = status!!,
                        colors = colors,
                        commitMessage = commitMessage,
                        onCommitMessageChange = { commitMessage = it },
                        inFlight = inFlight,
                        errorMessage = errorMessage,
                        onPull = { scope.launch { run("pull") { client.vcsPull(projectID) } } },
                        onPush = { scope.launch { run("push") { client.vcsPush(projectID) } } },
                        onCommit = {
                            scope.launch {
                                run("commit") {
                                    client.vcsCommit(projectID, commitMessage, stageAll = false)
                                    commitMessage = ""
                                }
                            }
                        },
                        onStageAll = { paths ->
                            scope.launch { run("stageAll") { client.stageFiles(projectID, paths) } }
                        },
                        onUnstageAll = { paths ->
                            scope.launch { run("unstageAll") { client.unstageFiles(projectID, paths) } }
                        },
                        onStage = { file ->
                            scope.launch { run("stage:${file.path}") { client.stageFiles(projectID, listOf(file.path)) } }
                        },
                        onUnstage = { file ->
                            scope.launch {
                                run("unstage:${file.path}") { client.unstageFiles(projectID, listOf(file.path)) }
                            }
                        },
                        onDiscard = { file ->
                            scope.launch {
                                run("discard:${file.path}") {
                                    if (file.isUntracked) {
                                        client.discardFiles(
                                            projectID = projectID,
                                            paths = emptyList(),
                                            untrackedPaths = listOf(file.path),
                                        )
                                    } else {
                                        client.discardFiles(
                                            projectID = projectID,
                                            paths = listOf(file.path),
                                            untrackedPaths = emptyList(),
                                        )
                                    }
                                }
                            }
                        },
                        onPullRequestTap = status?.pullRequest?.url,
                    )
            }
        }
    }

    if (showBranches) {
        BranchesSheet(
            projectID = projectID,
            onDismiss = { showBranches = false },
            onChange = { scope.launch { refresh() } },
        )
    }
    if (showWorktrees) {
        WorktreesSheet(
            projectID = projectID,
            onDismiss = { showWorktrees = false },
            onChange = { scope.launch { refresh() } },
        )
    }
    if (showCreatePR) {
        CreatePRSheet(
            projectID = projectID,
            defaultBase = status?.defaultBranch,
            currentBranch = status?.branch ?: "",
            onDismiss = { showCreatePR = false },
            onCreated = { scope.launch { refresh() } },
        )
    }
}

@Composable
private fun VCSHeader(
    colors: MuxyColors,
    onDismiss: () -> Unit,
    hasPullRequest: Boolean,
    onShowMenu: () -> Unit,
    menuExpanded: Boolean,
    onMenuDismiss: () -> Unit,
    onBranches: () -> Unit,
    onWorktrees: () -> Unit,
    onCreatePR: () -> Unit,
) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(horizontal = 8.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        TextButton(onClick = onDismiss) { Text("Done", color = colors.foreground) }
        Spacer(Modifier.weight(1f))
        Text(
            text = "Source Control",
            style = MaterialTheme.typography.titleMedium,
            color = colors.foreground,
        )
        Spacer(Modifier.weight(1f))
        Box {
            IconButton(onClick = onShowMenu) {
                Icon(Icons.Outlined.MoreVert, contentDescription = "More", tint = colors.foreground)
            }
            DropdownMenu(expanded = menuExpanded, onDismissRequest = onMenuDismiss) {
                DropdownMenuItem(
                    text = { Text("Branches") },
                    leadingIcon = { Icon(Icons.Outlined.AccountTree, contentDescription = null) },
                    onClick = onBranches,
                )
                DropdownMenuItem(
                    text = { Text("Worktrees") },
                    leadingIcon = { Icon(Icons.Outlined.AccountTree, contentDescription = null) },
                    onClick = onWorktrees,
                )
                if (!hasPullRequest) {
                    DropdownMenuItem(
                        text = { Text("Create Pull Request") },
                        leadingIcon = { Icon(Icons.AutoMirrored.Outlined.OpenInNew, contentDescription = null) },
                        onClick = onCreatePR,
                    )
                }
            }
        }
    }
}

@Composable
private fun StatusContent(
    status: VCSStatusDTO,
    colors: MuxyColors,
    commitMessage: String,
    onCommitMessageChange: (String) -> Unit,
    inFlight: Map<String, Boolean>,
    errorMessage: String?,
    onPull: () -> Unit,
    onPush: () -> Unit,
    onCommit: () -> Unit,
    onStageAll: (List<String>) -> Unit,
    onUnstageAll: (List<String>) -> Unit,
    onStage: (GitFileDTO) -> Unit,
    onUnstage: (GitFileDTO) -> Unit,
    onDiscard: (GitFileDTO) -> Unit,
    onPullRequestTap: String?,
) {
    val uriHandler = LocalUriHandler.current
    LazyColumn(
        modifier =
            Modifier
                .fillMaxSize()
                .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item {
            SummaryCard(
                status = status,
                colors = colors,
                pullInFlight = inFlight.containsKey("pull"),
                pushInFlight = inFlight.containsKey("push"),
                onPull = onPull,
                onPush = onPush,
                onPullRequestTap = onPullRequestTap?.let { url -> { uriHandler.openUri(url) } },
            )
        }
        if (status.stagedFiles.isNotEmpty()) {
            item {
                SectionHeader(
                    text = "Staged (${status.stagedFiles.size})",
                    actionLabel = "Unstage All",
                    onAction = { onUnstageAll(status.stagedFiles.map(GitFileDTO::path)) },
                    colors = colors,
                )
            }
            items(items = status.stagedFiles, key = { "staged-${it.path}" }) { file ->
                FileRow(
                    file = file,
                    staged = true,
                    inFlight = inFlight,
                    colors = colors,
                    onStage = onStage,
                    onUnstage = onUnstage,
                    onDiscard = onDiscard,
                )
            }
        }
        if (status.changedFiles.isNotEmpty()) {
            item {
                SectionHeader(
                    text = "Changes (${status.changedFiles.size})",
                    actionLabel = "Stage All",
                    onAction = { onStageAll(status.changedFiles.map(GitFileDTO::path)) },
                    colors = colors,
                )
            }
            items(items = status.changedFiles, key = { "change-${it.path}" }) { file ->
                FileRow(
                    file = file,
                    staged = false,
                    inFlight = inFlight,
                    colors = colors,
                    onStage = onStage,
                    onUnstage = onUnstage,
                    onDiscard = onDiscard,
                )
            }
        }
        if (status.stagedFiles.isEmpty() && status.changedFiles.isEmpty()) {
            item { CleanCard(colors = colors) }
        }
        if (status.stagedFiles.isNotEmpty()) {
            item {
                CommitCard(
                    colors = colors,
                    message = commitMessage,
                    onMessageChange = onCommitMessageChange,
                    inFlight = inFlight.containsKey("commit"),
                    onCommit = onCommit,
                )
            }
        }
        if (errorMessage != null) {
            item {
                Text(
                    text = errorMessage,
                    color = Color(0xFFE53935),
                    style = MaterialTheme.typography.bodySmall,
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .background(colors.cardBackground, shape = RoundedCornerShape(8.dp))
                            .padding(12.dp),
                )
            }
        }
    }
}

@Composable
private fun SummaryCard(
    status: VCSStatusDTO,
    colors: MuxyColors,
    pullInFlight: Boolean,
    pushInFlight: Boolean,
    onPull: () -> Unit,
    onPush: () -> Unit,
    onPullRequestTap: (() -> Unit)?,
) {
    Column(
        modifier =
            Modifier
                .fillMaxWidth()
                .background(colors.cardBackground, shape = RoundedCornerShape(8.dp))
                .padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Outlined.AccountTree, contentDescription = null, tint = colors.mutedForeground)
            Spacer(Modifier.width(8.dp))
            Text(
                text = status.branch,
                color = colors.foreground,
                style = MaterialTheme.typography.bodyLarge,
            )
            Spacer(Modifier.weight(1f))
            if (status.aheadCount > 0) {
                Icon(
                    Icons.Outlined.ArrowUpward,
                    contentDescription = "Ahead",
                    tint = colors.mutedForeground,
                    modifier = Modifier.size(16.dp),
                )
                Text(
                    text = "${status.aheadCount}",
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.mutedForeground,
                )
                Spacer(Modifier.width(8.dp))
            }
            if (status.behindCount > 0) {
                Icon(
                    Icons.Outlined.ArrowDownward,
                    contentDescription = "Behind",
                    tint = colors.mutedForeground,
                    modifier = Modifier.size(16.dp),
                )
                Text(
                    text = "${status.behindCount}",
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.mutedForeground,
                )
            }
        }

        val pr = status.pullRequest
        if (pr != null && onPullRequestTap != null) {
            Row(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .clickable { onPullRequestTap() },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(Icons.AutoMirrored.Outlined.OpenInNew, contentDescription = null, tint = colors.mutedForeground)
                Spacer(Modifier.width(8.dp))
                Text(
                    text = "PR #${pr.number} (${pr.state.lowercase()})",
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.foreground,
                )
            }
        }

        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            OutlinedButton(
                onClick = onPull,
                modifier = Modifier.weight(1f),
                enabled = !pullInFlight,
            ) {
                Icon(Icons.Outlined.ArrowDownward, contentDescription = null)
                Spacer(Modifier.width(8.dp))
                Text("Pull")
            }
            OutlinedButton(
                onClick = onPush,
                modifier = Modifier.weight(1f),
                enabled = !pushInFlight && !(status.aheadCount == 0 && status.hasUpstream),
            ) {
                Icon(Icons.Outlined.ArrowUpward, contentDescription = null)
                Spacer(Modifier.width(8.dp))
                Text("Push")
            }
        }
    }
}

@Composable
private fun SectionHeader(
    text: String,
    actionLabel: String,
    onAction: () -> Unit,
    colors: MuxyColors,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = text,
            style = MaterialTheme.typography.labelMedium,
            color = colors.mutedForeground,
        )
        Spacer(Modifier.weight(1f))
        TextButton(onClick = onAction) {
            Text(actionLabel, color = colors.foreground)
        }
    }
}

@Composable
private fun FileRow(
    file: GitFileDTO,
    staged: Boolean,
    inFlight: Map<String, Boolean>,
    colors: MuxyColors,
    onStage: (GitFileDTO) -> Unit,
    onUnstage: (GitFileDTO) -> Unit,
    onDiscard: (GitFileDTO) -> Unit,
) {
    val key = if (staged) "unstage:${file.path}" else "stage:${file.path}"
    val rowInFlight = inFlight.containsKey(key) || inFlight.containsKey("discard:${file.path}")
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .background(colors.cardBackground, shape = RoundedCornerShape(8.dp))
                .padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        StatusBadge(file.status)
        Spacer(Modifier.width(10.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = fileName(file.path),
                style = MaterialTheme.typography.bodyMedium,
                color = colors.foreground,
                maxLines = 1,
            )
            Text(
                text = file.path,
                style = MaterialTheme.typography.bodySmall,
                color = colors.faintForeground,
                maxLines = 1,
            )
        }
        if (rowInFlight) {
            CircularProgressIndicator(
                modifier = Modifier.size(18.dp),
                strokeWidth = 2.dp,
                color = colors.foreground,
            )
        } else if (staged) {
            TextButton(onClick = { onUnstage(file) }) {
                Text("Unstage", color = colors.foreground)
            }
        } else {
            TextButton(onClick = { onStage(file) }) {
                Text("Stage", color = colors.foreground)
            }
            TextButton(onClick = { onDiscard(file) }) {
                Text("Discard", color = Color(0xFFE53935))
            }
        }
    }
}

@Composable
private fun StatusBadge(status: GitFileStatusDTO) {
    val label =
        when (status) {
            GitFileStatusDTO.ADDED -> "A"
            GitFileStatusDTO.MODIFIED -> "M"
            GitFileStatusDTO.DELETED -> "D"
            GitFileStatusDTO.RENAMED -> "R"
            GitFileStatusDTO.COPIED -> "C"
            GitFileStatusDTO.UNTRACKED -> "U"
            GitFileStatusDTO.UNMERGED -> "!"
        }
    val color =
        when (status) {
            GitFileStatusDTO.ADDED, GitFileStatusDTO.UNTRACKED -> Color(0xFF2E7D32)
            GitFileStatusDTO.MODIFIED, GitFileStatusDTO.RENAMED, GitFileStatusDTO.COPIED -> Color(0xFFEF6C00)
            GitFileStatusDTO.DELETED -> Color(0xFFC62828)
            GitFileStatusDTO.UNMERGED -> Color(0xFF6A1B9A)
        }
    Box(
        modifier =
            Modifier
                .size(20.dp)
                .background(color.copy(alpha = 0.2f), shape = RoundedCornerShape(4.dp)),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            color = color,
            style = MaterialTheme.typography.labelSmall,
        )
    }
}

@Composable
private fun CleanCard(colors: MuxyColors) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .background(colors.cardBackground, shape = RoundedCornerShape(8.dp))
                .padding(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.Outlined.CheckCircle, contentDescription = null, tint = Color(0xFF2E7D32))
        Spacer(Modifier.width(8.dp))
        Text(
            text = "Working tree clean",
            color = colors.mutedForeground,
        )
    }
}

@Composable
private fun CommitCard(
    colors: MuxyColors,
    message: String,
    onMessageChange: (String) -> Unit,
    inFlight: Boolean,
    onCommit: () -> Unit,
) {
    Column(
        modifier =
            Modifier
                .fillMaxWidth()
                .background(colors.cardBackground, shape = RoundedCornerShape(8.dp))
                .padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            text = "Commit",
            style = MaterialTheme.typography.labelMedium,
            color = colors.mutedForeground,
        )
        OutlinedTextField(
            value = message,
            onValueChange = onMessageChange,
            modifier = Modifier.fillMaxWidth(),
            placeholder = { Text("Commit message", color = colors.faintForeground) },
            minLines = 2,
            maxLines = 5,
            colors =
                TextFieldDefaults.colors(
                    focusedTextColor = colors.foreground,
                    unfocusedTextColor = colors.foreground,
                    focusedContainerColor = Color.Transparent,
                    unfocusedContainerColor = Color.Transparent,
                    cursorColor = colors.foreground,
                    focusedIndicatorColor = colors.foreground,
                    unfocusedIndicatorColor = colors.outline,
                ),
        )
        Button(
            onClick = onCommit,
            modifier = Modifier.fillMaxWidth(),
            enabled = message.isNotBlank() && !inFlight,
            colors =
                ButtonDefaults.buttonColors(
                    containerColor = colors.foreground,
                    contentColor = colors.background,
                ),
        ) {
            if (inFlight) {
                CircularProgressIndicator(
                    modifier = Modifier.size(18.dp),
                    strokeWidth = 2.dp,
                    color = colors.background,
                )
            } else {
                Icon(Icons.Outlined.Check, contentDescription = null)
                Spacer(Modifier.width(8.dp))
                Text("Commit")
            }
        }
    }
}

@Composable
private fun StatusUnavailable(
    colors: MuxyColors,
    errorMessage: String?,
    onRetry: () -> Unit,
) {
    Column(
        modifier =
            Modifier
                .fillMaxSize()
                .padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(
            Icons.Outlined.AccountTree,
            contentDescription = null,
            tint = colors.outline,
            modifier = Modifier.size(48.dp),
        )
        Spacer(Modifier.height(12.dp))
        Text(
            text = "Could not load repository status",
            color = colors.mutedForeground,
            textAlign = TextAlign.Center,
        )
        if (errorMessage != null) {
            Spacer(Modifier.height(8.dp))
            Text(
                text = errorMessage,
                color = Color(0xFFE53935),
                style = MaterialTheme.typography.bodySmall,
                textAlign = TextAlign.Center,
            )
        }
        Spacer(Modifier.height(16.dp))
        Button(
            onClick = onRetry,
            colors =
                ButtonDefaults.buttonColors(
                    containerColor = colors.foreground,
                    contentColor = colors.background,
                ),
        ) { Text("Retry") }
    }
}

internal fun fileName(path: String): String = path.substringAfterLast('/', missingDelimiterValue = path)

internal fun errorMessageOf(t: Throwable): String =
    when (t) {
        is VCSClientError -> t.message ?: "Unknown error"
        else -> t.message ?: "Unknown error"
    }
