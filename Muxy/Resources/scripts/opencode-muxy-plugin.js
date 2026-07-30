const childSessions = new Set()
const sessionsFinishedBeforeIdle = new Set()
const replyDeadlines = new Map()
const sessionVersions = new Map()
const activeSessions = new Set()
let sendQueue = Promise.resolve()

const REPLY_SUPPRESSION_MS = 1500
const HOOK_TIMEOUT_MS = 2000
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
  const permission = firstNonEmpty(properties.permission)
  const metadata = properties.metadata || {}
  const detailFromMetadata = firstNonEmpty(
    ...PERMISSION_DETAIL_FIELDS.map((key) => metadata[key]),
  )
  const detailFromPatterns = Array.isArray(properties.patterns)
    ? firstNonEmpty(...properties.patterns)
    : ""
  const detail = detailFromMetadata || detailFromPatterns
  if (permission && detail)
    return `Permission needed: ${permission} - ${detail}`
  if (permission) return `Permission needed: ${permission}`
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
  if (!hookBinary) return { delivered: false, reason: "hook path unavailable" }
  try {
    const { access, constants } = await import("node:fs/promises")
    await access(hookBinary, constants.X_OK)
  } catch (error) {
    return {
      delivered: false,
      reason: error instanceof Error ? error.message : "hook binary unavailable",
    }
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
    return await new Promise((resolve) => {
      let settled = false
      const finish = (result) => {
        if (settled) return
        settled = true
        clearTimeout(timer)
        resolve(result)
      }
      const timer = setTimeout(() => {
        child.kill("SIGKILL")
        finish({ delivered: false, reason: "muxy-hook timed out" })
      }, HOOK_TIMEOUT_MS)
      child.on("error", (error) => {
        finish({ delivered: false, reason: error.message })
      })
      child.stdin.on("error", (error) => {
        child.kill("SIGKILL")
        finish({ delivered: false, reason: error.message })
      })
      child.on("close", (code) => {
        finish(
          code === 0
            ? { delivered: true, reason: "" }
            : { delivered: false, reason: `muxy-hook exited with ${code}` },
        )
      })
      child.stdin.end(JSON.stringify(input))
    })
  } catch (error) {
    return {
      delivered: false,
      reason: error instanceof Error ? error.message : "hook launch failed",
    }
  }
}

function sendEvent(report, phase, title = "", body = "") {
  const cleanTitle = sanitize(title)
  const cleanBody = sanitize(body)
  const transmit = async () => {
    const result = await invokeHookBinary(phase, cleanTitle, cleanBody)
    if (result.delivered) return true
    await report("error", "Muxy hook delivery failed", {
      phase,
      reason: result.reason,
    })
    process.stderr.write(
      `[muxy-opencode] ${result.reason}; skipping ${phase} event\n`,
    )
    return false
  }
  sendQueue = sendQueue.then(transmit, transmit)
  return sendQueue
}

function reporter(client) {
  return async (level, message, extra = {}) => {
    try {
      await client.app.log({
        body: { service: "muxy", level, message, extra },
      })
    } catch {}
  }
}

export const MuxyNotificationPlugin = async ({ client }) => {
  const report = reporter(client)
  const send = (phase, title = "", body = "") =>
    sendEvent(report, phase, title, body)
  await report("info", "Muxy notification plugin initialized", {
    hookBinary: stagedHookBinaryPath() || "unavailable",
  })

  return {
    event: async ({ event }) => {
      if (event.type === "session.created") {
        const info = event.properties.info
        const sessionID = info?.id || event.properties.sessionID
        if (info?.parentID && sessionID) childSessions.add(sessionID)
        return
      }

      if (event.type === "session.deleted") {
        const sessionID =
          event.properties.info?.id || event.properties.sessionID
        if (!sessionID) return
        if (activeSessions.has(sessionID) && !childSessions.has(sessionID)) {
          await send("finished")
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
          if (!childSessions.has(sessionID)) await send("finished")
          clearSettledSession(sessionID, version)
          return
        }
        const version = advanceSession(sessionID)
        if (!childSessions.has(sessionID)) {
          const body = firstNonEmpty(
            err?.data?.message,
            err?.message,
            err?.name,
            "Session failed",
          )
          await send("finished", "OpenCode", body)
        }
        clearSettledSession(sessionID, version)
        return
      }

      if (event.type === "permission.asked") {
        if (childSessions.has(event.properties.sessionID)) return
        const sessionID = event.properties.sessionID
        advanceSession(sessionID)
        activeSessions.add(sessionID)
        const delivered = await send(
          "waiting",
          "OpenCode",
          permissionBody(event.properties),
        )
        if (delivered) {
          await report("info", "Muxy attention event forwarded", {
            event: event.type,
            sessionID,
          })
        }
        return
      }

      if (event.type === "permission.replied") {
        const sessionID = event.properties.sessionID
        markRecentReply(sessionID)
        advanceSession(sessionID)
        if (!childSessions.has(sessionID)) await send("working")
        return
      }

      if (event.type === "question.asked") {
        if (childSessions.has(event.properties.sessionID)) return
        const sessionID = event.properties.sessionID
        advanceSession(sessionID)
        activeSessions.add(sessionID)
        const delivered = await send(
          "waiting",
          "OpenCode",
          questionBody(event.properties),
        )
        if (delivered) {
          await report("info", "Muxy attention event forwarded", {
            event: event.type,
            sessionID,
          })
        }
        return
      }

      if (
        event.type === "question.replied" ||
        event.type === "question.rejected"
      ) {
        const sessionID = event.properties.sessionID
        markRecentReply(sessionID)
        advanceSession(sessionID)
        if (!childSessions.has(sessionID)) await send("working")
        return
      }

      if (event.type !== "session.status") return

      const sessionID = event.properties.sessionID
      if (event.properties.status.type !== "idle") {
        advanceSession(sessionID)
        activeSessions.add(sessionID)
        if (!childSessions.has(sessionID)) await send("working")
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
          await send("finished")
          clearSettledSession(sessionID, version)
        }, REPLY_SUPPRESSION_MS)
        return
      }
      const version = sessionVersions.get(sessionID)
      await send("finished", "OpenCode", "Session completed")
      clearSettledSession(sessionID, version)
    },
  }
}
