# x-desk

![Platform](https://img.shields.io/badge/platform-Windows-blue?style=flat-square&logo=windows)
![License](https://img.shields.io/badge/license-MIT-green?style=flat-square)

**x-desk** is a lightweight desktop wallpaper tool for Windows, written in **Rust**.

It allows you to display **local videos** and **WebView content** directly on your desktop background.
By calling **native Windows APIs** for desktop integration, x-desk keeps the always-on main process focused on wallpaper orchestration.

---

## ✨ Features

- 🎥 **Video on Desktop** – Play videos behind desktop icons without affecting normal interaction.
- 🌐 **WebView Wallpaper** – Render URLs, local HTML files, inline HTML, or browser-playable media as wallpaper content.
- 🧩 **Multi-process Rendering** – Runs wallpaper renderers in separate `x-desk-player` or `x-desk-webview` processes and attaches their windows to the desktop.
- 🧊 **Non-intrusive** – Draws beneath desktop icons and pauses content when a dock is occluded.

---

## 🖥️ Platform Support

| OS | Status |
|---|---|
| Windows | ✅ Supported |
| macOS | ❌ Not supported |
| Linux | ❌ Not supported |

> x-desk currently targets **Windows only**, as it deeply integrates with native Windows desktop APIs.

---

## 🚀 Getting Started

### Prerequisites

- Windows 10 / 11
- [Rust (stable toolchain)](https://rustup.rs/)
- WebView2 Runtime for `webView` wallpaper content

### Configuration

x-desk reads `%APPDATA%\yinhaibo\x-desk\config.toml`. If the file does not exist, x-desk creates an empty default config.

Example:

```toml
[[monitors]]
kind = "video"
source = "C:\\videos\\one.mp4"

[[monitors]]
kind = "webView"
source = "https://example.com"
```

Supported monitor content kinds are `video` and `webView`. Empty `source` values disable wallpaper content for that monitor.

### Build & Run

- `Cargo run`
- `Cargo build --release`
