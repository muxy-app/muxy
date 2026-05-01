package com.muxy.android.vcs

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.muxy.android.LocalAppContainer
import com.muxy.android.ui.theme.muxyColors
import com.muxy.net.createPullRequest
import kotlinx.coroutines.launch
import java.util.UUID

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CreatePRSheet(
    projectID: UUID,
    defaultBase: String?,
    currentBranch: String,
    onDismiss: () -> Unit,
    onCreated: () -> Unit,
) {
    val container = LocalAppContainer.current
    val client = container.muxyClient
    val theme by client.deviceTheme.collectAsStateWithLifecycle()
    val colors = muxyColors(theme)
    val scope = rememberCoroutineScope()
    val uriHandler = LocalUriHandler.current
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    var title by remember { mutableStateOf("") }
    var body by remember { mutableStateOf("") }
    var baseBranch by remember { mutableStateOf(defaultBase ?: "") }
    var draft by remember { mutableStateOf(false) }
    var inProgress by remember { mutableStateOf(false) }
    var errorMessage by remember { mutableStateOf<String?>(null) }

    val canSubmit = title.isNotBlank()

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = colors.background,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                TextButton(onClick = onDismiss) { Text("Cancel", color = colors.foreground) }
                Spacer(Modifier.weight(1f))
                Text(
                    text = "New Pull Request",
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
                            val baseTrim = baseBranch.trim().ifEmpty { null }
                            scope.launch {
                                try {
                                    val result = client.createPullRequest(
                                        projectID = projectID,
                                        title = title.trim(),
                                        body = body,
                                        baseBranch = baseTrim,
                                        draft = draft,
                                    )
                                    onCreated()
                                    onDismiss()
                                    runCatching { uriHandler.openUri(result.url) }
                                } catch (t: Throwable) {
                                    errorMessage = errorMessageOf(t)
                                } finally {
                                    inProgress = false
                                }
                            }
                        },
                    ) { Text("Create", color = colors.foreground) }
                }
            }
            HorizontalDivider(color = colors.outline)

            Column {
                Text("From", color = colors.mutedForeground, style = MaterialTheme.typography.labelMedium)
                Text(currentBranch, color = colors.foreground)
            }
            OutlinedTextField(
                value = baseBranch,
                onValueChange = { baseBranch = it },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                placeholder = { Text("Base (e.g. main)", color = colors.faintForeground) },
                colors = textFieldColors(colors.foreground, colors.outline),
            )

            OutlinedTextField(
                value = title,
                onValueChange = { title = it },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                placeholder = { Text("Title", color = colors.faintForeground) },
                colors = textFieldColors(colors.foreground, colors.outline),
            )

            OutlinedTextField(
                value = body,
                onValueChange = { body = it },
                modifier = Modifier.fillMaxWidth(),
                placeholder = { Text("Body", color = colors.faintForeground) },
                minLines = 4,
                maxLines = 10,
                colors = textFieldColors(colors.foreground, colors.outline),
            )

            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Draft", color = colors.foreground, modifier = Modifier.weight(1f))
                Switch(checked = draft, onCheckedChange = { draft = it })
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

@Composable
private fun textFieldColors(foreground: Color, outline: Color) =
    TextFieldDefaults.colors(
        focusedTextColor = foreground,
        unfocusedTextColor = foreground,
        focusedContainerColor = Color.Transparent,
        unfocusedContainerColor = Color.Transparent,
        cursorColor = foreground,
        focusedIndicatorColor = foreground,
        unfocusedIndicatorColor = outline,
    )
