interface XalHookSession {
  kind: string;
}

interface XalHookContext {
  session: XalHookSession;
}

interface XalTurnEndInput {
  output?: string | Record<string, unknown>;
}

interface XalPluginContext {
  registerHook(hook: Record<string, unknown>): void;
}

type Phase = "working" | "finished";

const HOOK_TIMEOUT_MS = 2000;
const MAX_BODY_LENGTH = 200;

const stagedHookBinaryPath = () => {
  if (process.env.MUXY_HOOK_BIN) return process.env.MUXY_HOOK_BIN;
  if (!process.env.HOME) return "";
  return `${process.env.HOME}/Library/Application Support/Muxy/hooks/muxy-hook`;
};

const sanitize = (value: unknown) => {
  if (typeof value !== "string") return "";
  return value.replace(/[\n\r|]+/g, " ").trim().slice(0, MAX_BODY_LENGTH);
};

const normalizedHookInput = (phase: Phase, body: string) => {
  if (phase === "working") return ["user-prompt-submit", {}] as const;
  return [
    "stop",
    { last_assistant_message: body || "Session completed" },
  ] as const;
};

const invokeHookBinary = async (phase: Phase, body: string) => {
  const hookBinary = stagedHookBinaryPath();
  if (!hookBinary) return false;
  try {
    const { access, constants } = await import("node:fs/promises");
    await access(hookBinary, constants.X_OK);
  } catch {
    return false;
  }

  const [event, input] = normalizedHookInput(phase, body);
  try {
    const { spawn } = await import("node:child_process");
    const child = spawn(
      hookBinary,
      [
        "agent-event",
        "--provider",
        "xal",
        "--provider-title",
        "Xal",
        "--event",
        event,
      ],
      { env: process.env, stdio: ["pipe", "ignore", "ignore"] },
    );
    await new Promise<void>((resolve) => {
      let settled = false;
      const finish = () => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        resolve();
      };
      const timer = setTimeout(() => {
        child.kill("SIGKILL");
        finish();
      }, HOOK_TIMEOUT_MS);
      child.on("error", finish);
      child.on("close", finish);
      child.stdin.on("error", () => {
        child.kill("SIGKILL");
        finish();
      });
      child.stdin.end(JSON.stringify(input));
    });
  } catch {}
  return true;
};

const sendEvent = async (phase: Phase, body = "") => {
  if (await invokeHookBinary(phase, body)) return;
  process.stderr.write(
    `[muxy-xal] muxy-hook binary is not staged; skipping ${phase} event\n`,
  );
};

export default {
  name: "muxy",
  register(ctx: XalPluginContext) {
    ctx.registerHook({
      name: "notify",
      async prompt(_input: unknown, hookCtx: XalHookContext) {
        if (hookCtx.session.kind !== "primary") return undefined;
        await sendEvent("working");
        return undefined;
      },
      async turnEnd(input: XalTurnEndInput, hookCtx: XalHookContext) {
        if (hookCtx.session.kind !== "primary") return;
        await sendEvent("finished", sanitize(input.output));
      },
    });
  },
};
