document.addEventListener('DOMContentLoaded', () => {
    const grid = document.getElementById('galleryGrid');
    const refreshBtn = document.getElementById('refreshBtn');
    let knownFiles = new Set();

    // Fetch and render media
    async function syncGallery() {
        try {
            const res = await fetch('/api/files');
            const files = await res.json();

            let addedCount = 0;

            // Reverse to append oldest first in the DOM logic, or handle based on order
            files.forEach(file => {
                if (!knownFiles.has(file.filename)) {
                    knownFiles.add(file.filename);
                    const card = createCard(file);
                    // Prepend so newest appears at the top
                    grid.insertBefore(card, grid.firstChild);
                    addedCount++;
                    
                    // Small stagger animation
                    setTimeout(() => {
                        card.style.opacity = '1';
                        card.style.transform = 'translateY(0)';
                    }, 50 * addedCount);
                }
            });
        } catch (error) {
            console.error("Gallery Sync Error:", error);
        }
    }

    // DOM construction
    function createCard(file) {
        const card = document.createElement('div');
        card.className = 'media-card';
        card.style.opacity = '0';
        card.style.transform = 'translateY(20px)';
        card.style.transition = 'opacity 0.5s ease, transform 0.5s ease';

        const wrapper = document.createElement('div');
        wrapper.className = 'media-wrapper';

        if (file.type === 'video') {
            const video = document.createElement('video');
            video.src = file.url;
            video.controls = true;
            video.loop = true;
            // STOV specific audio normalization logic ensures these play fine
            wrapper.appendChild(video);
        } else {
            const img = document.createElement('img');
            img.src = file.url;
            img.loading = 'lazy';
            wrapper.appendChild(img);
        }

        const meta = document.createElement('div');
        meta.className = 'card-meta';
        
        const user = document.createElement('span');
        user.className = 'username';
        user.textContent = `@${file.username}`;

        const badge = document.createElement('span');
        badge.className = 'type-badge';
        badge.textContent = file.type;

        meta.appendChild(user);
        meta.appendChild(badge);

        card.appendChild(wrapper);
        card.appendChild(meta);

        return card;
    }

    // Initialize
    syncGallery();

    // Live Auto-Update every 5 seconds (Zero-reload injection)
    setInterval(syncGallery, 5000);

    // Manual Refresh Trigger
    refreshBtn.addEventListener('click', () => {
        refreshBtn.style.transform = 'scale(0.95)';
        setTimeout(() => refreshBtn.style.transform = 'none', 150);
        syncGallery();
    });
});

