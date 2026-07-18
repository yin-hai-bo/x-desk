# x-desk

![Platform](https://img.shields.io/badge/platform-Windows-blue?style=flat-square&logo=windows)
![License](https://img.shields.io/badge/license-MIT-green?style=flat-square)

**x-desk** is a lightweight desktop enhancement tool for Windows, written in **Rust**.

It allows you to display **videos** and **custom text** directly on your desktop background.  
By calling **native Windows APIs**, x-desk achieves smooth rendering with **minimal system resource usage**, making it ideal for always-on desktop customization.

---

## ✨ Features

- 🎥 **Video on Desktop** – Play videos behind desktop icons without affecting normal interaction.
- 📝 **Custom Text Overlay** – Render static or dynamic text on the desktop.
- ⚡ **Ultra-low Resource Usage** – Built in Rust with direct Win32 API calls; very low CPU and memory footprint.
- 🧊 **Non-intrusive** – Draws beneath desktop icons; fully transparent to daily workflows.

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

### Build & Run

- `Cargo run`
- `Cargo build --release`
