import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

export default function (pi: ExtensionAPI) {
  async function sendDirect(socketPath: string, payload: string) {
    try {
      const { createConnection } = await import("node:net");
      const conn = createConnection({ path: socketPath });
      conn.on("error", (err: any) => {
        process.stderr.write(`[muxy-pi] socket error: ${err?.message ?? err}\n`);
      });
      conn.write(`${payload}\n`, () => conn.end());
      await new Promise((resolve) => {
        conn.on("close", resolve);
        setTimeout(resolve, 3000);
      });
    } catch (err: any) {
      process.stderr.write(`[muxy-pi] connection error: ${err?.message ?? err}\n`);
    }
  }

  const stagedHookBinaryPath = () => {
    if (process.env.MUXY_HOOK_BIN) return process.env.MUXY_HOOK_BIN;
    if (!process.env.HOME) return "";
    return `${process.env.HOME}/Library/Application Support/Muxy/hooks/muxy-hook`;
  };

  const normalizedHookInput = (phase: string, title: string, body: string) => {
    if (phase === "working") return ["user-prompt-submit", {}] as const;
    if (phase === "waiting") {
      return [
        "notification",
        {
          notification_type: "permission_prompt",
          message: body || "Needs attention",
        },
      ] as const;
    }
    if (!title && !body) return ["session-end", {}] as const;
    return [
      "stop",
      { last_assistant_message: body || "Session completed" },
    ] as const;
  };

  const invokeHookBinary = async (
    phase: string,
    title: string,
    body: string,
  ) => {
    const hookBinary = stagedHookBinaryPath();
    if (!hookBinary) return false;
    try {
      const { access } = await import("node:fs/promises");
      await access(hookBinary, 1);
    } catch {
      return false;
    }

    const [event, input] = normalizedHookInput(phase, title, body);
    try {
      const { spawn } = await import("node:child_process");
      const child = spawn(
        hookBinary,
        [
          "agent-event",
          "--provider",
          "pi",
          "--provider-title",
          "Pi",
          "--event",
          event,
        ],
        { env: process.env, stdio: ["pipe", "ignore", "ignore"] },
      );
      child.stdin.on("error", () => {});
      child.stdin.end(JSON.stringify(input));
      await new Promise((resolve) => {
        child.on("error", resolve);
        child.on("close", resolve);
      });
    } catch {}
    return true;
  };

  const sendEvent = async (phase: string, title = "", body = "") => {
    if (await invokeHookBinary(phase, title, body)) return;
    const socketPath = process.env.MUXY_SOCKET_PATH;
    const paneID = process.env.MUXY_PANE_ID;
    if (!socketPath || !paneID) return;
    await sendDirect(
      socketPath,
      `agent_event|pi|${paneID}|${phase}|${title}|${body}`,
    );
  };
  let latestBody = "Session completed";
  let fallback: ReturnType<typeof setTimeout> | undefined;
  let turnActive = false;

  const extractBody = (messages: any[]) => {
    const lastAssistant = [...messages]
      .reverse()
      .find((message: any) => message.role === "assistant");
    if (!lastAssistant) return "Session completed";
    const content = lastAssistant.content;
    const text =
      typeof content === "string"
        ? content
        : (Array.isArray(content)
            ? content
                .filter((part: any) => part.type === "text")
                .map((part: any) => part.text ?? "")
                .join("")
            : "");
    if (!text) return "Session completed";
    return text.replace(/[\n\r|]+/g, " ").slice(0, 200);
  };

  const finish = async () => {
    if (!turnActive) return;
    turnActive = false;
    if (fallback) clearTimeout(fallback);
    fallback = undefined;
    await sendEvent("finished", "Pi", latestBody);
  };

  pi.on("agent_start", () => {
    if (fallback) clearTimeout(fallback);
    fallback = undefined;
    turnActive = true;
    latestBody = "Session completed";
    return sendEvent("working");
  });

  pi.on("agent_end", async (event, _ctx) => {
    try {
      latestBody = extractBody(event.messages ?? []);
    } catch {}
    if ((event as any).willRetry) return;
    if (fallback) clearTimeout(fallback);
    fallback = setTimeout(() => {
      void finish();
    }, 1500);
  });

  try {
    (pi as any).on("agent_settled", finish);
  } catch {}
}
