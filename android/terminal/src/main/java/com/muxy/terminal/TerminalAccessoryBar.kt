package com.muxy.terminal

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.ContentPaste
import androidx.compose.material.icons.outlined.Keyboard
import androidx.compose.material.icons.outlined.KeyboardHide
import androidx.compose.material3.Icon
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.math.hypot

private const val ARROW_UP = "\u001B[A"
private const val ARROW_DOWN = "\u001B[B"
private const val ARROW_LEFT = "\u001B[D"
private const val ARROW_RIGHT = "\u001B[C"
private const val ESC_PAYLOAD = "\u001B"
private const val TAB_PAYLOAD = "\t"

@Composable
fun TerminalAccessoryBar(
    actions: AccessoryActions,
    armedModifier: ArmedModifier?,
    activeModifier: ArmedModifier,
    onToggleArm: () -> Unit,
    onSelectModifier: (ArmedModifier) -> Unit,
    foreground: Color,
    background: Color,
    canCopySelection: Boolean,
    keyboardVisible: Boolean,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier.fillMaxWidth().background(background.copy(alpha = 0.94f))) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(horizontal = 10.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(
                modifier =
                    Modifier
                        .weight(1f)
                        .horizontalScroll(rememberScrollState()),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                AccessoryKey("esc", foreground) { actions.sendText(ESC_PAYLOAD) }
                ModifierKey(
                    active = activeModifier,
                    armed = armedModifier != null,
                    foreground = foreground,
                    background = background,
                    onTap = onToggleArm,
                    onSelect = onSelectModifier,
                )
                AccessoryKey("tab", foreground) { actions.sendText(TAB_PAYLOAD) }
                AccessoryIcon(Icons.Outlined.ContentPaste, "Paste", enabled = true, foreground = foreground) {
                    actions.pasteFromClipboard()
                }
                AccessoryIcon(Icons.Outlined.ContentCopy, "Copy", enabled = canCopySelection, foreground = foreground) {
                    actions.copySelectionToClipboard()
                }
                AccessoryKey("~", foreground) { actions.sendText("~") }
                AccessoryKey("|", foreground) { actions.sendText("|") }
                AccessoryKey("/", foreground) { actions.sendText("/") }
                AccessoryKey("-", foreground) { actions.sendText("-") }
            }
            KeyboardToggle(visible = keyboardVisible, foreground = foreground, onClick = actions::toggleKeyboard)
            DPad(foreground = foreground) { payload -> actions.sendText(payload) }
        }
    }
}

@Composable
private fun AccessoryKey(
    label: String,
    foreground: Color,
    onClick: () -> Unit,
) {
    Surface(
        color = Color.Transparent,
        contentColor = foreground,
        shape = RoundedCornerShape(8.dp),
        modifier =
            Modifier
                .height(36.dp)
                .semantics {
                    contentDescription = "Send $label"
                    role = Role.Button
                    this.onClick(label = "Send $label") {
                        onClick()
                        true
                    }
                }
                .pointerInput(label) {
                    detectTapGestures(onTap = { onClick() })
                }
                .padding(horizontal = 10.dp),
    ) {
        Box(contentAlignment = Alignment.Center, modifier = Modifier.fillMaxWidth().height(36.dp)) {
            Text(label, fontSize = 14.sp, color = foreground)
        }
    }
}

@Composable
private fun AccessoryIcon(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    description: String,
    enabled: Boolean,
    foreground: Color,
    onClick: () -> Unit,
) {
    val tint = if (enabled) foreground else foreground.copy(alpha = 0.4f)
    Box(
        contentAlignment = Alignment.Center,
        modifier =
            Modifier
                .size(36.dp)
                .semantics {
                    contentDescription = description
                    role = Role.Button
                    if (!enabled) disabled()
                    if (enabled) {
                        this.onClick(label = description) {
                            onClick()
                            true
                        }
                    }
                }
                .pointerInput(description, enabled) {
                    if (enabled) detectTapGestures(onTap = { onClick() })
                },
    ) {
        Icon(icon, contentDescription = null, tint = tint)
    }
}

@Composable
private fun KeyboardToggle(
    visible: Boolean,
    foreground: Color,
    onClick: () -> Unit,
) {
    val description = if (visible) "Hide keyboard" else "Show keyboard"
    Box(
        contentAlignment = Alignment.Center,
        modifier =
            Modifier
                .size(40.dp)
                .semantics {
                    contentDescription = description
                    role = Role.Button
                    this.onClick(label = description) {
                        onClick()
                        true
                    }
                }
                .pointerInput(visible) {
                    detectTapGestures(onTap = { onClick() })
                },
    ) {
        val icon = if (visible) Icons.Outlined.KeyboardHide else Icons.Outlined.Keyboard
        Icon(icon, contentDescription = null, tint = foreground)
    }
}

@Composable
private fun ModifierKey(
    active: ArmedModifier,
    armed: Boolean,
    foreground: Color,
    background: Color,
    onTap: () -> Unit,
    onSelect: (ArmedModifier) -> Unit,
) {
    var pickerVisible by remember { mutableStateOf(false) }
    val armedLabel = if (armed) "armed" else "off"
    Box(contentAlignment = Alignment.Center) {
        Surface(
            color = if (armed) foreground else Color.Transparent,
            contentColor = if (armed) background else foreground,
            shape = RoundedCornerShape(18.dp),
            modifier =
                Modifier
                    .height(36.dp)
                    .semantics {
                        contentDescription = "${active.displayName} modifier $armedLabel. Long-press to choose modifier."
                        role = Role.Button
                        this.onClick(label = "Toggle ${active.displayName}") {
                            onTap()
                            true
                        }
                    }
                    .pointerInput(active, armed) {
                        detectTapGestures(
                            onTap = { onTap() },
                            onLongPress = { pickerVisible = true },
                        )
                    }
                    .padding(horizontal = 12.dp),
        ) {
            Box(contentAlignment = Alignment.Center, modifier = Modifier.height(36.dp)) {
                Text(active.displayName, fontSize = 14.sp)
            }
        }
        AnimatedVisibility(
            visible = pickerVisible,
            enter = fadeIn(),
            exit = fadeOut(),
        ) {
            ModifierPicker(
                active = active,
                onPick = { picked ->
                    onSelect(picked)
                    pickerVisible = false
                },
                onDismiss = { pickerVisible = false },
                foreground = foreground,
                background = background,
            )
        }
    }
}

@Composable
private fun ModifierPicker(
    active: ArmedModifier,
    onPick: (ArmedModifier) -> Unit,
    onDismiss: () -> Unit,
    foreground: Color,
    background: Color,
) {
    Surface(
        color = background.copy(alpha = 0.95f),
        contentColor = foreground,
        shape = RoundedCornerShape(14.dp),
        modifier = Modifier.padding(8.dp),
    ) {
        Column(modifier = Modifier.padding(8.dp)) {
            ArmedModifier.values().forEach { modifier ->
                val disabled = modifier == active
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier =
                        Modifier
                            .padding(horizontal = 12.dp, vertical = 8.dp)
                            .pointerInput(modifier, disabled) {
                                if (!disabled) detectTapGestures(onTap = { onPick(modifier) })
                            },
                ) {
                    Text(
                        modifier.glyph,
                        fontSize = 16.sp,
                        color = if (disabled) foreground.copy(alpha = 0.4f) else foreground,
                    )
                    Spacer(Modifier.width(10.dp))
                    Text(
                        modifier.displayName,
                        fontSize = 14.sp,
                        color = if (disabled) foreground.copy(alpha = 0.4f) else foreground,
                    )
                }
            }
        }
    }
    LaunchedEffect(Unit) {
        // Auto-dismiss safety: if user lifts away, ensure we close.
        delay(8000)
        onDismiss()
    }
}

@Composable
private fun DPad(
    foreground: Color,
    onDirection: (String) -> Unit,
) {
    val scope = rememberCoroutineScope()
    var thumbOffset by remember { mutableStateOf(Offset.Zero) }
    var activeDirection by remember { mutableStateOf<DPadDirection?>(null) }
    var repeatJob by remember { mutableStateOf<Job?>(null) }

    fun stopRepeating() {
        repeatJob?.cancel()
        repeatJob = null
        activeDirection = null
    }

    fun startRepeating(direction: DPadDirection) {
        repeatJob?.cancel()
        onDirection(direction.payload)
        repeatJob =
            scope.launch {
                delay(300)
                while (true) {
                    onDirection(direction.payload)
                    delay(60)
                }
            }
    }

    DisposableEffect(Unit) {
        onDispose { stopRepeating() }
    }

    Box(
        contentAlignment = Alignment.Center,
        modifier =
            Modifier
                .size(48.dp)
                .background(Color.Black.copy(alpha = 0.35f), CircleShape)
                .semantics {
                    contentDescription = "Arrow key D-pad. Drag in a direction to send arrow keys."
                }
                .pointerInput(Unit) {
                    detectDragGestures(
                        onDragStart = { thumbOffset = Offset.Zero },
                        onDragEnd = {
                            thumbOffset = Offset.Zero
                            stopRepeating()
                        },
                        onDragCancel = {
                            thumbOffset = Offset.Zero
                            stopRepeating()
                        },
                    ) { change, drag ->
                        val next = thumbOffset + drag
                        val mag = hypot(next.x.toDouble(), next.y.toDouble()).toFloat()
                        val deadZone = 5f
                        if (mag <= deadZone) {
                            if (activeDirection != null) stopRepeating()
                            thumbOffset = Offset.Zero
                            change.consume()
                            return@detectDragGestures
                        }
                        val direction =
                            if (abs(next.x) > abs(next.y)) {
                                if (next.x > 0) DPadDirection.RIGHT else DPadDirection.LEFT
                            } else {
                                if (next.y > 0) DPadDirection.DOWN else DPadDirection.UP
                            }
                        val maxReach = 12f
                        thumbOffset =
                            when (direction) {
                                DPadDirection.UP -> Offset(0f, -maxReach)
                                DPadDirection.DOWN -> Offset(0f, maxReach)
                                DPadDirection.LEFT -> Offset(-maxReach, 0f)
                                DPadDirection.RIGHT -> Offset(maxReach, 0f)
                            }
                        if (direction != activeDirection) {
                            activeDirection = direction
                            startRepeating(direction)
                        }
                        change.consume()
                    }
                },
    ) {
        Canvas(modifier = Modifier.size(16.dp)) {
            drawCircle(
                color = foreground.copy(alpha = 0.55f),
                center = Offset(size.width / 2 + thumbOffset.x, size.height / 2 + thumbOffset.y),
            )
        }
    }
}

private enum class DPadDirection(val payload: String) {
    UP(ARROW_UP),
    DOWN(ARROW_DOWN),
    LEFT(ARROW_LEFT),
    RIGHT(ARROW_RIGHT),
}
