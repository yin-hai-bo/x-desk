<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { computed, onMounted, onUnmounted, ref } from "vue";

type MonitorRect = {
  left: number;
  top: number;
  right: number;
  bottom: number;
  width: number;
  height: number;
};

type MonitorViewModel = {
  index: number;
  isPrimary: boolean;
  rect: MonitorRect;
};

type MonitorLayoutViewModel = {
  monitors: MonitorViewModel[];
};

const blockedShortcutKeys = new Set(["F5", "F11", "F12"]);
const monitorLayout = ref<MonitorLayoutViewModel | null>(null);
const monitorLayoutError = ref<string | null>(null);
const isMonitorLayoutLoading = ref(false);
const monitorMapElement = ref<HTMLElement | null>(null);
const monitorMapSize = ref({ width: 0, height: 0 });
let monitorMapResizeObserver: ResizeObserver | null = null;

const virtualDesktopBounds = computed(() => {
  const monitors = monitorLayout.value?.monitors ?? [];
  if (monitors.length === 0) {
    return null;
  }

  const left = Math.min(...monitors.map((monitor) => monitor.rect.left));
  const top = Math.min(...monitors.map((monitor) => monitor.rect.top));
  const right = Math.max(...monitors.map((monitor) => monitor.rect.right));
  const bottom = Math.max(...monitors.map((monitor) => monitor.rect.bottom));

  return {
    left,
    top,
    width: Math.max(right - left, 1),
    height: Math.max(bottom - top, 1),
  };
});

const monitorStyle = (monitor: MonitorViewModel) => {
  const bounds = virtualDesktopBounds.value;
  if (!bounds) {
    return {};
  }

  return {
    left: `${((monitor.rect.left - bounds.left) / bounds.width) * 100}%`,
    top: `${((monitor.rect.top - bounds.top) / bounds.height) * 100}%`,
    width: `${(monitor.rect.width / bounds.width) * 100}%`,
    height: `${(monitor.rect.height / bounds.height) * 100}%`,
  };
};

const monitorCanvasStyle = computed(() => {
  const bounds = virtualDesktopBounds.value;
  const availableWidth = monitorMapSize.value.width;
  const availableHeight = monitorMapSize.value.height;

  if (!bounds || availableWidth === 0 || availableHeight === 0) {
    return {};
  }

  const boundsRatio = bounds.width / bounds.height;
  const availableRatio = availableWidth / availableHeight;
  const width = availableRatio > boundsRatio ? availableHeight * boundsRatio : availableWidth;
  const height = availableRatio > boundsRatio ? availableHeight : availableWidth / boundsRatio;

  return {
    width: `${width}px`,
    height: `${height}px`,
  };
});

const loadMonitorLayout = async () => {
  isMonitorLayoutLoading.value = true;
  monitorLayoutError.value = null;

  try {
    monitorLayout.value = await invoke<MonitorLayoutViewModel>("monitor_layout_view_model");
  } catch (error) {
    monitorLayoutError.value = error instanceof Error ? error.message : String(error);
  } finally {
    isMonitorLayoutLoading.value = false;
  }
};

const updateMonitorMapSize = () => {
  if (!monitorMapElement.value) {
    return;
  }

  monitorMapSize.value = {
    width: monitorMapElement.value.clientWidth,
    height: monitorMapElement.value.clientHeight,
  };
};

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
  void loadMonitorLayout();

  if (monitorMapElement.value) {
    monitorMapResizeObserver = new ResizeObserver(([entry]) => {
      monitorMapSize.value = {
        width: entry.contentRect.width,
        height: entry.contentRect.height,
      };
    });
    monitorMapResizeObserver.observe(monitorMapElement.value);
    updateMonitorMapSize();
  }

  if (import.meta.env.PROD) {
    window.addEventListener("contextmenu", blockContextMenu, { capture: true });
    window.addEventListener("keydown", blockWebShortcuts, { capture: true });
  }
});

onUnmounted(() => {
  monitorMapResizeObserver?.disconnect();
  monitorMapResizeObserver = null;

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

    <section class="content-area" aria-label="Main content area">
      <div class="monitor-panel">
        <div class="monitor-panel-header">
          <div>
            <h1 class="monitor-panel-title">Monitor Layout</h1>
          </div>
          <button class="refresh-button" type="button" :disabled="isMonitorLayoutLoading" @click="loadMonitorLayout">
            {{ isMonitorLayoutLoading ? "Scanning" : "Refresh" }}
          </button>
        </div>

        <p v-if="monitorLayoutError" class="monitor-error" role="alert">{{ monitorLayoutError }}</p>

        <div ref="monitorMapElement" class="monitor-map" aria-label="Detected monitors">
          <div v-if="monitorLayout?.monitors.length" class="monitor-canvas" :style="monitorCanvasStyle">
            <article
              v-for="monitor in monitorLayout.monitors"
              :key="monitor.index"
              class="monitor-card"
              :class="{ 'monitor-card-primary': monitor.isPrimary }"
              :style="monitorStyle(monitor)"
            >
              <span class="monitor-index">{{ monitor.index + 1 }}</span>
              <span class="monitor-primary" v-if="monitor.isPrimary">Primary</span>
              <span class="monitor-resolution">{{ monitor.rect.width }} x {{ monitor.rect.height }}</span>
            </article>
          </div>

          <div v-else-if="!isMonitorLayoutLoading" class="monitor-empty">No monitors returned by the backend.</div>
          <div v-else class="monitor-empty">Scanning monitors...</div>
        </div>
      </div>
    </section>
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
    linear-gradient(180deg, rgba(0, 230, 246, 0.04), transparent 24%),
    radial-gradient(ellipse at center, rgba(0, 230, 246, 0.035), transparent 58%),
    radial-gradient(circle at 18% 14%, rgba(0, 230, 246, 0.13), transparent 29%),
    radial-gradient(circle at 86% 78%, rgba(0, 230, 246, 0.08), transparent 34%),
    linear-gradient(145deg, #000000 0%, #020b0d 46%, #000000 100%);
  color: var(--text-color);
  -webkit-app-region: drag;
  user-select: none;
}

.titlebar {
  height: 64px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 16px 0 18px;
  background: transparent;
  border-bottom: 0;
  box-shadow: none;
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
  min-height: 0;
  padding: 22px 28px 28px;
  box-sizing: border-box;
  background: transparent;
}

.monitor-panel {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 18px;
  border: 1px solid rgba(0, 230, 246, 0.26);
  border-radius: 18px;
  padding: 22px;
  box-sizing: border-box;
  background: rgba(0, 10, 12, 0.58);
  box-shadow: inset 0 0 24px rgba(0, 230, 246, 0.06), 0 0 34px rgba(0, 230, 246, 0.08);
  -webkit-app-region: no-drag;
}

.monitor-panel-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.monitor-panel-title {
  margin: 0;
  font-size: 20px;
  font-weight: 650;
  letter-spacing: 0.02em;
}

.refresh-button {
  min-width: 92px;
  border: 1px solid rgba(0, 230, 246, 0.38);
  border-radius: 10px;
  padding: 8px 12px;
  background: rgba(0, 230, 246, 0.08);
  color: var(--text-color);
  cursor: pointer;
}

.refresh-button:hover:not(:disabled) {
  background: rgba(0, 230, 246, 0.16);
}

.refresh-button:disabled {
  cursor: default;
  opacity: 0.58;
}

.monitor-error {
  margin: 0;
  border: 1px solid rgba(255, 105, 105, 0.42);
  border-radius: 10px;
  padding: 10px 12px;
  color: #ff9a9a;
  background: rgba(90, 0, 0, 0.24);
}

.monitor-map {
  position: relative;
  flex: 1;
  min-height: 260px;
  display: grid;
  place-items: center;
  box-sizing: border-box;
  border: 1px solid rgba(0, 230, 246, 0.18);
  border-radius: 14px;
  overflow: hidden;
  background:
    linear-gradient(rgba(0, 230, 246, 0.08) 1px, transparent 1px),
    linear-gradient(90deg, rgba(0, 230, 246, 0.08) 1px, transparent 1px),
    rgba(0, 0, 0, 0.22);
  background-size: 32px 32px;
}

.monitor-canvas {
  position: relative;
  border: 1px solid rgba(0, 230, 246, 0.12);
  box-sizing: border-box;
}

.monitor-card {
  position: absolute;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 7px;
  min-width: 72px;
  min-height: 52px;
  border: 2px solid rgba(0, 230, 246, 0.72);
  border-radius: 12px;
  box-sizing: border-box;
  background: linear-gradient(145deg, rgba(0, 230, 246, 0.13), rgba(0, 40, 45, 0.62));
  color: var(--text-color);
  box-shadow: inset 0 0 28px rgba(0, 230, 246, 0.08), 0 0 24px rgba(0, 230, 246, 0.16);
}

.monitor-card-primary {
  border-color: rgba(116, 255, 225, 0.9);
  box-shadow: inset 0 0 30px rgba(116, 255, 225, 0.12), 0 0 28px rgba(116, 255, 225, 0.18);
}

.monitor-index {
  font-size: 28px;
  font-weight: 750;
  line-height: 1;
}

.monitor-primary {
  position: absolute;
  top: 8px;
  right: 10px;
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.12em;
  color: rgba(116, 255, 225, 0.9);
}

.monitor-resolution {
  font-size: 12px;
  color: rgba(0, 230, 246, 0.72);
}

.monitor-empty {
  width: 100%;
  height: 100%;
  display: grid;
  place-items: center;
  color: rgba(0, 230, 246, 0.66);
}

@media (max-width: 720px) {
  .content-area {
    padding: 16px;
  }

  .monitor-panel {
    padding: 16px;
  }

  .monitor-panel-header {
    align-items: stretch;
    flex-direction: column;
  }
}
</style>
