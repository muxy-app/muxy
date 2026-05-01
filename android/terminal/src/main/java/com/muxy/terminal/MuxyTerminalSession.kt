package com.muxy.terminal

import com.muxy.net.MuxyClient
import com.termux.terminal.TerminalSession
import com.termux.terminal.TerminalSessionClient
import java.util.UUID

class MuxyTerminalSession(
    private val client: MuxyClient,
    val paneID: UUID,
    sessionClient: TerminalSessionClient,
    transcriptRows: Int? = DEFAULT_TRANSCRIPT_ROWS,
) : TerminalSession(transcriptRows, sessionClient) {

    override fun write(data: ByteArray, offset: Int, count: Int) {
        if (count <= 0) return
        val payload = if (offset == 0 && count == data.size) {
            data.copyOf()
        } else {
            data.copyOfRange(offset, offset + count)
        }
        client.sendTerminalInput(paneID = paneID, bytes = payload)
    }

    fun acceptRemoteOutput(bytes: ByteArray) {
        if (bytes.isEmpty()) return
        feedRemoteOutput(bytes, bytes.size)
    }

    fun resetEmulatorScreen() {
        getEmulator()?.reset()
    }

    companion object {
        const val DEFAULT_TRANSCRIPT_ROWS: Int = 2000
    }
}
