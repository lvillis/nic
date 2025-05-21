<div align="right">Table of Contents ↗️</div>

<h1 align="center"><code>nic</code></h1>

<p align="center">
  🛠️ nic — a tiny cross-platform Network Config UI
</p>

<div align="center">

[![Crates.io](https://img.shields.io/crates/v/nic.svg)](https://crates.io/crates/nic)
[![Repo Size](https://img.shields.io/github/repo-size/lvillis/nic?color=328657)](https://github.com/lvillis/nic)
[![CI](https://github.com/lvillis/nic/actions/workflows/ci.yaml/badge.svg)](https://github.com/lvillis/nic/actions)
[![Docker Pulls](https://img.shields.io/docker/pulls/lvillis/nic)](https://hub.docker.com/r/lvillis/nic)
[![Image Size](https://img.shields.io/docker/image-size/lvillis/nic/latest?style=flat-square)](https://hub.docker.com/r/lvillis/nic)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

---

## ✨ Features

|                          | Windows | Linux / macOS |
|--------------------------|:-------:|:-------------:|
| Interactive TUI          |    ✅    |       ✅       |
| System-tray integration  |    ✅    |       —       |
| View NIC list            |    ✅    |       ✅       |
| Edit IPv4 / DNS          |    ✅    |      —¹       |
| Enable / Disable adapter |    ✅    |       —       |
| One-key refresh          |    ✅    |       ✅       |
| Fully keyboard-driven    |    ✅    |       ✅       |

## 🚀 Usage

### Hotkeys

| Key          | Action                      |
|--------------|-----------------------------|
| ↑ / ↓        | Navigate list / form fields |
| Enter / →    | Switch to form view         |
| **f**        | Filter NIC list             |
| **r**        | Refresh adapters            |
| **s / F10**  | Save changes (Windows only) |
| **q**        | Hide window → stays in tray |
| **F1 / ?**   | Help                        |
| **Ctrl + C** | Quit immediately            |

## 📜 License

Licensed under the **MIT** License — see [`LICENSE`](LICENSE).

---

<p align="center"><sub>Made with Rust ❤️ </sub></p>