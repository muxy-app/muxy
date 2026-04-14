export const MuxyNotificationPlugin = async ({ client }) => ({
  event: async ({ event }) => {
    const socketPath = process.env.MUXY_SOCKET_PATH
    const paneID = process.env.MUXY_PANE_ID
    if (!socketPath || !paneID) return
    if (event.type !== "session.idle") return

    const sessionID = event.properties.sessionID
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
          body = text
            .replace(/[\n\r]+/g, " ")
            .replace(/"/g, "")
            .replace(/\\/g, "")
            .slice(0, 200)
        }
      }
    } catch {}

    const { execSync } = await import("child_process")
    const payload = JSON.stringify({
      type: "opencode",
      paneID,
      title: "OpenCode",
      body,
    })
    try {
      execSync(`echo '${payload}' | nc -U '${socketPath}'`, {
        timeout: 3000,
      })
    } catch {}
  },
})
