<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { onMounted, onUnmounted } from "vue";

const blockedShortcutKeys = new Set(["F5", "F11", "F12"]);

const blockContextMenu = (event: MouseEvent) => {
  event.preventDefault();
};

const blockWebShortcuts = (event: KeyboardEvent) => {
  const key = event.key.toLowerCase();
  const isDevToolsShortcut = event.ctrlKey && event.shiftKey && ["c", "i", "j"].includes(key);
  const isReloadShortcut = event.ctrlKey && key === "r";
  const isViewSourceShortcut = event.ctrlKey && key === "u";

  if (blockedShortcutKeys.has(event.key) || isDevToolsShortcut || isReloadShortcut || isViewSourceShortcut) {
    event.preventDefault();
    event.stopPropagation();
  }
};

const exitMainUi = async () => {
  await invoke("exit_main_ui");
};

onMounted(() => {
  if (import.meta.env.PROD) {
    window.addEventListener("contextmenu", blockContextMenu, { capture: true });
    window.addEventListener("keydown", blockWebShortcuts, { capture: true });
  }
});

onUnmounted(() => {
  if (import.meta.env.PROD) {
    window.removeEventListener("contextmenu", blockContextMenu, { capture: true });
    window.removeEventListener("keydown", blockWebShortcuts, { capture: true });
  }
});
</script>

<template>
  <main class="app-shell" aria-label="X-Desk">
    <header class="titlebar" aria-label="Title Bar">
      <div class="brand">
        <span class="app-name">X-Desk</span>
      </div>

      <div class="titlebar-actions" aria-label="Window actions">
        <button class="titlebar-button" type="button" aria-label="About">
          <span class="titlebar-button-icon about-icon" aria-hidden="true" />
        </button>
        <button class="titlebar-button" type="button" aria-label="Settings">
          <span class="titlebar-button-icon settings-icon" aria-hidden="true" />
        </button>
        <button class="titlebar-button close-button" type="button" aria-label="Exit MainUI" @click="exitMainUi">
          <span class="titlebar-button-icon close-icon" aria-hidden="true" />
        </button>
      </div>
    </header>

    <section class="content-area" aria-label="Main content area" />
  </main>
</template>

<style>
:root {
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  color-scheme: dark;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
  width: 100%;
  height: 100%;
  --window-bg: #000000;
  --titlebar-bg: #000000;
  --titlebar-border: rgba(0, 230, 246, 0.34);
  --text-color: #00e6f6;
  --button-fg: #00e6f6;
  --button-bg: transparent;
  --button-bg-hover: rgba(0, 230, 246, 0.16);
  --button-bg-active: rgba(0, 230, 246, 0.28);
  --close-bg-hover: rgba(0, 230, 246, 0.22);
  --close-bg-active: rgba(0, 230, 246, 0.34);
  --button-fg-hover: #00e6f6;
  --focus-ring: #00e6f6;
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
  background:
    radial-gradient(circle at 18% 14%, rgba(0, 230, 246, 0.13), transparent 29%),
    radial-gradient(circle at 86% 78%, rgba(0, 230, 246, 0.08), transparent 34%),
    linear-gradient(145deg, #000000 0%, #020b0d 46%, #000000 100%);
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
  box-shadow: 0 0 22px rgba(0, 230, 246, 0.08);
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
  text-shadow: 0 0 14px rgba(0, 230, 246, 0.45);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.titlebar-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  -webkit-app-region: no-drag;
}

.titlebar-button {
  width: 36px;
  height: 30px;
  border: 0;
  border-radius: 8px;
  background: var(--button-bg);
  color: var(--button-fg);
  cursor: pointer;
  display: grid;
  place-items: center;
  -webkit-app-region: no-drag;
  transition: background-color 90ms ease, color 90ms ease, transform 60ms ease;
}

.titlebar-button:hover {
  background: var(--button-bg-hover);
}

.titlebar-button:active {
  background: var(--button-bg-active);
  transform: scale(0.98);
}

.titlebar-button-icon {
  width: 20px;
  height: 20px;
  background: currentColor;
  display: block;
}

.about-icon {
  -webkit-mask: url("./assets/titlebar-about.svg") center / contain no-repeat;
  mask: url("./assets/titlebar-about.svg") center / contain no-repeat;
}

.settings-icon {
  width: 22px;
  height: 22px;
  -webkit-mask: url("./assets/titlebar-settings.svg") center / contain no-repeat;
  mask: url("./assets/titlebar-settings.svg") center / contain no-repeat;
}

.close-icon {
  width: 24px;
  height: 24px;
  -webkit-mask: url("./assets/titlebar-close.svg") center / contain no-repeat;
  mask: url("./assets/titlebar-close.svg") center / contain no-repeat;
}

.close-button:hover {
  background: var(--close-bg-hover);
  color: var(--button-fg-hover);
}

.close-button:active {
  background: var(--close-bg-active);
  color: var(--button-fg-hover);
  transform: scale(0.98);
}

.titlebar-button:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: 2px;
}

.content-area {
  flex: 1;
  background:
    linear-gradient(180deg, rgba(0, 230, 246, 0.045), transparent 24%),
    radial-gradient(ellipse at center, rgba(0, 230, 246, 0.035), transparent 58%);
}
</style>
