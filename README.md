# x-desk

![Platform](https://img.shields.io/badge/platform-Windows-blue?style=flat-square&logo=windows)
![License](https://img.shields.io/badge/license-MIT-green?style=flat-square)

**x-desk** is a lightweight desktop wallpaper tool for Windows, written in **Rust**.

It allows you to display **local videos** directly on your desktop background.
By calling **native Windows APIs** for desktop integration, x-desk keeps the always-on main process focused on wallpaper orchestration.

---

## ✨ Features

- 🎥 **Video on Desktop** – Play videos behind desktop icons without affecting normal interaction.
- 🧩 **Multi-process Rendering** – Runs wallpaper renderers in a separate `x-desk-webview` process and attaches its window to the desktop.
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
- WebView2 Runtime

### Configuration

x-desk reads `%APPDATA%\yinhaibo\x-desk\config.toml`. If the file does not exist, x-desk creates an empty default config.

Example:

```toml
[[monitors]]
kind = "video"
source = "C:\\videos\\one.mp4"
```

Supported monitor content kind is `video`. Empty `source` values disable wallpaper content for that monitor.

### Build & Run

- `Cargo run`
- `Cargo build --release`
