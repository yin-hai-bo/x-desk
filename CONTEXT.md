# Glossary

- **Dock**: A desktop-mounted container window used to host wallpaper content behind desktop icons.
- **Dock Occlusion**: The state in which regular top-level windows cover the work area of the monitor associated with a Dock.
- **Desktop Rebuild Trigger**: A Windows event that indicates desktop wallpaper host windows may need refresh or full reconstruction.
- **Occluded Dock**: A Dock whose visible area is at or below the configured visibility threshold.
- **Main UI Process**: A separate Tauri process launched from the tray to present the main UI.
- **Settings Process**: A short-lived process launched from the hidden main window that opens the configuration file with the system file association.
- **Wallpaper Content Process**: A process that creates an opaque render window for one wallpaper content type. The main process attaches that window to the desktop.
- **Wallpaper Orchestrator**: The main x-desk process role that owns desktop discovery, WorkerW attachment, monitor layout, occlusion, desktop rebuild handling, and content process lifetime.
- **Video Host**: A render window owned by the `x-desk-player` content process and used as the Media Foundation video render target.
- **Video Wallpaper**: A local video rendered behind desktop icons as desktop background content.
- **Wallpaper Reset**: Recreating desktop host discovery and Dock windows from current config after shell or desktop handles become invalid.

# Architecture Constraints

- The Wallpaper Orchestrator owns all Progman, WorkerW, Raised Desktop, z-order, Explorer restart, monitor, occlusion, and desktop attachment logic.
- Wallpaper Content Processes own rendering only and must not attach themselves to WorkerW or assume they are being used as desktop wallpaper.
- The Wallpaper Orchestrator treats content windows as opaque HWNDs, verifies each HWND belongs to the child process it started, then attaches and resizes that HWND through native Win32 APIs.
- Wallpaper Content Processes communicate readiness and control over named pipes. The initial process message is `WindowReady { hwnd }`; commands are `Pause`, `Resume`, and `Stop`.
- `x-desk-player` renders local video through Media Foundation. `x-desk-webview` renders WebView2 content from URLs, file URLs, local paths, data URLs, or inline HTML.
- The current config uses `[[monitors]]` entries with `kind` and `source`. Supported `kind` values are `video` and `webView`; an empty or whitespace-only `source` disables that monitor.
- The Settings Process opens the config file through the shell and exits. The Main UI Process is separate from the Wallpaper Orchestrator and is launched from the tray.
