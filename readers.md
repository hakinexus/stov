# Maintainer Guide

This document is a compact map of STOV for anyone reading or extending the code.

## Start here

| Read | Why |
|---|---|
| `src/main.rs` | CLI input, profile selection, browser startup, login, and target dispatch |
| `src/browser.rs` | Chromium path discovery, viewport, sandbox, and runtime flags |
| `src/instagram.rs` | Login, story-viewer state, media selection, CDP response capture, and download orchestration |
| `src/utils.rs` | Atomic writes, validation, manifests, session storage, and FFmpeg/FFprobe integration |
| `src/config.rs` | Selectors, filesystem locations, and browser defaults |
| `stov-gallery/server.js` | Local media API, file watcher, SSE stream, safe deletion, and static serving |
| `stov-gallery/public/app.js` | Filtering, selection, lightbox, scrolling, and live gallery updates |

## Data flow

```text
CLI input
   │
   ▼
Chromium session ── login/session cookie ──▶ Instagram profile
   │                                           │
   │                                           ▼
   │                                  Story viewer state
   │                                           │
   ├── DOM media URL ── authenticated browser fetch ──┐
   │                                                   │
   └── CDP response listener ── blob/media body ───────┤
                                                       ▼
                                           atomic write + FFprobe
                                                       │
                                      optional FFmpeg audio mux
                                                       │
                                                       ▼
                                  validated media + JSON manifest
                                                       │
                                                       ▼
                                              local gallery API
```

## Reliability rules

The scraper must not infer a new story from a URL change alone. A story transition is accepted only after the observed viewer fingerprint changes or the viewer closes. Playback progress is not part of the fingerprint because it changes during the same story.

A media file is never considered published merely because a request returned bytes. Images are checked by signature, video and audio containers are inspected with FFprobe, and FFmpeg output is validated before it replaces the temporary artifact. If audio cannot be validated, the result is published as `video-only` with a manifest status instead of being marked complete.

The gallery only lists published media extensions and ignores temporary files. Every API path that accepts a filename is reduced to a basename and checked against the downloads directory. Lightbox scroll locking is paired with one idempotent unlock path that restores the body styles and original scroll position.

## Change checklist

Before changing selectors, capture a real page state and update the semantic fallback list in `src/config.rs`. Before changing media handling, add a unit test for the pure decision rule and keep FFprobe validation at the publication boundary. Before changing the gallery server, run `npm test` and exercise `/api/health`, `/api/files`, and `/api/stats` locally.

Run the complete quality gate from the repository root:

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cd stov-gallery
npm ci
npm test
```

## Private data

`profiles/` contains session credentials. `downloads/` contains archived media. `images/` and logs may contain account or content information. None of these directories belong in commits or public issue reports.
