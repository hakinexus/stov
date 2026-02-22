const express = require('express');
const fs = require('fs');
const path = require('path');
const cors = require('cors');

const app = express();
const PORT = 3000;
const DOWNLOADS_DIR = path.join(__dirname, '../downloads');

app.use(cors());
app.use(express.json()); // Required for bulk delete payloads
app.use(express.static(path.join(__dirname, 'public')));
app.use('/media', express.static(DOWNLOADS_DIR));

// Helper: Format Bytes
function formatBytes(bytes, decimals = 2) {
    if (!+bytes) return '0 Bytes';
    const k = 1024;
    const dm = decimals < 0 ? 0 : decimals;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(dm))} ${sizes[i]}`;
}

// Core Data Fetcher
function getMediaFiles() {
    if (!fs.existsSync(DOWNLOADS_DIR)) return [];
    try {
        const files = fs.readdirSync(DOWNLOADS_DIR);
        return files
            .filter(f => f.endsWith('.mp4') || f.endsWith('.jpg') || f.endsWith('.png'))
            .map(file => {
                const filePath = path.join(DOWNLOADS_DIR, file);
                const stats = fs.statSync(filePath);
                const username = file.split('_')[0] || 'Unknown';
                const dateObj = new Date(stats.birthtimeMs);
                
                return {
                    id: file,
                    filename: file,
                    url: `/media/${file}`,
                    type: file.endsWith('.mp4') ? 'video' : 'image',
                    username,
                    rawSize: stats.size,
                    size: formatBytes(stats.size),
                    createdAt: stats.birthtimeMs,
                    dateString: `${dateObj.toLocaleDateString()} ${dateObj.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`
                };
            })
            .sort((a, b) => b.createdAt - a.createdAt);
    } catch (e) {
        return [];
    }
}

// --- ENDPOINTS ---

app.get('/api/files', (req, res) => res.json(getMediaFiles()));

app.get('/api/stats', (req, res) => {
    const files = getMediaFiles();
    const totalSize = files.reduce((acc, f) => acc + f.rawSize, 0);
    const uniqueTargets = new Set(files.map(f => f.username)).size;
    res.json({
        count: files.length,
        size: formatBytes(totalSize),
        targets: uniqueTargets
    });
});

// Single Delete
app.delete('/api/files/:filename', (req, res) => {
    try {
        const filePath = path.join(DOWNLOADS_DIR, path.basename(req.params.filename));
        if (fs.existsSync(filePath) && filePath.startsWith(DOWNLOADS_DIR)) {
            fs.unlinkSync(filePath);
            broadcastUpdate();
            return res.json({ success: true });
        }
        res.status(404).json({ error: "File not found" });
    } catch (e) { res.status(500).json({ error: "Deletion failed" }); }
});

// Bulk Delete
app.post('/api/files/bulk-delete', (req, res) => {
    const { files } = req.body;
    if (!Array.isArray(files)) return res.status(400).json({ error: "Invalid payload" });
    
    let deleted = 0;
    files.forEach(filename => {
        const filePath = path.join(DOWNLOADS_DIR, path.basename(filename));
        if (fs.existsSync(filePath) && filePath.startsWith(DOWNLOADS_DIR)) {
            try { fs.unlinkSync(filePath); deleted++; } catch(e){}
        }
    });
    if (deleted > 0) broadcastUpdate();
    res.json({ success: true, deleted });
});

// --- REAL-TIME ENGINE (SSE) ---
let clients = [];
app.get('/api/stream', (req, res) => {
    res.setHeader('Content-Type', 'text/event-stream');
    res.setHeader('Cache-Control', 'no-cache');
    res.setHeader('Connection', 'keep-alive');
    
    clients.push(res);
    req.on('close', () => { clients = clients.filter(client => client !== res); });
});

function broadcastUpdate() {
    clients.forEach(client => client.write(`data: update\n\n`));
}

// Watch filesystem for STOV scraping updates
if (fs.existsSync(DOWNLOADS_DIR)) {
    let fsTimeout;
    fs.watch(DOWNLOADS_DIR, (eventType, filename) => {
        if (!fsTimeout) {
            broadcastUpdate();
            fsTimeout = setTimeout(() => { fsTimeout = null; }, 500); // Debounce
        }
    });
}

app.listen(PORT, () => console.log(`✨ STOV Masterclass Server live at http://localhost:${PORT}`));

