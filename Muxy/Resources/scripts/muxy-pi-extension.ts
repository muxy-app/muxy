import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

export default function (pi: ExtensionAPI) {
  const socketPath = process.env.MUXY_SOCKET_PATH;
  const paneID = process.env.MUXY_PANE_ID;
  if (!socketPath || !paneID) return;

  pi.on("agent_end", async (event, _ctx) => {
    let body = "Session completed";

    try {
      const messages = event.messages ?? [];
      const lastAssistant = [...messages]
        .reverse()
        .find((m: any) => m.role === "assistant");
      if (lastAssistant) {
        const content = lastAssistant.content;
        const text =
          typeof content === "string"
            ? content
            : (Array.isArray(content)
                ? content
                    .filter((p: any) => p.type === "text")
                    .map((p: any) => p.text ?? "")
                    .join("")
                : "");
        if (text) {
          body = text.replace(/[\n\r|]+/g, " ").slice(0, 200);
        }
      }
    } catch {}

    const payload = `pi|${paneID}|Pi|${body}\n`;

    try {
      const { createConnection } = await import("node:net");
      const conn = createConnection({ path: socketPath });
      conn.on("error", (err: any) => {
        process.stderr.write(`[muxy-pi] socket error: ${err?.message ?? err}\n`);
      });
      conn.write(payload, () => conn.end());
      await new Promise((resolve) => {
        conn.on("close", resolve);
        setTimeout(resolve, 3000);
      });
    } catch (err: any) {
      process.stderr.write(`[muxy-pi] connection error: ${err?.message ?? err}\n`);
    }
  });
}
