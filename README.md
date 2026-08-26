# STOV

**STOV** is a local Instagram Story archive tool written in Rust. It drives an authenticated Chromium session, observes the story viewer, captures media in the browser context, validates artifacts with FFmpeg/FFprobe, and publishes completed files for the local gallery.

STOV is designed for accounts and content that the operator is authorized to access. Instagram’s story viewer is an undocumented, changing interface; no browser automation can promise permanent compatibility without maintenance. The project is therefore built to fail visibly, preserve diagnostics, and avoid publishing unverified files.

## What changed in v3

The v3 recovery replaces URL-only story identity with observed viewer state, attempts semantic next-story controls before keyboard fallback, and uses the active DOM media as the first source of truth. It also registers a Chrome DevTools Protocol response listener for video, audio, and image bodies, preserves the authenticated browser context for fallback fetches, validates containers and streams with FFprobe, writes artifacts atomically, and stores a manifest beside each completed file.

A video with no validated separate audio track is published as `video-only` rather than being reported as a complete mux. The gallery displays that status. A completed artifact is renamed into `downloads/` only after validation, so the gallery does not expose `.part` files while they are being written.

## Requirements

The following tools are required:

| Component | Requirement |
|---|---|
| Rust | 1.98.0, pinned by `rust-toolchain.toml` |
| Chromium | A Chromium binary; set `STOV_CHROMIUM_PATH` when it is not at the Termux default path |
| FFmpeg | Required for media validation and audio muxing |
| FFprobe | Required for stream validation |
| Node.js | 20 or newer for the gallery |

On Termux, install the system dependencies with:

```bash
pkg update && pkg upgrade -y
pkg install rust binutils chromium ffmpeg nodejs -y
```

On Debian/Ubuntu, install the equivalent packages with:

```bash
sudo apt update
sudo apt install -y build-essential chromium ffmpeg pkg-config libssl-dev nodejs npm
```

## Build and run

```bash
git clone https://github.com/hakinexus/stov.git
cd stov
cargo build --release
cargo run --release
```

The CLI asks for an Instagram account, password, and comma-separated target usernames. Saved profiles are stored locally under `profiles/`; protect that directory because it contains session credentials. The scraper writes validated media and JSON manifests into `downloads/`.

To run the gallery:

```bash
cd stov/stov-gallery
npm ci
npm start
```

Open `http://127.0.0.1:3000`. The gallery reads the parent `downloads/` directory and updates through a resilient Server-Sent Events stream. It should be started from `stov-gallery/`, or `PORT` and `HOST` can be supplied explicitly:

```bash
HOST=127.0.0.1 PORT=3000 npm start
```

## Runtime configuration

The browser defaults preserve the last known-good 1280×720 viewport. Runtime options can be overridden without editing source files.

| Variable | Purpose |
|---|---|
| `STOV_CHROMIUM_PATH` | Absolute Chromium executable path |
| `STOV_WINDOW_WIDTH` | Browser width, minimum 320 |
| `STOV_WINDOW_HEIGHT` | Browser height, minimum 320 |
| `STOV_USER_AGENT` | Optional browser user-agent override |
| `STOV_ENABLE_GPU=1` | Enable GPU when the deployment supports it |
| `STOV_ALLOW_NO_SANDBOX=1` | Explicitly allow no-sandbox mode when required by the environment |
| `HOST` | Gallery bind address, default `127.0.0.1` |
| `PORT` | Gallery port, default `3000` |

## Project layout

```text
stov/
├── src/
│   ├── browser.rs       # Chromium launch and runtime options
│   ├── config.rs        # Semantic selectors and filesystem defaults
│   ├── instagram.rs     # Login, viewer state machine, CDP capture
│   ├── main.rs          # CLI workflow
│   └── utils.rs         # Atomic storage, manifests, FFmpeg/FFprobe
├── stov-gallery/
│   ├── package.json     # Reproducible Node dependencies and scripts
│   ├── package-lock.json
│   ├── server.js        # Media API, SSE, safe deletion, watcher
│   └── public/          # Gallery HTML, CSS, and JavaScript
├── rust-toolchain.toml
└── Cargo.lock
```

## Artifact states

Every media file should have a sidecar manifest named `<filename>.json`. The `status` field is either `complete` or `video-only`, and `has_audio` records whether a validated audio stream exists in the published file. A `video-only` artifact is valid video but did not receive a validated separate audio stream; this distinction is intentional and visible in the gallery.

## Quality checks

Run the project checks before changing the scraper:

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cd stov-gallery
npm ci
npm test
```

The browser-facing workflow should also be tested with authorized content using a small target set first. When a story fails, inspect `images/story_errors/` and the corresponding terminal error rather than increasing arbitrary sleep durations.

## Troubleshooting

| Symptom | First checks |
|---|---|
| Chromium does not start | Verify `STOV_CHROMIUM_PATH`, run `chromium --version`, and only use `STOV_ALLOW_NO_SANDBOX=1` when the environment requires it. |
| Login is rejected or challenged | Re-authenticate, use a fresh session, and inspect the saved screenshot under `images/story_errors/`. Do not commit `profiles/`. |
| Story viewer does not advance | Check the captured screenshot and viewer-state logs. The controller now verifies media/fingerprint transition instead of assuming the URL must change. |
| A file is marked `video-only` | Check FFprobe availability and the story’s captured network responses. The file is not falsely labeled as a successful audio mux. |
| Gallery is empty | Start it from `stov-gallery/`, run `npm ci`, verify `downloads/` exists, and call `http://127.0.0.1:3000/api/health`. |
| Gallery scrolling is locked | Close the lightbox with Escape or the close button. v3 restores the original body styles and scroll position through one idempotent unlock path. |

## Security notes

STOV handles session credentials and downloaded personal media. Keep `profiles/`, `downloads/`, `images/`, and any logs private. The browser’s certificate-error bypass and single-process flags from older revisions are no longer enabled by default. Use a dedicated account and comply with the applicable terms, permissions, and privacy requirements for the content being archived.
