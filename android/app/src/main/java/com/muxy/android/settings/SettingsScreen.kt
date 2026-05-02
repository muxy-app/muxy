package com.muxy.android.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Remove
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.muxy.android.LocalAppContainer
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val container = LocalAppContainer.current
    val viewModel: SettingsViewModel =
        viewModel(
            factory =
                SettingsViewModel.factory(
                    terminalPreferences = container.terminalPreferences,
                    credentialsStore = container.deviceCredentialsStore,
                    savedDevicesStore = container.savedDevicesStore,
                    lastSessionStore = container.lastSessionStore,
                ),
        )

    val fontSize by viewModel.fontSize.collectAsStateWithLifecycle()
    val useNerdFont by viewModel.useNerdFont.collectAsStateWithLifecycle()
    val appVersionName = remember { container.appVersionName() }
    val appVersionCode = remember { container.appVersionCode() }
    val coroutineScope = rememberCoroutineScope()
    var showForgetDialog by remember { mutableStateOf(false) }

    Scaffold(
        modifier = modifier.fillMaxSize(),
        topBar = {
            TopAppBar(
                title = { Text("Settings") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Outlined.ArrowBack, contentDescription = "Back")
                    }
                },
                colors =
                    TopAppBarDefaults.topAppBarColors(
                        containerColor = MaterialTheme.colorScheme.background,
                    ),
            )
        },
        containerColor = MaterialTheme.colorScheme.background,
    ) { padding ->
        Column(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 16.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            TerminalSection(
                fontSize = fontSize,
                useNerdFont = useNerdFont,
                onFontSizeChange = { viewModel.setFontSize(it) },
                onUseNerdFontChange = { viewModel.setUseNerdFont(it) },
            )
            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
            DevicesSection()
            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
            AboutSection(versionName = appVersionName, versionCode = appVersionCode)
            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
            DangerSection(onForgetTap = { showForgetDialog = true })
        }
    }

    if (showForgetDialog) {
        ForgetDeviceDialog(
            onConfirm = {
                showForgetDialog = false
                coroutineScope.launch { viewModel.forgetDevice() }
            },
            onDismiss = { showForgetDialog = false },
        )
    }
}

@Composable
private fun TerminalSection(
    fontSize: Int,
    useNerdFont: Boolean,
    onFontSizeChange: (Int) -> Unit,
    onUseNerdFontChange: (Boolean) -> Unit,
) {
    SectionTitle(text = "Terminal")
    Spacer(Modifier.height(8.dp))
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Row(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .semantics { contentDescription = "Use Nerd Font" },
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text("Use Nerd Font", style = MaterialTheme.typography.bodyLarge)
                Text(
                    text = "Enables glyph icons in terminals that use a Nerd Font.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Switch(checked = useNerdFont, onCheckedChange = onUseNerdFontChange)
        }
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text("Font size", style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
            FontSizeStepper(value = fontSize, onChange = onFontSizeChange)
        }
        Text(
            text = "The quick brown fox",
            style = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun FontSizeStepper(
    value: Int,
    onChange: (Int) -> Unit,
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        FilledIconButton(
            onClick = { onChange((value - 1).coerceAtLeast(TerminalPreferences.MIN_FONT_SIZE)) },
            enabled = value > TerminalPreferences.MIN_FONT_SIZE,
            colors =
                IconButtonDefaults.filledIconButtonColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant,
                    contentColor = MaterialTheme.colorScheme.onSurfaceVariant,
                ),
            modifier =
                Modifier
                    .size(32.dp)
                    .semantics { contentDescription = "Decrease font size" },
        ) {
            Icon(Icons.Outlined.Remove, contentDescription = null, modifier = Modifier.size(18.dp))
        }
        Text(
            text = value.toString(),
            style = MaterialTheme.typography.bodyMedium,
            modifier =
                Modifier
                    .padding(horizontal = 4.dp)
                    .semantics { contentDescription = "Font size $value" },
        )
        FilledIconButton(
            onClick = { onChange((value + 1).coerceAtMost(TerminalPreferences.MAX_FONT_SIZE)) },
            enabled = value < TerminalPreferences.MAX_FONT_SIZE,
            colors =
                IconButtonDefaults.filledIconButtonColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant,
                    contentColor = MaterialTheme.colorScheme.onSurfaceVariant,
                ),
            modifier =
                Modifier
                    .size(32.dp)
                    .semantics { contentDescription = "Increase font size" },
        ) {
            Icon(Icons.Outlined.Add, contentDescription = null, modifier = Modifier.size(18.dp))
        }
    }
}

@Composable
private fun DevicesSection() {
    SectionTitle(text = "Devices")
    Spacer(Modifier.height(8.dp))
    Text(
        text = "Saved devices live on the Connect screen. Tap a device to reconnect or swipe to remove it.",
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun AboutSection(
    versionName: String,
    versionCode: Long,
) {
    SectionTitle(text = "About")
    Spacer(Modifier.height(8.dp))
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        InfoRow(label = "Version", value = versionName)
        InfoRow(label = "Build", value = versionCode.toString())
        InfoRow(label = "Source code", value = "github.com/muxy-app/muxy")
        InfoRow(label = "Terminal core", value = "Termux terminal-emulator + terminal-view")
    }
}

@Composable
private fun InfoRow(
    label: String,
    value: String,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.weight(1f),
        )
        Text(text = value, style = MaterialTheme.typography.bodyMedium)
    }
}

@Composable
private fun DangerSection(onForgetTap: () -> Unit) {
    SectionTitle(text = "Pairing")
    Spacer(Modifier.height(8.dp))
    OutlinedButton(
        onClick = onForgetTap,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Icon(Icons.Outlined.Delete, contentDescription = null, modifier = Modifier.size(18.dp))
        Spacer(Modifier.size(8.dp))
        Text("Forget this device", color = MaterialTheme.colorScheme.error)
    }
    Spacer(Modifier.height(4.dp))
    Text(
        text = "Forgetting clears the pairing token and saved devices on this phone. The Mac keeps its approved-device record until you remove it from Mac settings.",
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun SectionTitle(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.titleMedium,
        color = MaterialTheme.colorScheme.onSurface,
    )
}

@Composable
private fun ForgetDeviceDialog(
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Forget this device?") },
        text = {
            Text(
                text = "This deletes your pairing token and removes every saved Mac from this phone. You will need to pair again the next time you connect.",
            )
        },
        confirmButton = {
            TextButton(onClick = onConfirm) {
                Text("Forget", color = MaterialTheme.colorScheme.error)
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}
