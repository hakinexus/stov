
<div align="center">

# STOV

![Rust](https://img.shields.io/badge/Language-Rust-000000?style=for-the-badge&logo=rust&logoColor=white)
![Platform](https://img.shields.io/badge/Platform-Termux_|_Linux-000000?style=for-the-badge&logo=linux&logoColor=white)
![License](https://img.shields.io/badge/License-MIT-000000?style=for-the-badge)

</div>

**STOV** is a high-performance, asynchronous Instagram Story Archiver engineered in Rust. It utilizes advanced browser automation via the Chrome DevTools Protocol (CDP) to execute human-mimetic scraping operations.

Unlike traditional scrapers reliant on unstable APIs, STOV operates a fully authenticated Chromium instance, leveraging **DOM-First Context Awareness** and **Browser-Side Fetching** to secure media with cryptographic integrity while bypassing standard anti-bot detection mechanisms.

---

## System Architecture

### Intelligent Automation
STOV employs a **Stateful Batch Processing** engine that intelligently navigates user profiles. It maintains session state to download stories sequentially, ensuring zero-skip data integrity.

### Hybrid Detection Engine
The tool utilizes a dual-layer verification system:
1.  **Visual Geometry Check:** Algorithms calculate element position and aspect ratio to distinguish active stories from background feed elements.
2.  **Network Traffic Analysis:** An internal wiretap intercepts browser traffic to identify high-quality media streams directly from the source.

### Freeze & Fetch Protocol
To counter auto-advancing timers, STOV implements a logic lock that programmatically pauses video elements and UI timers immediately upon detection, ensuring downloads complete regardless of network latency.

### Stealth & Security
*   **Browser-Side Fetching:** Media retrieval occurs inside the authenticated browser context, effectively bypassing signature token validation and HTTP 403 Forbidden errors.
*   **Session Persistence:** Successful authentication sessions are serialized into JSON profiles. Subsequent executions inject session cookies directly, bypassing login forms and reducing heuristic flagging.

---

## Installation

STOV is optimized for the Termux environment on Android.

**1. Dependency Configuration**
```bash
pkg update && pkg upgrade -y
pkg install rust binutils x11-repo tur-repo chromium -y
```

**2. Compilation**
```bash
git clone https://github.com/hakinexus/stov.git
cd stov
cargo build --release
```

---

## Execution & Workflow

### Standard Operation
The tool manages the runtime environment automatically.

```bash
cargo run
```

**Operational Workflow:**
1.  **Initialization:** The Chromium engine initializes, and the CLI interface is sanitized.
2.  **Credential Evaluation:** The system checks for existing session profiles. Users may select a cached session or authenticate manually.
3.  **Context Injection:** Session cookies are injected into the browser context.
4.  **Recursive Extraction:** The bot iterates through the target list, engaging the Freeze & Fetch protocol for every story slide before navigating to the next target.

### Visual Mode (X11)
To observe the automation process in real-time via an external display server:

1.  Open the **Termux-X11** application.
2.  Execute the following in the Termux terminal:

```bash
export DISPLAY=:0
termux-x11 :0 &
cargo run
```

---

## Directory Hierarchy

STOV manages its own file system structure upon initialization.

```text
stov/
├── downloads/
├── profiles/
├── images/
│   ├── login_proofs/
│   └── story_errors/
└── src/
```

---

## Troubleshooting

| Issue | Solution |
| :--- | :--- |
| **Browser Hangs on Start** | Ensure `DISPLAY` is not exported if X11 is closed. Terminate processes with `pkill chromium`. |
| **Login Failure** | The software is currently in Beta. If authentication hangs or fails repeatedly, terminate the process and restart the tool. |
| **Compilation Errors** | Verify `rust` and `binutils` packages are up to date. Execute `cargo clean` and rebuild. |
