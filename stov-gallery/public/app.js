document.addEventListener('DOMContentLoaded', () => {
    const grid = document.getElementById('galleryGrid');
    const refreshBtn = document.getElementById('refreshBtn');
    const searchInput = document.getElementById('searchInput');
    const filterBtns = document.querySelectorAll('.filter-btn');
    const emptyState = document.getElementById('emptyState');
    
    // Lightbox elements
    const lightbox = document.getElementById('lightbox');
    const lightboxContent = document.querySelector('.lightbox-content');
    const lightboxClose = document.querySelector('.lightbox-close');

    // Global State
    let allMedia = [];
    let knownFiles = new Set();
    let currentFilter = 'all';
    let searchQuery = '';

    // Smart Video Observer (Plays videos only when visible on screen)
    const videoObserver = new IntersectionObserver((entries) => {
        entries.forEach(entry => {
            const video = entry.target;
            if (entry.isIntersecting) {
                video.play().catch(() => {}); // Catch auto-play restrictions
            } else {
                video.pause();
            }
        });
    }, { threshold: 0.5 });

    // Toast Notification System
    function showToast(message) {
        const container = document.getElementById('toastContainer');
        const toast = document.createElement('div');
        toast.className = 'toast show';
        toast.textContent = message;
        container.appendChild(toast);
        setTimeout(() => {
            toast.classList.remove('show');
            setTimeout(() => toast.remove(), 300);
        }, 3000);
    }

    // Lightbox System
    function openLightbox(file) {
        lightboxContent.innerHTML = '';
        if (file.type === 'video') {
            const video = document.createElement('video');
            video.src = file.url;
            video.controls = true;
            video.autoplay = true;
            lightboxContent.appendChild(video);
        } else {
            const img = document.createElement('img');
            img.src = file.url;
            lightboxContent.appendChild(img);
        }
        lightbox.classList.add('active');
        document.body.style.overflow = 'hidden'; // Prevent background scrolling
    }

    lightboxClose.addEventListener('click', () => {
        lightbox.classList.remove('active');
        document.body.style.overflow = 'auto';
        lightboxContent.innerHTML = ''; // Stop video playback
    });

    // Create DOM Node for Media
    function createCard(file) {
        const card = document.createElement('div');
        card.className = 'media-card';
        card.dataset.id = file.id;
        card.dataset.type = file.type;
        card.dataset.user = file.username.toLowerCase();

        const wrapper = document.createElement('div');
        wrapper.className = 'media-wrapper';

        if (file.type === 'video') {
            const video = document.createElement('video');
            video.src = file.url;
            video.loop = true;
            video.muted = true; // Required for autoplay policies
            video.playsInline = true; // Crucial for iOS
            
            const svgPlay = `<svg class="play-icon" width="40" height="40" viewBox="0 0 24 24" fill="white"><path d="M8 5v14l11-7z"/></svg>`;
            wrapper.innerHTML = svgPlay;
            wrapper.appendChild(video);
            videoObserver.observe(video);
        } else {
            const img = document.createElement('img');
            img.src = file.url;
            img.loading = 'lazy';
            wrapper.appendChild(img);
        }

        const meta = document.createElement('div');
        meta.className = 'card-meta';
        meta.innerHTML = `
            <div class="meta-left">
                <span class="username">@${file.username}</span>
                <span class="timestamp">${file.dateString}</span>
            </div>
            <span class="size-badge">${file.size}</span>
        `;

        card.appendChild(wrapper);
        card.appendChild(meta);

        // Click to open lightbox
        card.addEventListener('click', () => openLightbox(file));

        return card;
    }

    // Filter Logic
    function applyFilters() {
        const cards = document.querySelectorAll('.media-card');
        let visibleCount = 0;

        cards.forEach(card => {
            const matchType = currentFilter === 'all' || card.dataset.type === currentFilter;
            const matchSearch = card.dataset.user.includes(searchQuery);

            if (matchType && matchSearch) {
                card.classList.remove('hidden');
                visibleCount++;
            } else {
                card.classList.add('hidden');
            }
        });

        emptyState.classList.toggle('hidden', visibleCount > 0);
    }

    // Fetch and sync data
    async function syncGallery(isAuto = false) {
        try {
            const res = await fetch('/api/files');
            const files = await res.json();
            
            allMedia = files; // Update local state
            let newFilesCount = 0;

            // Iterate backward so we can prepend and maintain newest-first order
            files.slice().reverse().forEach(file => {
                if (!knownFiles.has(file.filename)) {
                    knownFiles.add(file.filename);
                    const card = createCard(file);
                    grid.insertBefore(card, grid.firstChild);
                    newFilesCount++;
                }
            });

            if (isAuto && newFilesCount > 0) {
                showToast(`Intercepted ${newFilesCount} new target${newFilesCount > 1 ? 's' : ''}`);
            }

            // Sync empty state if absolutely no files exist
            if (knownFiles.size === 0) {
                emptyState.classList.remove('hidden');
            } else {
                applyFilters(); // Re-apply current filters to new DOM
            }
            
        } catch (error) {
            console.error("Gallery Sync Error:", error);
        }
    }

    // Event Listeners for UI Controls
    refreshBtn.addEventListener('click', () => syncGallery(false));

    searchInput.addEventListener('input', (e) => {
        searchQuery = e.target.value.toLowerCase().trim();
        applyFilters();
    });

    filterBtns.forEach(btn => {
        btn.addEventListener('click', (e) => {
            filterBtns.forEach(b => b.classList.remove('active'));
            e.target.classList.add('active');
            currentFilter = e.target.dataset.filter;
            applyFilters();
        });
    });

    // Initialize
    syncGallery(false);

    // Live Auto-Update every 5 seconds
    setInterval(() => syncGallery(true), 5000);
});

