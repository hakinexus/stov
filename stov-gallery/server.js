const express = require('express');
const fs = require('fs');
const path = require('path');
const cors = require('cors');

const app = express();
const PORT = 3000;

// Path to STOV's downloads directory
const DOWNLOADS_DIR = path.join(__dirname, '../downloads');

app.use(cors());
app.use(express.static(path.join(__dirname, 'public')));
app.use('/media', express.static(DOWNLOADS_DIR));

// Helper: Format bytes to human-readable sizes
function formatBytes(bytes, decimals = 2) {
    if (!+bytes) return '0 Bytes';
    const k = 1024;
    const dm = decimals < 0 ? 0 : decimals;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(dm))} ${sizes[i]}`;
}

app.get('/api/files', (req, res) => {
    if (!fs.existsSync(DOWNLOADS_DIR)) return res.json([]);

    try {
        const files = fs.readdirSync(DOWNLOADS_DIR);
        const mediaFiles = files
            .filter(file => file.endsWith('.mp4') || file.endsWith('.jpg'))
            .map(file => {
                const filePath = path.join(DOWNLOADS_DIR, file);
                const stats = fs.statSync(filePath);
                const parts = file.split('_');
                const username = parts[0] || 'Unknown';
                
                // Format the date nicely
                const dateObj = new Date(stats.birthtimeMs);
                const formattedDate = dateObj.toLocaleDateString() + ' ' + dateObj.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

                return {
                    id: file,
                    filename: file,
                    url: `/media/${file}`,
                    type: file.endsWith('.mp4') ? 'video' : 'image',
                    username: username,
                    size: formatBytes(stats.size),
                    createdAt: stats.birthtimeMs,
                    dateString: formattedDate
                };
            })
            .sort((a, b) => b.createdAt - a.createdAt);

        res.json(mediaFiles);
    } catch (error) {
        console.error("Error reading downloads:", error);
        res.status(500).json({ error: "Failed to read media" });
    }
});

app.listen(PORT, () => console.log(`✨ STOV Gallery live at http://localhost:${PORT}`));

