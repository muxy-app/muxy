import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

export default function (pi: ExtensionAPI) {
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
    process.stderr.write(
      `[muxy-pi] muxy-hook binary is not staged; skipping ${phase} event\n`,
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
