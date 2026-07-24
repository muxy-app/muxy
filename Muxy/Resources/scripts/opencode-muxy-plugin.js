const childSessions = new Set()
const sessionsFinishedBeforeIdle = new Set()
const replyDeadlines = new Map()
const sessionVersions = new Map()
const activeSessions = new Set()
let sendQueue = Promise.resolve()

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

function advanceSession(sessionID) {
  if (!sessionID) return 0
  const version = (sessionVersions.get(sessionID) || 0) + 1
  sessionVersions.set(sessionID, version)
  return version
}

function clearSession(sessionID) {
  childSessions.delete(sessionID)
  sessionsFinishedBeforeIdle.delete(sessionID)
  replyDeadlines.delete(sessionID)
  sessionVersions.delete(sessionID)
  activeSessions.delete(sessionID)
}

function clearSettledSession(sessionID, version) {
  if (sessionVersions.get(sessionID) !== version) return
  replyDeadlines.delete(sessionID)
  sessionVersions.delete(sessionID)
  activeSessions.delete(sessionID)
}

function stagedHookBinaryPath() {
  if (process.env.MUXY_HOOK_BIN) return process.env.MUXY_HOOK_BIN
  if (!process.env.HOME) return ""
  return `${process.env.HOME}/Library/Application Support/Muxy/hooks/muxy-hook`
}

function normalizedHookInput(phase, title, body) {
  if (phase === "working") return ["user-prompt-submit", {}]
  if (phase === "waiting") {
    return [
      "notification",
      {
        notification_type: body.startsWith("Question:")
          ? "elicitation_dialog"
          : "permission_prompt",
        message: body || "Needs attention",
      },
    ]
  }
  if (!title && !body) return ["session-end", {}]
  return ["stop", { last_assistant_message: body || "Session completed" }]
}

async function invokeHookBinary(phase, title, body) {
  const hookBinary = stagedHookBinaryPath()
  if (!hookBinary) return false
  try {
    const { access } = await import("node:fs/promises")
    await access(hookBinary, 1)
  } catch {
    return false
  }

  const [event, input] = normalizedHookInput(phase, title, body)
  try {
    const { spawn } = await import("node:child_process")
    const child = spawn(
      hookBinary,
      [
        "agent-event",
        "--provider",
        "opencode",
        "--provider-title",
        "OpenCode",
        "--event",
        event,
      ],
      { env: process.env, stdio: ["pipe", "ignore", "ignore"] },
    )
    child.stdin.on("error", () => {})
    child.stdin.end(JSON.stringify(input))
    await new Promise((resolve) => {
      child.on("error", resolve)
      child.on("close", resolve)
    })
  } catch {}
  return true
}

function sendEvent(phase, title = "", body = "") {
  const cleanTitle = sanitize(title)
  const cleanBody = sanitize(body)
  const transmit = async () => {
    if (await invokeHookBinary(phase, cleanTitle, cleanBody)) return
    process.stderr.write(
      `[muxy-opencode] muxy-hook binary is not staged; skipping ${phase} event\n`,
    )
  }
  sendQueue = sendQueue.then(transmit, transmit)
  return sendQueue
}

export const MuxyNotificationPlugin = async () => ({
  event: async ({ event }) => {
    if (event.type === "session.created") {
      const info = event.properties.info
      const sessionID = info?.id || event.properties.sessionID
      if (info?.parentID && sessionID) childSessions.add(sessionID)
      return
    }

    if (event.type === "session.deleted") {
      const sessionID = event.properties.info?.id || event.properties.sessionID
      if (!sessionID) return
      if (activeSessions.has(sessionID) && !childSessions.has(sessionID)) {
        await sendEvent("finished")
      }
      clearSession(sessionID)
      return
    }

    if (event.type === "session.error") {
      const sessionID = event.properties.sessionID
      const err = event.properties.error
      if (sessionID) sessionsFinishedBeforeIdle.add(sessionID)
      if (err?.name === "MessageAbortedError") {
        const version = advanceSession(sessionID)
        if (!childSessions.has(sessionID)) await sendEvent("finished")
        clearSettledSession(sessionID, version)
        return
      }
      const version = advanceSession(sessionID)
      if (!childSessions.has(sessionID)) {
        const body = firstNonEmpty(err?.data?.message, err?.message, err?.name, "Session failed")
        await sendEvent("finished", "OpenCode", body)
      }
      clearSettledSession(sessionID, version)
      return
    }

    if (event.type === "permission.asked") {
      if (childSessions.has(event.properties.sessionID)) return
      const sessionID = event.properties.sessionID
      advanceSession(sessionID)
      activeSessions.add(sessionID)
      await sendEvent("waiting", "OpenCode", permissionBody(event.properties))
      return
    }

    if (event.type === "permission.replied") {
      const sessionID = event.properties.sessionID
      markRecentReply(sessionID)
      advanceSession(sessionID)
      if (!childSessions.has(sessionID)) await sendEvent("working")
      return
    }

    if (event.type === "question.asked") {
      if (childSessions.has(event.properties.sessionID)) return
      const sessionID = event.properties.sessionID
      advanceSession(sessionID)
      activeSessions.add(sessionID)
      await sendEvent("waiting", "OpenCode", questionBody(event.properties))
      return
    }

    if (event.type === "question.replied" || event.type === "question.rejected") {
      const sessionID = event.properties.sessionID
      markRecentReply(sessionID)
      advanceSession(sessionID)
      if (!childSessions.has(sessionID)) await sendEvent("working")
      return
    }

    if (event.type !== "session.status") return

    const sessionID = event.properties.sessionID
    if (event.properties.status.type !== "idle") {
      advanceSession(sessionID)
      activeSessions.add(sessionID)
      if (!childSessions.has(sessionID)) await sendEvent("working")
      return
    }

    if (sessionsFinishedBeforeIdle.has(sessionID)) {
      sessionsFinishedBeforeIdle.delete(sessionID)
      return
    }
    if (childSessions.has(sessionID)) return
    if (consumeRecentReply(sessionID)) {
      const version = sessionVersions.get(sessionID)
      setTimeout(async () => {
        if (sessionVersions.get(sessionID) !== version) return
        await sendEvent("finished")
        clearSettledSession(sessionID, version)
      }, REPLY_SUPPRESSION_MS)
      return
    }
    const version = sessionVersions.get(sessionID)
    await sendEvent("finished", "OpenCode", "Session completed")
    clearSettledSession(sessionID, version)
  },
})
