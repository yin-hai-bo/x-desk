# Temporary Main UI Monitor Configuration Plan

This document is temporary. Delete it after the full Main UI monitor configuration feature is complete.

## Review Steps

1. [x] Extend the config model with optional preview metadata.
   - Add `preview_source` to each monitor config entry.
   - Keep existing `kind/source` behavior unchanged for wallpaper rendering.
   - Add read-facing helpers needed by later UI work.
   - Add focused Rust tests for backward-compatible config loading.

2. [x] Add config mutation helpers.
   - Add APIs to set a monitor to WebView content with `preview_source`.
   - Add APIs to clear a monitor while preserving its index entry.
   - Make writes pad missing monitor entries only when setting/clearing a specific index.
   - Preserve entries beyond the current display count.

3. [x] Add config save support with readable inline HTML.
   - Add a config crate `save_to_file` API.
   - Ensure inline HTML `source` is written in a readable multiline TOML form.
   - Add tests for multiline HTML, path strings, empty source, and optional preview metadata.

4. [x] Add video source template generation in the Main UI backend.
   - Add the fixed video wrapper template with `{{VIDEO_FILE_URL}}` replacement.
   - Normalize selected video paths to `file:///...` URLs.
   - Validate supported extensions: `.html`, `.htm`, `.mp4`, `.webm`, `.mov`, `.m4v`.
   - Keep HTML files as direct WebView file sources.

5. [ ] Add monitor layout ViewModel commands in the Main UI backend.
   - Enumerate Win32 monitors and return real virtual desktop rectangles.
   - Merge monitor entries with config by index.
   - Return preview kind and backend-generated preview URL when possible.
   - Reload config from disk for the refresh command.

6. [ ] Add save and clear commands in the Main UI backend.
   - Save selected local files as `webView` monitor config.
   - Clear a monitor entry by writing an empty source and removing preview metadata.
   - Return the full refreshed ViewModel after save/clear.
   - Add a TODO beside successful save/clear for future Wallpaper Orchestrator refresh notification.

7. [ ] Add official Tauri dialog support.
   - Add `@tauri-apps/plugin-dialog` to the Main UI package.
   - Add `tauri-plugin-dialog` to the Main UI Tauri app.
   - Register the plugin and add the required capability permission.

8. [ ] Build the Vue monitor layout UI.
   - Render monitors as scaled rectangles using real coordinates.
   - Preserve the existing neon HUD visual language.
   - Show empty monitor click prompts.
   - Preview videos with muted looping autoplay and `object-fit: contain`.
   - Preview HTML files in a sandboxed static iframe.
   - Add a per-monitor clear button.
   - Add a content-area refresh layout button.
   - Add a non-blocking error banner.

9. [ ] Increase Main UI default window size.
   - Change the default window size to `1280x860`.
   - Keep the window centered, undecorated, non-maximizable, and non-resizable.

10. [ ] Verification pass.
   - Run `cargo fmt` after Rust changes.
   - Run config crate tests.
   - Run Main UI typecheck/build.
   - Manually inspect generated `config.toml` examples for readability.

11. [ ] Cleanup.
   - Delete this temporary plan document after the whole feature is done.

## Agreed Design Decisions

- Monitor settings bind by current monitor index.
- All Main UI-created sources use `kind = "webView"`.
- Video files are stored as inline HTML generated from the agreed template.
- HTML files are stored as direct file sources.
- Main UI previews original selected files through `preview_source`.
- Existing `kind = "video"` entries are ignored by the Main UI.
- Existing `webView` entries without `preview_source` are previewed only if the source is recognizably an HTML file path or file URL.
- Immediate Wallpaper Orchestrator refresh is a future TODO, not part of this implementation pass.
