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
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.CheckCircle
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Folder
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.muxy.android.LocalAppContainer
import com.muxy.android.ui.theme.muxyColors
import com.muxy.net.addWorktree
import com.muxy.net.listBranches
import com.muxy.net.removeWorktree
import com.muxy.protocol.dto.WorktreeDTO
import kotlinx.coroutines.launch
import java.util.UUID

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WorktreesSheet(
    projectID: UUID,
    onDismiss: () -> Unit,
    onChange: () -> Unit,
) {
    val container = LocalAppContainer.current
    val client = container.muxyClient
    val theme by client.deviceTheme.collectAsStateWithLifecycle()
    val workspace by client.workspace.collectAsStateWithLifecycle()
    val projectWorktrees by client.projectWorktrees.collectAsStateWithLifecycle()
    val colors = muxyColors(theme)
    val scope = rememberCoroutineScope()
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    var errorMessage by remember { mutableStateOf<String?>(null) }
    var busyID by remember { mutableStateOf<UUID?>(null) }
    var showAdd by remember { mutableStateOf(false) }

    val worktrees = projectWorktrees[projectID] ?: emptyList()
    val activeID = workspace?.worktreeID

    LaunchedEffect(projectID) { client.refreshWorktrees(projectID) }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = colors.background,
    ) {
        Column(modifier = Modifier.fillMaxWidth()) {
            Row(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 8.dp, vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TextButton(onClick = onDismiss) { Text("Close", color = colors.foreground) }
                Spacer(Modifier.weight(1f))
                Text(
                    text = "Worktrees",
                    style = MaterialTheme.typography.titleMedium,
                    color = colors.foreground,
                )
                Spacer(Modifier.weight(1f))
                IconButton(onClick = { showAdd = true }) {
                    Icon(Icons.Outlined.Add, contentDescription = "Add worktree", tint = colors.foreground)
                }
            }
            HorizontalDivider(color = colors.outline)
            LazyColumn(
                modifier =
                    Modifier
                        .fillMaxSize()
                        .padding(horizontal = 16.dp, vertical = 12.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                items(items = worktrees, key = { it.id }) { worktree ->
                    WorktreeRow(
                        worktree = worktree,
                        isActive = worktree.id == activeID,
                        busy = busyID == worktree.id,
                        foreground = colors.foreground,
                        muted = colors.mutedForeground,
                        cardBackground = colors.cardBackground,
                        onTap = {
                            if (worktree.id == activeID) return@WorktreeRow
                            busyID = worktree.id
                            scope.launch {
                                try {
                                    val ok = client.selectWorktree(projectID, worktree.id)
                                    if (ok) {
                                        onChange()
                                        onDismiss()
                                    } else {
                                        errorMessage = "Could not switch worktree"
                                    }
                                } catch (t: Throwable) {
                                    errorMessage = errorMessageOf(t)
                                } finally {
                                    busyID = null
                                }
                            }
                        },
                        onRemove = {
                            busyID = worktree.id
                            scope.launch {
                                try {
                                    client.removeWorktree(projectID, worktree.id)
                                } catch (t: Throwable) {
                                    errorMessage = errorMessageOf(t)
                                } finally {
                                    busyID = null
                                }
                            }
                        },
                    )
                }
                if (errorMessage != null) {
                    item {
                        Text(
                            errorMessage!!,
                            color = Color(0xFFE53935),
                            style = MaterialTheme.typography.bodySmall,
                            modifier = Modifier.padding(top = 8.dp),
                        )
                    }
                }
                item { Spacer(Modifier.height(8.dp)) }
            }
        }
    }

    if (showAdd) {
        AddWorktreeSheet(
            projectID = projectID,
            onDismiss = { showAdd = false },
            onAdded = {
                scope.launch { client.refreshWorktrees(projectID) }
                onChange()
            },
        )
    }
}

@Composable
private fun WorktreeRow(
    worktree: WorktreeDTO,
    isActive: Boolean,
    busy: Boolean,
    foreground: Color,
    muted: Color,
    cardBackground: Color,
    onTap: () -> Unit,
    onRemove: () -> Unit,
) {
    var menuOpen by remember { mutableStateOf(false) }
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .background(cardBackground, shape = RoundedCornerShape(8.dp))
                .clickable(enabled = !isActive, onClick = onTap)
                .padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val icon =
            when {
                isActive -> Icons.Outlined.CheckCircle
                worktree.isPrimary -> Icons.Outlined.Home
                else -> Icons.Outlined.Folder
            }
        val tint = if (isActive) Color(0xFF2E7D32) else muted
        Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(18.dp))
        Spacer(Modifier.width(10.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(worktree.name, color = foreground)
            val branch = worktree.branch
            if (branch != null) {
                Text(branch, color = muted, style = MaterialTheme.typography.bodySmall)
            }
        }
        if (busy) {
            CircularProgressIndicator(
                modifier = Modifier.size(16.dp),
                strokeWidth = 2.dp,
                color = foreground,
            )
        } else if (worktree.canBeRemoved && !isActive) {
            Box {
                IconButton(onClick = { menuOpen = true }) {
                    Icon(Icons.Outlined.Delete, contentDescription = "Remove", tint = foreground)
                }
                DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                    DropdownMenuItem(
                        text = { Text("Remove") },
                        onClick = {
                            menuOpen = false
                            onRemove()
                        },
                    )
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AddWorktreeSheet(
    projectID: UUID,
    onDismiss: () -> Unit,
    onAdded: () -> Unit,
) {
    val container = LocalAppContainer.current
    val client = container.muxyClient
    val theme by client.deviceTheme.collectAsStateWithLifecycle()
    val colors = muxyColors(theme)
    val scope = rememberCoroutineScope()
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    var name by remember { mutableStateOf("") }
    var branchName by remember { mutableStateOf("") }
    var useExistingBranch by remember { mutableStateOf(false) }
    var existingBranches by remember { mutableStateOf<List<String>>(emptyList()) }
    var selectedExisting by remember { mutableStateOf("") }
    var inProgress by remember { mutableStateOf(false) }
    var errorMessage by remember { mutableStateOf<String?>(null) }
    var dropdownOpen by remember { mutableStateOf(false) }

    LaunchedEffect(projectID) {
        try {
            val branches = client.listBranches(projectID)
            existingBranches = branches.locals
            if (selectedExisting.isEmpty()) selectedExisting = branches.locals.firstOrNull() ?: ""
        } catch (t: Throwable) {
            errorMessage = errorMessageOf(t)
        }
    }

    val canSubmit =
        name.isNotBlank() &&
            if (useExistingBranch) selectedExisting.isNotEmpty() else branchName.isNotBlank()

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = colors.background,
    ) {
        Column(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                TextButton(onClick = onDismiss) { Text("Cancel", color = colors.foreground) }
                Spacer(Modifier.weight(1f))
                Text(
                    text = "Add Worktree",
                    style = MaterialTheme.typography.titleMedium,
                    color = colors.foreground,
                )
                Spacer(Modifier.weight(1f))
                if (inProgress) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(20.dp),
                        strokeWidth = 2.dp,
                        color = colors.foreground,
                    )
                } else {
                    TextButton(
                        enabled = canSubmit,
                        onClick = {
                            inProgress = true
                            val branch = if (useExistingBranch) selectedExisting else branchName.trim()
                            scope.launch {
                                try {
                                    client.addWorktree(
                                        projectID = projectID,
                                        name = name.trim(),
                                        branch = branch,
                                        createBranch = !useExistingBranch,
                                    )
                                    onAdded()
                                    onDismiss()
                                } catch (t: Throwable) {
                                    errorMessage = errorMessageOf(t)
                                } finally {
                                    inProgress = false
                                }
                            }
                        },
                    ) { Text("Add", color = colors.foreground) }
                }
            }

            Text("Worktree name", color = colors.mutedForeground, style = MaterialTheme.typography.labelMedium)
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                placeholder = { Text("Name", color = colors.faintForeground) },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
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

            Text("Branch", color = colors.mutedForeground, style = MaterialTheme.typography.labelMedium)
            SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
                SegmentedButton(
                    selected = !useExistingBranch,
                    onClick = { useExistingBranch = false },
                    shape = SegmentedButtonDefaults.itemShape(0, 2),
                ) { Text("New Branch") }
                SegmentedButton(
                    selected = useExistingBranch,
                    onClick = { useExistingBranch = true },
                    shape = SegmentedButtonDefaults.itemShape(1, 2),
                ) { Text("Existing") }
            }

            if (useExistingBranch) {
                Box {
                    OutlinedTextField(
                        value = selectedExisting,
                        onValueChange = { },
                        readOnly = true,
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .clickable { dropdownOpen = true },
                        placeholder = { Text("Select branch", color = colors.faintForeground) },
                        colors =
                            TextFieldDefaults.colors(
                                focusedTextColor = colors.foreground,
                                unfocusedTextColor = colors.foreground,
                                focusedContainerColor = Color.Transparent,
                                unfocusedContainerColor = Color.Transparent,
                                focusedIndicatorColor = colors.foreground,
                                unfocusedIndicatorColor = colors.outline,
                            ),
                    )
                    DropdownMenu(
                        expanded = dropdownOpen,
                        onDismissRequest = { dropdownOpen = false },
                    ) {
                        existingBranches.forEach { b ->
                            DropdownMenuItem(
                                text = { Text(b) },
                                onClick = {
                                    selectedExisting = b
                                    dropdownOpen = false
                                },
                            )
                        }
                    }
                }
            } else {
                OutlinedTextField(
                    value = branchName,
                    onValueChange = { branchName = it },
                    placeholder = { Text("new-branch-name", color = colors.faintForeground) },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
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
            }

            if (errorMessage != null) {
                Text(
                    errorMessage!!,
                    color = Color(0xFFE53935),
                    style = MaterialTheme.typography.bodySmall,
                )
            }
        }
    }
}
