const express = require('express');
const fs = require('fs');
const path = require('path');
const cors = require('cors');

const app = express();
const PORT = 3000;

// Path to STOV's downloads directory (relative to this server)
const DOWNLOADS_DIR = path.join(__dirname, '../downloads');

app.use(cors());
app.use(express.static(path.join(__dirname, 'public')));
app.use('/media', express.static(DOWNLOADS_DIR));

// API Endpoint to fetch the live list of files
app.get('/api/files', (req, res) => {
    if (!fs.existsSync(DOWNLOADS_DIR)) {
        return res.json([]);
    }

    try {
        const files = fs.readdirSync(DOWNLOADS_DIR);
        const mediaFiles = files
            .filter(file => file.endsWith('.mp4') || file.endsWith('.jpg'))
            .map(file => {
                const filePath = path.join(DOWNLOADS_DIR, file);
                const stats = fs.statSync(filePath);
                
                // STOV format: username_timestamp_id.ext
                const parts = file.split('_');
                const username = parts[0] || 'Unknown';

                return {
                    filename: file,
                    url: `/media/${file}`,
                    type: file.endsWith('.mp4') ? 'video' : 'image',
                    username: username,
                    createdAt: stats.birthtimeMs // For accurate chronological sorting
                };
            })
            .sort((a, b) => b.createdAt - a.createdAt); // Newest first

        res.json(mediaFiles);
    } catch (error) {
        console.error("Error reading downloads:", error);
        res.status(500).json({ error: "Failed to read media" });
    }
});

app.listen(PORT, () => {
    console.log(`✨ STOV Glass Gallery live at http://localhost:${PORT}`);
});

