import { clear, h } from "@/lib/dom";
import { icon } from "@/lib/icons";

export class HelloPanel {
  constructor(root) {
    this.root = root;
    this.refreshes = 0;
  }

  start() {
    muxy.events.subscribe("command.refresh-hello", () => this.bump());
    this.render();
  }

  bump() {
    this.refreshes += 1;
    this.render();
  }

  render() {
    clear(this.root);
    this.root.appendChild(this.view());
  }

  view() {
    return h(
      "div",
      { class: "flex h-full flex-col gap-3 p-3" },
      h(
        "div",
        { class: "flex items-center gap-2 text-[14px] font-semibold text-foreground" },
        icon("sparkles", 16, "text-primary"),
        "Hello from Muxy",
      ),
      h(
        "p",
        { class: "text-[11px] text-muted-foreground" },
        "A vanilla JS + Tailwind panel that follows the app theme.",
      ),
      h(
        "div",
        {
          class:
            "flex items-center justify-between rounded-md border border-border bg-surface px-3 py-2 text-[12px] text-foreground",
        },
        h("span", null, "Refreshes"),
        h("span", { class: "font-mono font-semibold text-primary" }, String(this.refreshes)),
      ),
      h(
        "button",
        {
          type: "button",
          class:
            "flex h-8 items-center justify-center gap-1.5 rounded-md bg-primary px-3 text-[12px] font-medium text-primary-foreground outline-none transition-opacity hover:opacity-95",
          onclick: () => this.bump(),
        },
        icon("refresh", 13),
        "Refresh",
      ),
    );
  }
}
