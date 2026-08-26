# STOV

[![Version](https://img.shields.io/badge/version-2.15.1-111827?style=flat-square)](Cargo.toml)
[![Rust](https://img.shields.io/badge/Rust-2021-111827?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![Node](https://img.shields.io/badge/Node-%3E%3D20-111827?style=flat-square&logo=node.js)](https://nodejs.org/)
[![Platform](https://img.shields.io/badge/platform-Termux%20%7C%20Linux-111827?style=flat-square&logo=linux)](https://termux.com/)
[![License](https://img.shields.io/badge/license-MIT-111827?style=flat-square)](LICENSE)

**STOV** is a local Instagram Story archive tool built around an authenticated Chromium session. It observes the story viewer, captures media in the browser context, validates output with FFmpeg/FFprobe, and serves completed artifacts through a local gallery.

> Built for content the operator is authorized to access. The story viewer is an undocumented interface and can change; STOV is designed to fail visibly, preserve diagnostics, and never publish unverified media as complete.

## Highlights

| Area | Implementation |
|---|---|
| Story navigation | Viewer-state fingerprints, semantic controls, verified transitions, and keyboard fallback |
| Media capture | DOM media first, authenticated browser fetch second, CDP response capture for blob-backed media |
| File integrity | Atomic writes, stream validation, validated audio/video muxing, and sidecar manifests |
| Gallery | Express API, safe deletion, live SSE updates, resilient file watching, responsive lightbox |
| Operations | Pinned toolchain, lockfiles, CI checks, screenshots/HTML diagnostics, and explicit video-only status |

## Quick start

### 1. Install prerequisites

**Termux**

```bash
pkg update && pkg upgrade -y
pkg install rust binutils ffmpeg nodejs -y
pkg install x11-repo tur-repo
pkg install chromium
command -v chromium
chromium --version
```

**Debian/Ubuntu**

```bash
sudo apt update
sudo apt install -y build-essential chromium ffmpeg pkg-config libssl-dev nodejs npm
```

### 2. Build and run STOV

```bash
git clone https://github.com/hakinexus/stov.git
cd stov
cargo run --release
```

The CLI accepts an account, password, and comma-separated target usernames. On Termux, STOV detects Chromium through `$PREFIX/bin`, `$TERMUX_PREFIX/bin`, and the active `$PATH`; it no longer depends on the external `which` command. If Chromium is installed in a custom location, set `STOV_CHROMIUM_PATH` to the executable path before running. A successful login can save a local session profile for later use. Session files are private credentials; never commit `profiles/`.

### 3. Run the gallery

```bash
cd stov-gallery
npm ci
npm start
```

Open [http://127.0.0.1:3000](http://127.0.0.1:3000). The gallery reads validated files from `../downloads` and updates when new artifacts are published.

## Configuration

| Variable | Default | Purpose |
|---|---:|---|
| `STOV_CHROMIUM_PATH` | Termux Chromium path | Override the Chromium executable |
| `STOV_WINDOW_WIDTH` | `1280` | Browser viewport width |
| `STOV_WINDOW_HEIGHT` | `720` | Browser viewport height |
| `STOV_USER_AGENT` | Chromium default | Optional user-agent override |
| `STOV_ENABLE_GPU` | disabled | Enable GPU where supported |
| `STOV_ALLOW_NO_SANDBOX` | disabled | Explicitly allow no-sandbox mode when required |
| `HOST` | `127.0.0.1` | Gallery bind address |
| `PORT` | `3000` | Gallery port |

## Output

```text
downloads/
├── <username>_<timestamp>_<story-key>.mp4
├── <username>_<timestamp>_<story-key>.mp4.json
└── ...

profiles/              # local session credentials; keep private
images/login_proofs/   # login evidence when available
images/story_errors/   # failure screenshots and HTML snapshots
```

Each media file has a JSON manifest when created by the repaired pipeline. `complete` means the published file passed validation. `video-only` means the video is valid but no validated audio track was available; it is not falsely labeled as a complete mux.

## Development checks

The repository uses a pinned Rust toolchain and committed lockfiles.

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cd stov-gallery
npm ci
npm test
```

See [`readers.md`](readers.md) for the short maintainer guide and data flow.

## Troubleshooting

| Symptom | Check first |
|---|---|
| Chromium will not start | Run `chromium --version`; set `STOV_CHROMIUM_PATH`; use no-sandbox only when required by the environment. |
| Login does not complete | Re-authenticate, use a fresh session, and inspect `images/story_errors/`. |
| Stories do not advance | Inspect the captured evidence and terminal logs; the controller now requires an observed media/state transition instead of a URL change. |
| Video has no sound | Confirm `ffmpeg` and `ffprobe` are installed. A validated `video-only` result is reported honestly in the manifest. |
| Gallery is empty | Start it from `stov-gallery/`, run `npm ci`, and check `/api/health`. |
| Gallery scrolling is locked | Close the lightbox with Escape or its close button; the page restores the previous scroll position through one unlock path. |

## Privacy and scope

Keep `profiles/`, `downloads/`, `images/`, and logs private. Use a dedicated account where appropriate and comply with applicable platform terms, permissions, and privacy obligations.
