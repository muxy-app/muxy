import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

export default function (pi: ExtensionAPI) {
  const socketPath = process.env.MUXY_SOCKET_PATH;
  const paneID = process.env.MUXY_PANE_ID;
  if (!socketPath || !paneID) return;

  async function send(payload: string) {
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

  const sendEvent = async (phase: string, title = "", body = "") => {
    if (process.env.MUXY_AGENT_EVENT_PROTOCOL === "2") {
      await send(`agent_event|pi|${paneID}|${phase}|${title}|${body}`);
      return;
    }
    const status = phase === "finished" ? "idle" : phase;
    if (title || body) {
      await send(
        `agent_status|pi|${paneID}|${status}\npi|${paneID}|${title}|${body}`,
      );
      return;
    }
    await send(`agent_status|pi|${paneID}|${status}`);
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
