package com.muxy.terminal

import android.content.ClipboardManager
import android.content.Context
import android.graphics.Typeface
import android.view.View
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import com.muxy.net.DeviceTheme
import com.muxy.net.MuxyClient
import com.muxy.protocol.dto.PaneOwnerDTO
import com.termux.terminal.TerminalEmulator
import com.termux.terminal.TextStyle
import com.termux.view.TerminalView
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import java.util.UUID

@Composable
fun MuxyTerminalView(
    client: MuxyClient,
    paneID: UUID,
    fontSizeSp: Int = 12,
    useNerdFont: Boolean = false,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val theme by client.deviceTheme.collectAsStateOrNull()
    val owners by client.paneOwners.collectAsStateOrNull()
    val myClientID by client.myClientID.collectAsStateOrNull()
    val sessionEpoch by client.sessionEpoch.collectAsStateOrNull()
    val typeface = remember(useNerdFont) { resolveTypeface(context, useNerdFont) }

    val foreground = theme?.let { rgbColor(it.fg) } ?: Color.White
    val background = theme?.let { rgbColor(it.bg) } ?: Color.Black

    var armed by remember { mutableStateOf<ArmedModifier?>(null) }
    var activeModifier by remember { mutableStateOf(ArmedModifier.CTRL) }
    var keyboardVisible by remember { mutableStateOf(false) }
    var reportedCols by remember { mutableStateOf<UInt?>(null) }
    var reportedRows by remember { mutableStateOf<UInt?>(null) }
    var autoTakenPaneID by remember { mutableStateOf<UUID?>(null) }

    val sessionClient = remember(context) { MuxyTerminalSessionClient(context) }
    val viewClient = remember { MuxyTerminalViewClient() }
    val session =
        remember(client, paneID) {
            MuxyTerminalSession(client = client, paneID = paneID, sessionClient = sessionClient)
        }
    val terminalViewRef = remember { mutableStateOf<TerminalView?>(null) }
    val sizeReporter =
        remember(client, paneID) {
            SizeReporter(client = client, paneID = paneID, scope = scope) { cols, rows ->
                reportedCols = cols
                reportedRows = rows
            }
        }

    viewClient.modifierProvider = { armed }
    sessionClient.onPasteRequested = {
        pasteClipboardThrough(context, session)
    }

    LaunchedEffect(client, paneID) {
        client.terminalBytes(paneID).collectLatest { bytes ->
            session.acceptRemoteOutput(bytes)
            terminalViewRef.value?.onScreenUpdated()
        }
    }

    LaunchedEffect(sessionEpoch) {
        autoTakenPaneID = null
    }

    LaunchedEffect(paneID, reportedCols, reportedRows, sessionEpoch) {
        val cols = reportedCols ?: return@LaunchedEffect
        val rows = reportedRows ?: return@LaunchedEffect
        if (autoTakenPaneID == paneID) return@LaunchedEffect
        autoTakenPaneID = paneID
        session.resetEmulatorScreen()
        client.takeOverPane(paneID = paneID, cols = cols, rows = rows)
    }

    DisposableEffect(client, paneID) {
        onDispose {
            scope.launch { client.releasePane(paneID) }
            session.finishIfRunning()
        }
    }

    val actions =
        remember(session, terminalViewRef, context) {
            object : AccessoryActions {
                override fun sendText(text: String) {
                    if (text.isEmpty()) return
                    val transformed = armed?.let { ModifierTransform.transform(text, it) } ?: text
                    if (armed != null) armed = null
                    if (transformed.isNotEmpty()) {
                        val bytes = transformed.toByteArray(Charsets.UTF_8)
                        client.sendTerminalInput(paneID = paneID, bytes = bytes)
                    }
                }

                override fun pasteFromClipboard() {
                    pasteClipboardThrough(context, session)
                }

                override fun toggleKeyboard() {
                    val view = terminalViewRef.value ?: return
                    val imm = context.getSystemService(Context.INPUT_METHOD_SERVICE) as? android.view.inputmethod.InputMethodManager ?: return
                    keyboardVisible = !keyboardVisible
                    if (keyboardVisible) {
                        view.isFocusable = true
                        view.isFocusableInTouchMode = true
                        view.requestFocus()
                        view.post {
                            imm.showSoftInput(view, android.view.inputmethod.InputMethodManager.SHOW_IMPLICIT)
                        }
                    } else {
                        imm.hideSoftInputFromWindow(view.windowToken, 0)
                    }
                }
            }
        }

    val ownerName =
        remember(owners, paneID) {
            when (val owner = owners?.get(paneID)) {
                is PaneOwnerDTO.Mac -> owner.deviceName
                is PaneOwnerDTO.Remote -> owner.deviceName
                null -> "Mac"
            }
        }
    val isOwnedBySelf =
        remember(owners, myClientID, paneID) {
            val mine = myClientID ?: return@remember false
            val owner = owners?.get(paneID) ?: return@remember false
            owner is PaneOwnerDTO.Remote && owner.deviceID == mine
        }

    Column(
        modifier =
            modifier
                .fillMaxSize()
                .background(background)
                .imePadding(),
    ) {
        Box(modifier = Modifier.weight(1f).fillMaxWidth()) {
            AndroidView(
                modifier = Modifier.fillMaxSize(),
                factory = { ctx ->
                    TerminalView(ctx, null).apply {
                        setTerminalViewClient(viewClient)
                        setTextSize(spToPx(ctx, fontSizeSp).toInt())
                        setTypeface(typeface)
                        attachSession(session)
                        applyTheme(theme, this)
                        sizeReporter.attach(this)
                        terminalViewRef.value = this
                    }
                },
                update = { view ->
                    view.attachSession(session)
                    applyTheme(theme, view)
                    sizeReporter.attach(view)
                    if (view.mEmulator != null) {
                        view.setTextSize(spToPx(context, fontSizeSp).toInt())
                        view.setTypeface(typeface)
                    }
                    view.alpha = if (isOwnedBySelf) 1f else 0f
                    view.isFocusable = isOwnedBySelf
                    view.isFocusableInTouchMode = isOwnedBySelf
                },
            )

            if (!isOwnedBySelf) {
                TakeOverOverlay(
                    ownerName = ownerName,
                    foreground = foreground,
                    background = background,
                    onTakeOver = {
                        val cols = reportedCols
                        val rows = reportedRows
                        if (cols != null && rows != null) {
                            scope.launch {
                                session.resetEmulatorScreen()
                                client.takeOverPane(paneID = paneID, cols = cols, rows = rows)
                            }
                        }
                    },
                )
            }
        }

        TerminalAccessoryBar(
            actions = actions,
            armedModifier = armed,
            activeModifier = activeModifier,
            onToggleArm = {
                armed = if (armed == null) activeModifier else null
            },
            onSelectModifier = { picked ->
                activeModifier = picked
                if (armed != null) armed = null
            },
            foreground = foreground,
            background = background,
            keyboardVisible = keyboardVisible,
        )
    }
}

private fun pasteClipboardThrough(
    context: Context,
    session: MuxyTerminalSession,
) {
    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager ?: return
    val text = clipboard.primaryClip?.getItemAt(0)?.coerceToText(context)?.toString() ?: return
    if (text.isEmpty()) return
    val bytes = text.toByteArray(Charsets.UTF_8)
    session.write(bytes, 0, bytes.size)
}

private fun applyTheme(
    theme: DeviceTheme?,
    view: TerminalView,
) {
    val emulator: TerminalEmulator = view.mEmulator ?: return
    val fg = (theme?.fg ?: 0xFFFFFFu).toInt() or 0xFF000000.toInt()
    val bg = (theme?.bg ?: 0x000000u).toInt() or 0xFF000000.toInt()
    emulator.mColors.mCurrentColors[TextStyle.COLOR_INDEX_FOREGROUND] = fg
    emulator.mColors.mCurrentColors[TextStyle.COLOR_INDEX_BACKGROUND] = bg
    emulator.mColors.mCurrentColors[TextStyle.COLOR_INDEX_CURSOR] = fg
    val palette = theme?.palette
    if (palette != null && palette.size == 16) {
        for (i in 0 until 16) {
            emulator.mColors.mCurrentColors[i] = palette[i].toInt() or 0xFF000000.toInt()
        }
    }
    view.setBackgroundColor(bg)
    view.invalidate()
}

private class SizeReporter(
    private val client: MuxyClient,
    private val paneID: UUID,
    private val scope: kotlinx.coroutines.CoroutineScope,
    private val onSize: (UInt, UInt) -> Unit,
) {
    private var lastCols: Int = 0
    private var lastRows: Int = 0
    private var attached: View? = null
    private val listener =
        View.OnLayoutChangeListener { v, _, _, _, _, _, _, _, _ ->
            report(v as TerminalView)
        }

    fun attach(view: TerminalView) {
        if (attached === view) {
            report(view)
            return
        }
        attached?.removeOnLayoutChangeListener(listener)
        view.removeOnLayoutChangeListener(listener)
        view.addOnLayoutChangeListener(listener)
        attached = view
        report(view)
    }

    private fun report(view: TerminalView) {
        val emulator = view.mEmulator ?: return
        val cols = emulator.mColumns
        val rows = emulator.mRows
        if (cols <= 0 || rows <= 0) return
        if (cols == lastCols && rows == lastRows) return
        lastCols = cols
        lastRows = rows
        onSize(cols.toUInt(), rows.toUInt())
        scope.launch { client.resizeTerminal(paneID = paneID, cols = cols.toUInt(), rows = rows.toUInt()) }
    }
}

private fun TerminalView.onScreenUpdated() {
    invalidate()
}

@Composable
private fun <T> StateFlow<T>.collectAsStateOrNull(): State<T> = collectAsState()

private fun rgbColor(rgb: UInt): Color {
    val r = ((rgb shr 16) and 0xFFu).toInt()
    val g = ((rgb shr 8) and 0xFFu).toInt()
    val b = (rgb and 0xFFu).toInt()
    return Color(red = r, green = g, blue = b)
}

private fun spToPx(
    context: Context,
    sp: Int,
): Float = sp * context.resources.displayMetrics.scaledDensity

private fun resolveTypeface(
    context: Context,
    useNerdFont: Boolean,
): Typeface {
    if (!useNerdFont) return Typeface.MONOSPACE
    return runCatching {
        Typeface.createFromAsset(context.assets, "fonts/JetBrainsMonoNerdFontMono-Regular.ttf")
    }.getOrDefault(Typeface.MONOSPACE)
}
