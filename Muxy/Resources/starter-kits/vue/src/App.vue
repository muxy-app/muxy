<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";

const refreshes = ref(0);
let off: (() => void) | undefined;

onMounted(() => {
  off = muxy.events.subscribe("command.refresh-hello", () => (refreshes.value += 1));
});

onUnmounted(() => off?.());
</script>

<template>
  <div class="panel">
    <div class="title">Hello from Muxy</div>
    <p class="caption">A starter panel that follows the theme and the sizing scale.</p>
    <div class="card">
      <span>Refreshes</span>
      <span class="count">{{ refreshes }}</span>
    </div>
    <button class="button" @click="refreshes += 1">Refresh</button>
  </div>
</template>
