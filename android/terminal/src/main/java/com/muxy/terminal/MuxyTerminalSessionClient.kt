package com.muxy.terminal

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.util.Log
import com.termux.terminal.TerminalSession
import com.termux.terminal.TerminalSessionClient

internal class MuxyTerminalSessionClient(
    private val context: Context,
) : TerminalSessionClient {

    var onTextChanged: (() -> Unit)? = null
    var onTitleChanged: (() -> Unit)? = null
    var onColorsChanged: (() -> Unit)? = null
    var onPasteRequested: (() -> Unit)? = null

    override fun onTextChanged(changedSession: TerminalSession) {
        onTextChanged?.invoke()
    }

    override fun onTitleChanged(changedSession: TerminalSession) {
        onTitleChanged?.invoke()
    }

    override fun onSessionFinished(finishedSession: TerminalSession) = Unit

    override fun onCopyTextToClipboard(session: TerminalSession, text: String?) {
        if (text.isNullOrEmpty()) return
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
        clipboard?.setPrimaryClip(ClipData.newPlainText("muxy-terminal", text))
    }

    override fun onPasteTextFromClipboard(session: TerminalSession?) {
        onPasteRequested?.invoke()
    }

    override fun onBell(session: TerminalSession) = Unit

    override fun onColorsChanged(session: TerminalSession) {
        onColorsChanged?.invoke()
    }

    override fun onTerminalCursorStateChange(state: Boolean) = Unit

    override fun setTerminalShellPid(session: TerminalSession, pid: Int) = Unit

    override fun getTerminalCursorStyle(): Int? = null

    override fun logError(tag: String?, message: String?) { Log.e(tag ?: TAG, message ?: "") }
    override fun logWarn(tag: String?, message: String?) { Log.w(tag ?: TAG, message ?: "") }
    override fun logInfo(tag: String?, message: String?) { Log.i(tag ?: TAG, message ?: "") }
    override fun logDebug(tag: String?, message: String?) { Log.d(tag ?: TAG, message ?: "") }
    override fun logVerbose(tag: String?, message: String?) { Log.v(tag ?: TAG, message ?: "") }

    override fun logStackTraceWithMessage(tag: String?, message: String?, e: Exception?) {
        Log.e(tag ?: TAG, message ?: "", e)
    }

    override fun logStackTrace(tag: String?, e: Exception?) {
        Log.e(tag ?: TAG, "", e)
    }

    private companion object {
        const val TAG = "MuxyTerminal"
    }
}
