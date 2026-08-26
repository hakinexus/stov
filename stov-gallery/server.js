const express = require('express');
const fs = require('fs');
const path = require('path');
const cors = require('cors');
const chokidar = require('chokidar');

const app = express();
const PORT = Number(process.env.PORT || 3000);
const HOST = process.env.HOST || '127.0.0.1';
const DOWNLOADS_DIR = path.resolve(__dirname, '../downloads');
const PUBLIC_DIR = path.resolve(__dirname, 'public');

fs.mkdirSync(DOWNLOADS_DIR, { recursive: true });
app.disable('x-powered-by');
app.use(cors({ origin: true }));
app.use(express.json({ limit: '64kb' }));
app.use(express.static(PUBLIC_DIR, { etag: true, index: 'index.html' }));
app.use('/media', express.static(DOWNLOADS_DIR, { fallthrough: false, maxAge: '1h' }));

function formatBytes(bytes, decimals = 2) {
    if (!Number.isFinite(bytes) || bytes <= 0) return '0 Bytes';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
    return `${parseFloat((bytes / (1024 ** index)).toFixed(decimals))} ${units[index]}`;
}

function isPublishedMedia(filename) {
    const lower = filename.toLowerCase();
    return (lower.endsWith('.mp4') || lower.endsWith('.jpg') || lower.endsWith('.jpeg') ||
        lower.endsWith('.png') || lower.endsWith('.webp')) && !filename.includes('.part');
}

function fallbackUsername(filename) {
    // Legacy files used username_timestamp.ext. New files use a manifest instead.
    const match = filename.match(/^(.*)_\d+_[^.]+\.(?:mp4|jpg|jpeg|png|webp)$/i);
    return match?.[1] || filename.replace(/\.[^.]+$/, '');
}

function readManifest(filename) {
    const manifestPath = path.join(DOWNLOADS_DIR, `${filename}.json`);
    try {
        if (!fs.existsSync(manifestPath)) return null;
        return JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
    } catch (_) {
        return null;
    }
}

function getMediaFiles() {
    let filenames;
    try {
        filenames = fs.readdirSync(DOWNLOADS_DIR);
    } catch (_) {
        return [];
    }

    const files = [];
    for (const filename of filenames) {
        if (!isPublishedMedia(filename)) continue;
        const filePath = path.join(DOWNLOADS_DIR, filename);
        try {
            const stats = fs.statSync(filePath);
            if (!stats.isFile() || stats.size === 0) continue;
            const manifest = readManifest(filename);
            const createdAt = Number(manifest?.created_at)
                ? Number(manifest.created_at) * 1000
                : stats.mtimeMs;
            files.push({
                id: filename,
                filename,
                url: `/media/${encodeURIComponent(filename)}`,
                type: manifest?.media_type || (filename.toLowerCase().endsWith('.mp4') ? 'video' : 'image'),
                username: manifest?.username || fallbackUsername(filename),
                status: manifest?.status || 'complete',
                hasAudio: manifest?.has_audio ?? null,
                error: manifest?.error || null,
                rawSize: stats.size,
                size: formatBytes(stats.size),
                createdAt,
                dateString: new Date(createdAt).toLocaleString(),
            });
        } catch (_) {
            // Files may be renamed or deleted while the scraper is publishing them.
        }
    }
    return files.sort((a, b) => b.createdAt - a.createdAt);
}

function safePublishedPath(filename) {
    const clean = path.basename(String(filename || ''));
    const resolved = path.resolve(DOWNLOADS_DIR, clean);
    const root = `${DOWNLOADS_DIR}${path.sep}`;
    if (!clean || !resolved.startsWith(root) || !isPublishedMedia(clean)) return null;
    return resolved;
}

app.get('/api/health', (_req, res) => res.json({ ok: true, service: 'stov-gallery' }));
app.get('/api/files', (_req, res) => res.json(getMediaFiles()));

app.get('/api/stats', (_req, res) => {
    const files = getMediaFiles();
    const totalSize = files.reduce((sum, file) => sum + file.rawSize, 0);
    res.json({
        count: files.length,
        size: formatBytes(totalSize),
        targets: new Set(files.map(file => file.username)).size,
        complete: files.filter(file => file.status === 'complete').length,
        videoOnly: files.filter(file => file.status === 'video-only').length,
    });
});

function deleteOne(filename) {
    const filePath = safePublishedPath(filename);
    if (!filePath || !fs.existsSync(filePath)) return false;
    fs.unlinkSync(filePath);
    const manifestPath = `${filePath}.json`;
    if (fs.existsSync(manifestPath)) fs.unlinkSync(manifestPath);
    return true;
}

app.delete('/api/files/:filename', (req, res) => {
    try {
        if (!deleteOne(req.params.filename)) return res.status(404).json({ error: 'File not found' });
        scheduleBroadcast();
        return res.json({ success: true });
    } catch (error) {
        return res.status(500).json({ error: `Deletion failed: ${error.message}` });
    }
});

app.post('/api/files/bulk-delete', (req, res) => {
    if (!Array.isArray(req.body?.files)) return res.status(400).json({ error: 'Invalid payload' });
    let deleted = 0;
    for (const filename of req.body.files) {
        try {
            if (deleteOne(filename)) deleted += 1;
        } catch (_) {
            // Continue deleting the remaining safe filenames.
        }
    }
    if (deleted) scheduleBroadcast();
    res.json({ success: true, deleted });
});

const clients = new Set();
app.get('/api/stream', (_req, res) => {
    res.setHeader('Content-Type', 'text/event-stream');
    res.setHeader('Cache-Control', 'no-cache, no-transform');
    res.setHeader('Connection', 'keep-alive');
    res.flushHeaders?.();
    res.write('retry: 2000\n\n');
    clients.add(res);

    const heartbeat = setInterval(() => {
        try { res.write(': keep-alive\n\n'); } catch (_) { cleanup(); }
    }, 15000);
    const cleanup = () => {
        clearInterval(heartbeat);
        clients.delete(res);
    };
    _req.on('close', cleanup);
});

function broadcastUpdate() {
    for (const client of [...clients]) {
        try {
            client.write(`event: update\ndata: ${Date.now()}\n\n`);
        } catch (_) {
            clients.delete(client);
        }
    }
}

let broadcastTimer = null;
function scheduleBroadcast() {
    clearTimeout(broadcastTimer);
    broadcastTimer = setTimeout(() => {
        broadcastTimer = null;
        broadcastUpdate();
    }, 250);
}

const watcher = chokidar.watch(DOWNLOADS_DIR, {
    ignoreInitial: true,
    awaitWriteFinish: { stabilityThreshold: 500, pollInterval: 100 },
    ignored: /(^|[\\/])\../,
});
watcher.on('add', scheduleBroadcast).on('change', scheduleBroadcast).on('unlink', scheduleBroadcast);

const server = app.listen(PORT, HOST, () => {
    console.log(`STOV gallery listening at http://${HOST}:${PORT}`);
});

function shutdown() {
    watcher.close().catch(() => {});
    for (const client of clients) client.end();
    server.close(() => process.exit(0));
}
process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);
