
# STOV (State of the Art Observation Tool)

![Rust](https://img.shields.io/badge/built_with-Rust-dca282?style=flat-square)
![Platform](https://img.shields.io/badge/platform-Termux%20%7C%20Linux-blue?style=flat-square)
![License](https://img.shields.io/badge/license-MIT-green?style=flat-square)

**STOV** is a high-performance, asynchronous Instagram Story Archiver engineered in Rust. It utilizes advanced browser automation via the Chrome DevTools Protocol (CDP) to perform "human-like" scraping with zero-detection capability.

Unlike traditional scrapers that rely on reverse-engineered APIs (which get banned quickly), STOV operates a real Chromium instance, utilizing **DOM-First Context Awareness** and **Browser-Side Fetching** to secure media with cryptographic integrity.

---

## 🚀 Key Features

### 🧠 **Intelligent Automation**
*   **Stateful Batch Processing:** Intelligently navigates user profiles, maintaining session state to download stories sequentially without skipping.
*   **Hybrid Detection Engine:** Uses a dual-layer approach:
    1.  **Visual Geometry Check:** Calculates element position and aspect ratio to distinguish actual stories from background feed posts.
    2.  **Network Sniffing:** Intercepts internal browser traffic to identify high-quality media streams.
*   **Freeze & Fetch Protocol:** Automatically pauses video and image timers (`v.pause()`) to ensure downloads complete regardless of network speed.

### 🛡️ **Stealth & Security**
*   **Browser-Side Fetching:** Media is downloaded *inside* the authenticated browser context, bypassing signature tokens and "403 Forbidden" errors common with external downloaders.
*   **Session Management:** Successfully logins are serialized into JSON profiles. Subsequent runs inject cookies directly, bypassing login screens and reducing suspicious activity flags.
*   **Randomized Heuristics:** Human-like delays and mouse movements prevent bot detection.

### 📱 **Optimized for Termux (Android)**
*   **Smart Display Detection:** Automatically detects X11 environments. Runs in **Visual Mode** if a display is found, or **Headless Mode** (background) if not, preventing crash loops.
*   **Resource Efficient:** Compiles to a lightweight binary with minimal memory footprint compared to Node.js or Python alternatives.

---

## 🛠️ Installation (Termux)

STOV is optimized for the Termux environment on Android.

### 1. System Requirements
Update your repositories and install the necessary dependencies:

```bash
pkg update && pkg upgrade -y
pkg install rust binutils x11-repo tur-repo chromium -y
```

### 2. Clone & Build
```bash
git clone https://github.com/hakinexus/stov.git
cd stov
cargo build --release
```

---

## ⚡ Usage

Run the tool using Cargo. STOV handles the rest.

```bash
cargo run
```

### The Workflow
1.  **Initialization:** STOV initializes the Chromium engine and cleans the terminal UI.
2.  **Authentication:**
    *   **New Session:** Enter credentials manually. Session ID is auto-saved.
    *   **Saved Session:** Select a profile from the menu to login instantly without credentials.
3.  **Targeting:** Enter usernames separated by commas (e.g., `nike, adidas, natgeo`).
4.  **Extraction:** The bot will visit each profile, iterate through all available stories, download them, and move to the next target.

### 📺 Visual Mode (Optional)
To watch the bot work in real-time on Android, use **Termux-X11**:

1.  Open the **Termux-X11** app.
2.  In Termux terminal, run:
    ```bash
    export DISPLAY=:0
    cargo run
    ```

---

## 📂 Output Structure

STOV automatically manages its file system. No manual setup required.

```text
stov/
├── downloads/          # High-res .mp4 and .jpg files (Format: username_timestamp.ext)
├── profiles/           # Serialized session JSON files for auto-login
├── images/             # (Debug) Snapshots of errors or verification proofs
│   ├── login_proofs/   
│   └── story_errors/
└── src/                # Source code
```

---

## 🔧 Troubleshooting

| Issue | Solution |
| :--- | :--- |
| **Browser Hangs on Start** | Ensure you are not using `DISPLAY=:0` without the X11 app open. Kill stuck processes with `pkill chromium`. |
| **Login "Please wait..."** | Instagram soft-block. STOV has an auto-retry mechanism. Allow it to wait and retry 3 times. |
| **Compilation Errors** | Ensure `rust` and `binutils` are up to date. Run `cargo clean` and try again. |

---

## ⚠️ Disclaimer

This tool is for **educational and archival purposes only**. The user assumes all responsibility for complying with Instagram's Terms of Service and applicable privacy laws. The developers of STOV are not liable for account suspensions or misuse.

---

**Built with 🦀 Rust and Expert Engineering.**
```
