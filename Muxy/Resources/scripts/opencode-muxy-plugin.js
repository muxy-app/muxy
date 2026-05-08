const childSessions = new Set()
const sessionsWithCancelledTurnWaitingForIdle = new Set()

export const MuxyNotificationPlugin = async ({ client }) => ({
  event: async ({ event }) => {
    const socketPath = process.env.MUXY_SOCKET_PATH
    const paneID = process.env.MUXY_PANE_ID
    if (!socketPath || !paneID) return

    if (event.type === "session.created") {
      const info = event.properties.info
      if (info?.parentID) childSessions.add(event.properties.sessionID)
      return
    }

    if (event.type === "session.error") {
      const sessionID = event.properties.sessionID
      const err = event.properties.error
      if (err?.name === "MessageAbortedError") {
        if (sessionID) sessionsWithCancelledTurnWaitingForIdle.add(sessionID)
        return
      }
      return
    }

    if (event.type !== "session.status") return
    if (event.properties.status.type !== "idle") return

    const sessionID = event.properties.sessionID
    if (sessionsWithCancelledTurnWaitingForIdle.has(sessionID)) {
      sessionsWithCancelledTurnWaitingForIdle.delete(sessionID)
      return
    }
    if (childSessions.has(sessionID)) return

    let body = "Session completed"

    try {
      const result = await client.session.messages({
        path: { id: sessionID },
        query: { limit: 3 },
      })
      const messages = result.data || []
      const lastAssistant = [...messages]
        .reverse()
        .find((m) => m.info.role === "assistant")
      if (lastAssistant) {
        const textParts = (lastAssistant.parts || []).filter(
          (p) => p.type === "text",
        )
        const text = textParts.map((p) => p.text || "").join("")
        if (text) {
          body = text.replace(/[\n\r|]+/g, " ").slice(0, 200)
        }
      }
    } catch {}

    const payload = `opencode|${paneID}|OpenCode|${body}`

    try {
      const { createConnection } = await import("net")
      const conn = createConnection({ path: socketPath })
      conn.on("error", () => {})
      conn.write(payload, () => conn.end())
      await new Promise((resolve) => {
        conn.on("close", resolve)
        setTimeout(resolve, 3000)
      })
    } catch {}
  },
})
