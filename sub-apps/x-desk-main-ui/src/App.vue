<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";

const exitMainUi = async () => {
  await invoke("exit_main_ui");
};
</script>

<template>
  <main class="app-shell" aria-label="X Desk">
    <header class="titlebar" aria-label="Title Bar">
      <div class="brand">
        <span class="app-name">X Desk</span>
      </div>

      <button class="close-button" type="button" aria-label="Exit MainUI" @click="exitMainUi">
        <span aria-hidden="true">×</span>
      </button>
    </header>

    <section class="content-area" aria-label="Main content area" />
  </main>
</template>

<style>
:root {
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  color-scheme: light dark;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
  width: 100%;
  height: 100%;
  --window-bg: #f4f5f7;
  --titlebar-bg: rgba(255, 255, 255, 0.78);
  --titlebar-border: rgba(0, 0, 0, 0.08);
  --text-color: #1f2937;
  --muted-color: #6b7280;
  --close-fg: #5f6368;
  --close-bg: transparent;
  --close-bg-hover: #e81123;
  --close-bg-active: #c50f1f;
  --close-fg-hover: #ffffff;
  --close-ring: rgba(0, 95, 184, 0.72);
}

@media (prefers-color-scheme: dark) {
  :root {
    --window-bg: #181818;
    --titlebar-bg: rgba(32, 32, 32, 0.88);
    --titlebar-border: rgba(255, 255, 255, 0.08);
    --text-color: #f3f4f6;
    --muted-color: #9ca3af;
    --close-fg: #d1d5db;
    --close-ring: rgba(96, 165, 250, 0.85);
  }
}

html,
body,
#app {
  width: 100%;
  height: 100%;
  margin: 0;
}

body {
  overflow: hidden;
  background: var(--window-bg);
}

.app-shell {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--window-bg);
  color: var(--text-color);
}

.titlebar {
  height: 64px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 16px 0 18px;
  background: var(--titlebar-bg);
  border-bottom: 1px solid var(--titlebar-border);
  -webkit-app-region: drag;
  user-select: none;
}

.brand {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.app-name {
  font-size: 16px;
  font-weight: 600;
  letter-spacing: 0.01em;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.close-button {
  width: 46px;
  height: 36px;
  border: 0;
  border-radius: 8px;
  background: var(--close-bg);
  color: var(--close-fg);
  font-family: "Segoe UI Symbol", "Segoe UI", sans-serif;
  font-size: 24px;
  font-weight: 300;
  line-height: 1;
  cursor: pointer;
  display: grid;
  place-items: center;
  -webkit-app-region: no-drag;
  transition: background-color 90ms ease, color 90ms ease, transform 60ms ease;
}

.close-button:hover {
  background: var(--close-bg-hover);
  color: var(--close-fg-hover);
}

.close-button:active {
  background: var(--close-bg-active);
  color: var(--close-fg-hover);
  transform: scale(0.98);
}

.close-button span {
  transform: translateY(-2px);
}

.close-button:focus-visible {
  outline: 2px solid var(--close-ring);
  outline-offset: 2px;
}

.content-area {
  flex: 1;
  background: var(--window-bg);
}
</style>
