import "./styles.css";
import type {} from "./types/muxy";

const root = document.getElementById("root");

function render(count: number) {
  if (!root) return;
  root.innerHTML = `
    <div class="panel">
      <div class="title">Hello from Muxy</div>
      <p class="caption">A starter panel that follows the theme and the sizing scale.</p>
      <div class="card">
        <span>Refreshes</span>
        <span class="count">${count}</span>
      </div>
      <button class="button" id="refresh">Refresh</button>
    </div>
  `;
  root.querySelector<HTMLButtonElement>("#refresh")?.addEventListener("click", () => bump());
}

let refreshes = 0;
function bump() {
  refreshes += 1;
  render(refreshes);
}

render(refreshes);
muxy.events.subscribe("command.refresh-hello", () => bump());
