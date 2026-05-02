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
import androidx.compose.material.icons.outlined.RadioButtonUnchecked
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
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
import com.muxy.net.createBranch
import com.muxy.net.listBranches
import com.muxy.net.switchBranch
import com.muxy.protocol.dto.VCSBranchesDTO
import kotlinx.coroutines.launch
import java.util.UUID

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BranchesSheet(
    projectID: UUID,
    onDismiss: () -> Unit,
    onChange: () -> Unit,
) {
    val container = LocalAppContainer.current
    val client = container.muxyClient
    val theme by client.deviceTheme.collectAsStateWithLifecycle()
    val colors = muxyColors(theme)
    val scope = rememberCoroutineScope()
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    var branches by remember { mutableStateOf<VCSBranchesDTO?>(null) }
    var isLoading by remember { mutableStateOf(false) }
    var errorMessage by remember { mutableStateOf<String?>(null) }
    var busyBranch by remember { mutableStateOf<String?>(null) }
    var showCreate by remember { mutableStateOf(false) }
    var newBranchName by remember { mutableStateOf("") }

    suspend fun load() {
        isLoading = true
        errorMessage = null
        try {
            branches = client.listBranches(projectID)
        } catch (t: Throwable) {
            errorMessage = errorMessageOf(t)
        } finally {
            isLoading = false
        }
    }

    LaunchedEffect(projectID) { load() }

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
                    text = "Branches",
                    style = MaterialTheme.typography.titleMedium,
                    color = colors.foreground,
                )
                Spacer(Modifier.weight(1f))
                IconButton(onClick = { showCreate = true }) {
                    Icon(Icons.Outlined.Add, contentDescription = "Create branch", tint = colors.foreground)
                }
            }
            HorizontalDivider(color = colors.outline)
            when {
                isLoading && branches == null ->
                    Box(
                        modifier =
                            Modifier
                                .fillMaxSize()
                                .padding(48.dp),
                        contentAlignment = Alignment.Center,
                    ) { CircularProgressIndicator(color = colors.foreground) }
                branches == null ->
                    Text(
                        text = errorMessage ?: "No branches",
                        color = colors.mutedForeground,
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .padding(24.dp),
                    )
                else ->
                    LazyColumn(
                        modifier =
                            Modifier
                                .fillMaxSize()
                                .padding(horizontal = 16.dp, vertical = 12.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        items(items = branches!!.locals, key = { it }) { branch ->
                            val isCurrent = branch == branches!!.current
                            Row(
                                modifier =
                                    Modifier
                                        .fillMaxWidth()
                                        .background(colors.cardBackground, shape = RoundedCornerShape(8.dp))
                                        .clickable(enabled = !isCurrent) {
                                            if (isCurrent) return@clickable
                                            busyBranch = branch
                                            scope.launch {
                                                try {
                                                    client.switchBranch(projectID, branch)
                                                    onChange()
                                                    onDismiss()
                                                } catch (t: Throwable) {
                                                    errorMessage = errorMessageOf(t)
                                                } finally {
                                                    busyBranch = null
                                                }
                                            }
                                        }
                                        .padding(horizontal = 12.dp, vertical = 10.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Icon(
                                    imageVector = if (isCurrent) Icons.Outlined.CheckCircle else Icons.Outlined.RadioButtonUnchecked,
                                    contentDescription = null,
                                    tint = if (isCurrent) Color(0xFF2E7D32) else colors.outline,
                                    modifier = Modifier.size(18.dp),
                                )
                                Spacer(Modifier.width(10.dp))
                                Text(branch, color = colors.foreground, modifier = Modifier.weight(1f))
                                if (busyBranch == branch) {
                                    CircularProgressIndicator(
                                        modifier = Modifier.size(16.dp),
                                        strokeWidth = 2.dp,
                                        color = colors.foreground,
                                    )
                                }
                            }
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
    }

    if (showCreate) {
        AlertDialog(
            onDismissRequest = {
                showCreate = false
                newBranchName = ""
            },
            title = { Text("New Branch") },
            text = {
                Column {
                    Text("Creates and switches to a new branch from HEAD.")
                    Spacer(Modifier.height(8.dp))
                    OutlinedTextField(
                        value = newBranchName,
                        onValueChange = { newBranchName = it },
                        placeholder = { Text("branch-name") },
                        singleLine = true,
                        colors = TextFieldDefaults.colors(),
                    )
                }
            },
            confirmButton = {
                TextButton(onClick = {
                    val name = newBranchName.trim()
                    showCreate = false
                    newBranchName = ""
                    if (name.isEmpty()) return@TextButton
                    busyBranch = name
                    scope.launch {
                        try {
                            client.createBranch(projectID, name)
                            onChange()
                            onDismiss()
                        } catch (t: Throwable) {
                            errorMessage = errorMessageOf(t)
                        } finally {
                            busyBranch = null
                        }
                    }
                }) { Text("Create") }
            },
            dismissButton = {
                TextButton(onClick = {
                    showCreate = false
                    newBranchName = ""
                }) { Text("Cancel") }
            },
        )
    }
}
