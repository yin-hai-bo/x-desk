# Multi-Process Wallpaper Architecture

## Goal

x-desk will evolve from an in-process video wallpaper application into a desktop wallpaper orchestrator. The main process keeps ownership of Windows desktop integration, while separate short-lived or content-specific processes handle configuration UI and wallpaper rendering.

The target design has three process roles:

- **Main Process**: Long-lived coordinator that owns desktop discovery, WorkerW attachment, monitor layout, configuration reloads, occlusion tracking, tray integration, and child process lifecycle.
- **Settings UI Process**: Short-lived UI process that can use WebView for rich configuration screens. It exits completely after the user finishes configuration.
- **Wallpaper Content Process**: Rendering process that creates a window and renders one kind of wallpaper content, such as video, image, WebView content, or a game. The main process attaches that window to the desktop and does not inspect what it renders.

## Current Shape

The current implementation is video-specific inside the main process:

- `WallpaperManager` discovers monitors, creates Docks, attaches them to the desktop, and applies configuration.
- `Dock` is a desktop-mounted host window and directly owns `VideoHost`.
- `VideoHost` creates a child window and uses Media Foundation through `VideoPlayer`.
- `Desktop` owns Progman and WorkerW discovery, Raised Desktop handling, and z-order repair.

This means the main process currently knows both desktop orchestration and video playback details.

## Target Shape

The target implementation separates those responsibilities:

```text
x-desk main process
  reads config
  watches config changes
  discovers monitors
  discovers Progman / WorkerW
  starts wallpaper content processes
  receives content HWNDs
  attaches content HWNDs to desktop parents
  resizes content windows
  sends pause / resume / stop commands

x-desk-settings process
  shows WebView settings UI
  writes config
  optionally notifies main process to reload
  exits when settings UI closes

x-desk-player process
  creates a renderable top-level window
  initializes Media Foundation
  plays video inside its own window
  reports HWND to main process
  handles playback commands
  exits when stopped or when parent process is gone
```

## Design Rules

- The main process owns all WorkerW, Progman, Raised Desktop, z-order, Explorer restart, monitor, and occlusion logic.
- Wallpaper content processes own rendering only.
- Settings UI owns configuration interaction only.
- A content process must not attach itself to WorkerW.
- A content process must not need to know whether it is being used as desktop wallpaper, preview content, or another embedded surface.
- The main process treats content windows as opaque HWNDs.
- The main process must verify that a received HWND belongs to the child process it started.
- The main process owns content process lifetime and cleans up content processes on exit.

## Module Seams

### Desktop Attachment Seam

The main process needs a deep module that attaches any external content window to a desktop parent.

Proposed interface:

```rust
fn attach_content_window(hwnd: HWND, parent: HWND, rect: RECT) -> Result<()>;
```

Responsibilities hidden behind this interface:

- Convert a top-level content window into a child window.
- Remove caption, resizing, system menu, minimize, maximize, and popup styles.
- Add child, clip-children, and clip-siblings styles.
- Hide the content window from taskbar and Alt-Tab where possible.
- Call `SetParent` with the selected desktop parent.
- Move and resize the window into monitor coordinates relative to the desktop parent.
- Preserve Raised Desktop z-order rules.

This seam should grow out of the current `WallpaperManager::set_to_child_window` and `WallpaperManager::set_wallpaper` logic.

### Content Process Seam

The main process needs a module that starts and controls wallpaper content processes.

Proposed interface:

```rust
struct ContentProcessSpec {
    monitor_index: usize,
    kind: WallpaperKind,
    source: String,
}

struct ContentProcessHandle {
    process_id: u32,
    hwnd: HWND,
}

fn start_content_process(spec: &ContentProcessSpec) -> Result<ContentProcessHandle>;
fn send_content_command(handle: &ContentProcessHandle, command: ContentCommand) -> Result<()>;
fn stop_content_process(handle: ContentProcessHandle) -> Result<()>;
```

Initial commands:

- `Pause`
- `Resume`
- `Stop`

Potential later commands:

- `SetSource`
- `SetMuted`
- `SetVolume`
- `SetPlaybackRate`
- `Reload`

Resize should remain a main-process `SetWindowPos` operation unless a future renderer needs explicit logical resize events beyond normal `WM_SIZE`.

### Settings UI Seam

The settings UI should be isolated from the main process runtime.

Proposed first interface:

```text
x-desk-settings.exe --config <path>
```

The settings process writes the configuration file directly. The main process reloads through the existing configuration watcher.

An optional later notification channel can be added if file watching is not responsive enough, but it should not be required for the first version.

## IPC Strategy

Use the simplest IPC that gives reliable HWND delivery and command routing.

Recommended first implementation:

- Main process creates a named pipe for each content process.
- Main process starts the content process with the pipe name and content arguments.
- Content process creates its top-level render window.
- Content process sends `WindowReady { hwnd }` through the pipe.
- Main process verifies that `hwnd` belongs to the expected child process.
- Main process attaches the HWND to the desktop parent.
- Main process uses the same pipe for `Pause`, `Resume`, and `Stop` commands.

Avoid stdout as the primary channel for GUI processes because Windows subsystem binaries may not have a useful console stream.

## Window Contract

A wallpaper content process must create a window with these expectations:

- It starts as a top-level window.
- It must tolerate the main process changing its style and parent.
- It must repaint correctly after `SetParent`.
- It must handle `WM_SIZE` and resize its renderer to the full client area.
- It must not assume it can activate, focus, or appear in the taskbar.
- It must exit after receiving `Stop` or after detecting that the main process is gone.

The main process must handle these expectations:

- It must attach the content window only after validating process ownership.
- It must resize the content window when monitor geometry changes.
- It must reattach content windows after desktop reconstruction when possible.
- It must restart content processes when their windows are destroyed unexpectedly.

## Configuration Model

The current config is video-specific. The first implementation can keep compatibility while introducing a content-oriented shape.

Current shape:

```toml
[[monitors]]
video_url = "C:\\videos\\one.mp4"
```

Target shape:

```toml
[[monitors]]
kind = "video"
source = "C:\\videos\\one.mp4"
```

Future plugin-oriented shape, only if multiple external providers become real:

```toml
[[monitors]]
provider = "x-desk-player.exe"
args = ["--source", "C:\\videos\\one.mp4"]
```

Do not add the plugin-oriented shape until at least two provider implementations exist.

## Implementation Plan

### [✓] Phase 1: Extract Desktop Attachment

- Introduce a desktop attachment helper in the main process.
- Move child-window style conversion and `SetParent` logic out of video-specific flow.
- Keep `Dock` and `VideoHost` in-process for this phase.
- Verify current video wallpaper behavior is unchanged.

### [✓] Phase 2: Introduce Content Spec

- Add a content-oriented config model internally, even if the file still uses `video_url`.
- Convert monitor config into `ContentProcessSpec` or an equivalent internal model.
- Rename video-specific orchestration concepts where they now mean generic content.
- Keep Media Foundation playback in-process for this phase.

### [✓] Phase 3: Create Video Content Process

- Add a separate video player binary.
- Move `VideoHost` and `VideoPlayer` responsibilities into that binary.
- Make the video process create a top-level render window.
- Add named-pipe `WindowReady { hwnd }` delivery.
- Make the main process start the video process and attach its HWND.
- Preserve existing pause/resume behavior through IPC commands.

### [✓] Phase 4: Own Process Lifetime Robustly

- Track process handles, process IDs, HWNDs, monitor indices, and current content specs.
- Stop content processes when config removes a monitor entry.
- Restart content processes when a content process exits unexpectedly.
- Reattach or restart content after Explorer restarts and WorkerW is rebuilt.
- Clean up all content processes during main process shutdown.

### [✓] Phase 5: Extract Settings UI Process

- Add `x-desk-settings` as a separate binary.
- Use WebView only in the settings process.
- Let the settings process write config and exit on close.
- Launch settings from tray or another main-process command.
- Keep the main process independent of WebView dependencies if practical.

### [✓] Phase 6: Generalize Content Types

- Change config from `video_url` to `kind` and `source` when compatibility requirements are clear.
- Add a WebView2 content process that can render inline HTML, HTML pages, or browser-playable video sources.
- Only after two real renderers exist, consider provider-based config.

## Verification Plan

Each phase should be verified against these behaviors:

- Video appears behind desktop icons on Windows 10 and Windows 11.
- Multi-monitor placement remains correct.
- Explorer restart recovers wallpaper content.
- TaskbarCreated handling does not duplicate content processes.
- Occlusion pause/resume still works.
- Removing a monitor config stops the correct content process.
- Main process exit does not leave orphan wallpaper windows or player processes.
- Settings UI can open, save config, close, and leave no resident WebView process.

## Risks

- Cross-process `SetParent` can expose DPI awareness mismatches.
- Window style changes after renderer initialization can break some renderers.
- HWND delivery can be spoofed if not verified against the child process ID.
- Explorer restarts can invalidate desktop parents while content processes remain alive.
- Renderer process crashes must not leave stale state in the main process.
- WebView dependencies should not leak into the always-on main process if low resource usage remains a core product goal.

## Non-Goals

- Do not build a generic plugin system in the first pass.
- Do not let content processes own WorkerW discovery.
- Do not move tray, config watching, occlusion, or Explorer recovery into the settings UI process.
- Do not introduce third-party GUI libraries for Windows UI logic in the main process.
- Do not support non-Windows desktop integration in this architecture.
