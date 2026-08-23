import { spawn } from "node:child_process";
import type { ExtensionAPI } from "@oh-my-pi/pi-coding-agent";

const HOOK_TIMEOUT_MS = 2000;
const SANITIZE_PATTERN = /(?:\x1B\[[0-9;]*[a-zA-Z]|[\x00-\x1F\x7F|])/g;

type Phase = "working" | "waiting" | "finished";
type DeliveryResult = { ok: true } | { ok: false; reason: string };

function stagedHookBinaryPath(): string {
  if (process.env.MUXY_HOOK_BIN) return process.env.MUXY_HOOK_BIN;
  if (!process.env.HOME) return "";
  return `${process.env.HOME}/Library/Application Support/Muxy/hooks/muxy-hook`;
}

function sanitize(text: string): string {
  return text.replace(SANITIZE_PATTERN, " ").trim().slice(0, 200);
}

function assistantBody(messages: unknown[]): string {
  for (let index = messages.length - 1; index >= 0; index--) {
    const message = messages[index];
    if (!message || typeof message !== "object" || !("role" in message) || message.role !== "assistant") continue;
    if ("content" in message) {
      if (typeof message.content === "string") {
        const text = sanitize(message.content);
        if (text) return text;
      } else if (Array.isArray(message.content)) {
        let text = "";
        for (const part of message.content) {
          if (
            part &&
            typeof part === "object" &&
            "type" in part &&
            part.type === "text" &&
            "text" in part &&
            typeof part.text === "string"
          ) {
            text += part.text;
          }
        }
        const body = sanitize(text);
        if (body) return body;
      }
    }
    if ("errorMessage" in message && typeof message.errorMessage === "string") {
      return sanitize(message.errorMessage) || "Session completed";
    }
    return "Session completed";
  }
  return "Session completed";
}

function questionBody(args: unknown): string {
  if (!args || typeof args !== "object" || !("questions" in args) || !Array.isArray(args.questions)) {
    return "Question waiting";
  }
  const first = args.questions[0];
  if (!first || typeof first !== "object") return "Question waiting";
  const header = "header" in first && typeof first.header === "string" ? sanitize(first.header) : "";
  const question = "question" in first && typeof first.question === "string" ? sanitize(first.question) : "";
  const more = args.questions.length > 1 ? ` (+${args.questions.length - 1} more)` : "";
  if (header && question) return `Question: ${header} - ${question}${more}`;
  if (question) return `Question: ${question}${more}`;
  if (header) return `Question: ${header}${more}`;
  return "Question waiting";
}

export default function (omp: ExtensionAPI) {
  let active = false;

  const deliver = async (event: string, input: object): Promise<DeliveryResult> => {
    const hookBinary = stagedHookBinaryPath();
    if (!hookBinary) return { ok: false, reason: "hook path unavailable" };

    try {
      const child = spawn(
        hookBinary,
        [
          "agent-event",
          "--provider",
          "omp",
          "--provider-title",
          "Oh My Pi",
          "--event",
          event,
        ],
        { env: process.env, stdio: ["pipe", "ignore", "ignore"] }
      );
      return await new Promise<DeliveryResult>((resolve) => {
        let settled = false;
        const finish = (result: DeliveryResult) => {
          if (settled) return;
          settled = true;
          clearTimeout(timer);
          resolve(result);
        };
        const timer = setTimeout(() => {
          child.kill("SIGKILL");
          finish({ ok: false, reason: "muxy-hook timed out" });
        }, HOOK_TIMEOUT_MS);
        timer.unref?.();
        child.on("error", (error) => finish({ ok: false, reason: error.message }));
        child.stdin.on("error", (error) => finish({ ok: false, reason: error.message }));
        child.on("close", (code) =>
          finish(code === 0 ? { ok: true } : { ok: false, reason: `muxy-hook exited with ${code}` })
        );
        child.stdin.end(JSON.stringify(input));
      });
    } catch (error) {
      return { ok: false, reason: error instanceof Error ? error.message : "muxy-hook launch failed" };
    }
  };

  const send = async (phase: Phase, body = "", waitingType = "permission_prompt") => {
    const [event, input] =
      phase === "working"
        ? ["user-prompt-submit", {}]
        : phase === "waiting"
          ? ["notification", { notification_type: waitingType, message: body || "Needs attention" }]
          : body
            ? ["stop", { last_assistant_message: body }]
            : ["session-end", {}];
    const result = await deliver(event, input);
    if (!result.ok) {
      process.stderr.write(`[muxy-omp] failed to deliver ${phase} event: ${result.reason}\n`);
    }
  };

  omp.on("before_agent_start", (_event, ctx) => {
    if (!ctx.hasUI) return;
    active = true;
    return send("working");
  });

  omp.on("tool_approval_requested", (event, ctx) => {
    if (!ctx.hasUI) return;
    active = true;
    const reason = event.reason ? ` - ${sanitize(event.reason)}` : "";
    return send("waiting", `Approval needed: ${sanitize(event.toolName)}${reason}`);
  });

  omp.on("tool_approval_resolved", (_event, ctx) => {
    if (!ctx.hasUI) return;
    active = true;
    return send("working");
  });

  omp.on("tool_execution_start", (event, ctx) => {
    if (!ctx.hasUI || event.toolName !== "ask") return;
    active = true;
    return send("waiting", questionBody(event.args), "elicitation_dialog");
  });

  omp.on("tool_execution_end", (event, ctx) => {
    if (!ctx.hasUI || event.toolName !== "ask") return;
    active = true;
    return send("working");
  });

  omp.on("agent_end", (event, ctx) => {
    if (!ctx.hasUI || event.willContinue) return;
    active = false;
    return send("finished", assistantBody(event.messages));
  });

  omp.on("session_shutdown", (_event, ctx) => {
    if (!ctx.hasUI || !active) return;
    active = false;
    return send("finished");
  });
}
