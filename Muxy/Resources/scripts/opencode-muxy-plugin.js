const childSessions = new Set()
const sessionsWithCancelledTurnWaitingForIdle = new Set()
const replyDeadlines = new Map()

const REPLY_SUPPRESSION_MS = 1500
const MAX_BODY_LENGTH = 200
const PERMISSION_DETAIL_FIELDS = [
  "command",
  "pattern",
  "path",
  "filePath",
  "url",
  "title",
]

function sanitize(text) {
  if (typeof text !== "string") return ""
  return text.replace(/[\n\r|]+/g, " ").trim().slice(0, MAX_BODY_LENGTH)
}

function firstNonEmpty(...values) {
  for (const value of values) {
    if (typeof value === "string" && value.trim().length > 0) return value.trim()
  }
  return ""
}

function permissionBody(properties) {
  const tool = firstNonEmpty(properties.tool)
  const metadata = properties.metadata || {}
  const detailFromMetadata = firstNonEmpty(
    ...PERMISSION_DETAIL_FIELDS.map((key) => metadata[key]),
  )
  const detailFromPatterns = Array.isArray(properties.patterns)
    ? firstNonEmpty(...properties.patterns)
    : ""
  const detail = detailFromMetadata || detailFromPatterns
  if (tool && detail) return `Permission needed: ${tool} - ${detail}`
  if (tool) return `Permission needed: ${tool}`
  if (detail) return `Permission needed: ${detail}`
  return "Permission needed"
}

function questionBody(properties) {
  const list = Array.isArray(properties.questions) ? properties.questions : []
  const first = list[0] || {}
  const header = firstNonEmpty(first.header)
  const text = firstNonEmpty(first.question, first.prompt, first.text)
  const more = list.length > 1 ? ` (+${list.length - 1} more)` : ""
  if (header && text) return `Question: ${header} - ${text}${more}`
  if (text) return `Question: ${text}${more}`
  if (header) return `Question: ${header}${more}`
  return "Question waiting"
}

function markRecentReply(sessionID) {
  if (!sessionID) return
  replyDeadlines.set(sessionID, Date.now() + REPLY_SUPPRESSION_MS)
}

function consumeRecentReply(sessionID) {
  const deadline = replyDeadlines.get(sessionID)
  if (deadline === undefined) return false
  replyDeadlines.delete(sessionID)
  return Date.now() <= deadline
}

async function sendNotification(socketPath, paneID, body) {
  const payload = `opencode|${paneID}|OpenCode|${sanitize(body)}`
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
}

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
      }
      return
    }

    if (event.type === "permission.asked") {
      if (childSessions.has(event.properties.sessionID)) return
      await sendNotification(socketPath, paneID, permissionBody(event.properties))
      return
    }

    if (event.type === "permission.replied") {
      markRecentReply(event.properties.sessionID)
      return
    }

    if (event.type === "question.asked") {
      if (childSessions.has(event.properties.sessionID)) return
      await sendNotification(socketPath, paneID, questionBody(event.properties))
      return
    }

    if (event.type === "question.replied" || event.type === "question.rejected") {
      markRecentReply(event.properties.sessionID)
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
    if (consumeRecentReply(sessionID)) return

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
        if (text) body = text
      }
    } catch {}

    await sendNotification(socketPath, paneID, body)
  },
})
