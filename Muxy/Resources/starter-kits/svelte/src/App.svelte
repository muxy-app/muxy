<script lang="ts">
  import { onDestroy } from "svelte";

  let refreshes = $state(0);
  const off = muxy.events.subscribe("command.refresh-hello", () => (refreshes += 1));

  onDestroy(() => off?.());
</script>

<div class="panel">
  <div class="title">Hello from Muxy</div>
  <p class="caption">A starter panel that follows the theme and the sizing scale.</p>
  <div class="card">
    <span>Refreshes</span>
    <span class="count">{refreshes}</span>
  </div>
  <button class="button" onclick={() => (refreshes += 1)}>Refresh</button>
</div>
