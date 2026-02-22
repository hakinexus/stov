document.addEventListener('DOMContentLoaded', () => {
    // --- DOM Elements ---
    const grid = document.getElementById('galleryGrid');
    const emptyState = document.getElementById('emptyState');
    const themeSwitcher = document.getElementById('themeSwitcher');
    const searchInput = document.getElementById('searchInput');
    const filterBtns = document.querySelectorAll('.filter-btn');
    const connectionStatus = document.getElementById('connectionStatus');
    
    // Lightbox
    const lightbox = document.getElementById('lightbox');
    const lightboxContent = document.getElementById('lightboxContent');
    const lightboxMeta = document.getElementById('lightboxMeta');
    const btnLbClose = document.querySelector('.lightbox-close');
    const btnLbDelete = document.getElementById('lightboxDeleteBtn');
    const btnLbDownload = document.getElementById('lightboxDownloadBtn');
    const btnLbPrev = document.getElementById('lightboxPrevBtn');
    const btnLbNext = document.getElementById('lightboxNextBtn');

    // Bulk & Stats
    const selectModeBtn = document.getElementById('selectModeBtn');
    const bulkActionBar = document.getElementById('bulkActionBar');
    const bulkCount = document.getElementById('bulkCount');
    const statCount = document.getElementById('statCount');
    const statSize = document.getElementById('statSize');
    const statTargets = document.getElementById('statTargets');

    // --- State Management ---
    let allMedia = [];          // Complete sorted array from server
    let visibleMedia = [];      // Array of currently filtered media
    let knownFiles = new Map(); // filename -> DOM Element
    let selectedFiles = new Set();
    
    let currentFilter = 'all';
    let searchQuery = '';
    let isSelectionMode = false;
    let currentLightboxIndex = -1;

    // --- Init Theme & Mouse Tracker ---
    const savedTheme = localStorage.getItem('stov_theme') || 'dark';
    document.documentElement.setAttribute('data-theme', savedTheme);
    themeSwitcher.value = savedTheme;
    themeSwitcher.addEventListener('change', (e) => {
        document.documentElement.setAttribute('data-theme', e.target.value);
        localStorage.setItem('stov_theme', e.target.value);
    });

    const cursorBlob = document.getElementById('cursorBlob');
    document.addEventListener('mousemove', (e) => {
        cursorBlob.style.left = `${e.clientX}px`;
        cursorBlob.style.top = `${e.clientY}px`;
    });

    // --- Helpers ---
    function showToast(message, type = 'default') {
        const container = document.getElementById('toastContainer');
        const toast = document.createElement('div');
        toast.className = `toast ${type}`;
        const icon = type === 'error' ? '!' : '✓';
        toast.innerHTML = `<b style="font-size:1.2em">${icon}</b> <span>${message}</span>`;
        container.appendChild(toast);
        void toast.offsetWidth; 
        toast.classList.add('show');
        setTimeout(() => {
            toast.classList.remove('show');
            setTimeout(() => toast.remove(), 400);
        }, 3000);
    }

    const videoObserver = new IntersectionObserver((entries) => {
        entries.forEach(entry => {
            if (entry.isIntersecting) entry.target.play().catch(()=>{});
            else entry.target.pause();
        });
    }, { threshold: 0.5 });

    // --- Server Sync Engine ---
    async function updateStats() {
        try {
            const res = await fetch('/api/stats');
            const data = await res.json();
            statCount.innerText = `${data.count} files`;
            statSize.innerText = data.size;
            statTargets.innerText = `${data.targets} targets`;
        } catch (e) {}
    }

    async function syncGallery(silent = false) {
        try {
            const res = await fetch('/api/files');
            const files = await res.json();
            allMedia = files;

            const serverFilenames = new Set(files.map(f => f.filename));
            
            // Remove deleted items
            for (const [filename, card] of knownFiles.entries()) {
                if (!serverFilenames.has(filename)) {
                    card.classList.add('deleting');
                    setTimeout(() => card.remove(), 400);
                    knownFiles.delete(filename);
                    selectedFiles.delete(filename);
                }
            }

            // Add new items
            files.slice().reverse().forEach(file => {
                if (!knownFiles.has(file.filename)) {
                    const card = createCard(file);
                    grid.insertBefore(card, grid.firstChild);
                    knownFiles.set(file.filename, card);
                    if (!silent) showToast(`Intercepted: @${file.username}`);
                }
            });

            applyFilters();
            updateStats();
            updateBulkUI();
        } catch (error) { console.error("Sync Error:", error); }
    }

    // --- SSE Connection (Real-Time Push) ---
    function connectSSE() {
        const evtSource = new EventSource('/api/stream');
        evtSource.onmessage = (e) => {
            if (e.data === 'update') syncGallery(false);
        };
        evtSource.onopen = () => {
            connectionStatus.classList.remove('offline');
            connectionStatus.classList.add('pulse');
            syncGallery(true); 
        };
        evtSource.onerror = () => {
            connectionStatus.classList.add('offline');
            connectionStatus.classList.remove('pulse');
            evtSource.close();
            setTimeout(connectSSE, 3000); // Reconnect loop
        };
    }
    connectSSE();

    // --- Component Generators ---
    function createCard(file) {
        const card = document.createElement('div');
        card.className = 'media-card';
        card.dataset.id = file.id;
        
        // Add select overlay
        const overlay = document.createElement('div');
        overlay.className = 'select-overlay';
        card.appendChild(overlay);

        const wrapper = document.createElement('div');
        wrapper.className = 'media-wrapper';

        if (file.type === 'video') {
            const video = document.createElement('video');
            video.src = file.url; video.loop = true; video.muted = true; video.playsInline = true;
            wrapper.innerHTML = `<svg class="play-icon" width="48" height="48" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>`;
            wrapper.appendChild(video);
            videoObserver.observe(video);
        } else {
            const img = document.createElement('img');
            img.src = file.url; img.loading = 'lazy';
            wrapper.appendChild(img);
        }

        const meta = document.createElement('div');
        meta.className = 'card-meta';
        meta.innerHTML = `<div class="meta-left"><span class="username">@${file.username}</span><span class="timestamp">${file.dateString}</span></div><span class="size-badge">${file.size}</span>`;

        card.appendChild(wrapper);
        card.appendChild(meta);

        // Interaction Handler (Click vs Select)
        card.addEventListener('click', (e) => {
            if (isSelectionMode) {
                toggleSelection(file.filename, card);
            } else {
                openLightbox(file.filename);
            }
        });

        // Long press for mobile / right click to trigger select mode
        card.addEventListener('contextmenu', (e) => {
            e.preventDefault();
            if (!isSelectionMode) toggleSelectMode(true);
            toggleSelection(file.filename, card);
        });

        return card;
    }

    // --- Filters & UI States ---
    function applyFilters() {
        visibleMedia = [];
        allMedia.forEach(file => {
            const card = knownFiles.get(file.filename);
            if (!card) return;

            const matchType = currentFilter === 'all' || file.type === currentFilter;
            const matchSearch = file.username.toLowerCase().includes(searchQuery);

            if (matchType && matchSearch) {
                card.classList.remove('hidden');
                visibleMedia.push(file);
            } else {
                card.classList.add('hidden');
            }
        });
        emptyState.classList.toggle('hidden', visibleMedia.length > 0 || knownFiles.size > 0);
    }

    searchInput.addEventListener('input', (e) => { searchQuery = e.target.value.toLowerCase().trim(); applyFilters(); });
    filterBtns.forEach(btn => {
        if(btn.id === 'selectModeBtn') return;
        btn.addEventListener('click', (e) => {
            filterBtns.forEach(b => { if(b.id !== 'selectModeBtn') b.classList.remove('active')});
            e.target.classList.add('active');
            currentFilter = e.target.dataset.filter;
            applyFilters();
        });
    });

    // --- Bulk Selection Engine ---
    function toggleSelectMode(forceState = null) {
        isSelectionMode = forceState !== null ? forceState : !isSelectionMode;
        document.body.classList.toggle('selecting', isSelectionMode);
        selectModeBtn.classList.toggle('active', isSelectionMode);
        
        if (!isSelectionMode) {
            selectedFiles.clear();
            knownFiles.forEach(card => card.classList.remove('selected'));
        }
        updateBulkUI();
    }

    function toggleSelection(filename, card) {
        if (selectedFiles.has(filename)) {
            selectedFiles.delete(filename);
            card.classList.remove('selected');
        } else {
            selectedFiles.add(filename);
            card.classList.add('selected');
        }
        updateBulkUI();
    }

    function updateBulkUI() {
        bulkCount.innerText = selectedFiles.size;
        if (isSelectionMode && selectedFiles.size > 0) {
            bulkActionBar.classList.add('visible');
        } else {
            bulkActionBar.classList.remove('visible');
        }
    }

    selectModeBtn.addEventListener('click', () => toggleSelectMode());
    document.getElementById('bulkCancelBtn').addEventListener('click', () => toggleSelectMode(false));
    
    document.getElementById('bulkSelectAllBtn').addEventListener('click', () => {
        visibleMedia.forEach(f => {
            selectedFiles.add(f.filename);
            const card = knownFiles.get(f.filename);
            if(card) card.classList.add('selected');
        });
        updateBulkUI();
    });

    document.getElementById('bulkDeleteBtn').addEventListener('click', async () => {
        if (!confirm(`Permanently delete ${selectedFiles.size} selected items?`)) return;
        
        try {
            const res = await fetch('/api/files/bulk-delete', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ files: Array.from(selectedFiles) })
            });
            if (res.ok) {
                showToast(`Deleted ${selectedFiles.size} items`);
                toggleSelectMode(false);
                // SSE will trigger the sync to clear the DOM
            }
        } catch (e) { showToast('Bulk delete failed', 'error'); }
    });


    // --- Advanced Lightbox Engine ---
    function renderLightboxMedia(index) {
        if (index < 0 || index >= visibleMedia.length) return;
        currentLightboxIndex = index;
        const file = visibleMedia[index];
        
        lightboxContent.innerHTML = '';
        lightboxMeta.innerHTML = `<span style="color:var(--accent)">@${file.username}</span> <span style="opacity:0.6; font-size:0.9em; margin-left:8px;">${file.dateString}</span> <span style="opacity:0.6; font-size:0.8em; margin-left:8px; border:1px solid rgba(255,255,255,0.2); padding:2px 6px; border-radius:10px;">${file.size}</span>`;
        btnLbDownload.href = file.url;

        if (file.type === 'video') {
            const video = document.createElement('video');
            video.src = file.url; video.controls = true; video.autoplay = true;
            lightboxContent.appendChild(video);
        } else {
            const img = document.createElement('img');
            img.src = file.url;
            lightboxContent.appendChild(img);
        }

        btnLbPrev.style.display = index === 0 ? 'none' : 'flex';
        btnLbNext.style.display = index === visibleMedia.length - 1 ? 'none' : 'flex';
    }

    function openLightbox(filename) {
        const index = visibleMedia.findIndex(f => f.filename === filename);
        if (index !== -1) {
            renderLightboxMedia(index);
            lightbox.classList.add('active');
            document.body.style.overflow = 'hidden';
        }
    }

    function closeLightbox() {
        lightbox.classList.remove('active');
        document.body.style.overflow = 'auto';
        setTimeout(() => { lightboxContent.innerHTML = ''; currentLightboxIndex = -1; }, 300);
    }

    btnLbClose.addEventListener('click', closeLightbox);
    lightbox.addEventListener('click', (e) => { if (e.target === lightbox || e.target === lightboxContent) closeLightbox(); });

    // Lightbox Controls
    btnLbPrev.addEventListener('click', () => renderLightboxMedia(currentLightboxIndex - 1));
    btnLbNext.addEventListener('click', () => renderLightboxMedia(currentLightboxIndex + 1));

    document.addEventListener('keydown', (e) => {
        if (!lightbox.classList.contains('active')) return;
        if (e.key === 'Escape') closeLightbox();
        if (e.key === 'ArrowLeft') renderLightboxMedia(currentLightboxIndex - 1);
        if (e.key === 'ArrowRight') renderLightboxMedia(currentLightboxIndex + 1);
    });

    // Touch Swiping
    let touchStartX = 0;
    lightboxContent.addEventListener('touchstart', e => { touchStartX = e.changedTouches[0].screenX; });
    lightboxContent.addEventListener('touchend', e => {
        const touchEndX = e.changedTouches[0].screenX;
        if (touchEndX < touchStartX - 50) renderLightboxMedia(currentLightboxIndex + 1); // Swipe Left
        if (touchEndX > touchStartX + 50) renderLightboxMedia(currentLightboxIndex - 1); // Swipe Right
    });

    // Lightbox Delete Action
    btnLbDelete.addEventListener('click', async () => {
        if (currentLightboxIndex === -1) return;
        const file = visibleMedia[currentLightboxIndex];
        
        if (!confirm(`Delete this file?\nTarget: @${file.username}`)) return;

        try {
            const res = await fetch(`/api/files/${encodeURIComponent(file.filename)}`, { method: 'DELETE' });
            if (res.ok) {
                showToast('Asset deleted');
                // The SSE stream will update the DOM. Just handle lightbox transition.
                if (visibleMedia.length > 1) {
                    const nextIndex = currentLightboxIndex >= visibleMedia.length - 1 ? currentLightboxIndex - 1 : currentLightboxIndex;
                    // Note: visibleMedia array will update briefly after SSE fires, so we close to avoid desync, or we wait.
                    closeLightbox(); 
                } else {
                    closeLightbox();
                }
            }
        } catch (error) { showToast('Deletion failed', 'error'); }
    });

});

