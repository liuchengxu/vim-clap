// Diff Overlay - Shows changes since last file view
// Displays inline diff with added/removed lines

/**
 * Store the current diff result for later display.
 * Called after fetching diff from backend.
 */
function setCurrentDiff(diff) {
    currentDiff = diff;
}

/**
 * Get the current diff result.
 */
function getCurrentDiff() {
    return currentDiff;
}

/**
 * Toggle the diff overlay visibility.
 */
function toggleDiffOverlay() {
    if (!currentDiff) {
        showToast('No previous version available');
        return;
    }

    if (!currentDiff.has_changes) {
        showToast('No changes since last view');
        return;
    }

    diffOverlayVisible = !diffOverlayVisible;

    if (diffOverlayVisible) {
        showDiffOverlay(currentDiff);
    } else {
        hideDiffOverlay();
    }
}

/**
 * Show the diff overlay with the given diff data.
 */
function showDiffOverlay(diff) {
    // Remove existing overlay if any
    hideDiffOverlay();

    const overlay = document.createElement('div');
    overlay.id = 'diff-overlay';
    overlay.className = 'diff-overlay';

    // Create header with timestamps
    const header = document.createElement('div');
    header.className = 'diff-header';

    const snapshotTime = formatRelativeTime(diff.snapshot_time);
    const currentTime = formatRelativeTime(diff.current_time);

    header.innerHTML = `
        <div class="diff-title">Changes since last view</div>
        <div class="diff-timestamps">
            <span class="diff-from">From: ${snapshotTime}</span>
            <span class="diff-arrow">→</span>
            <span class="diff-to">To: ${currentTime}</span>
        </div>
        <button class="diff-close-btn" title="Close (Ctrl+D or Escape)">×</button>
    `;

    // Create content area with diff lines
    const content = document.createElement('div');
    content.className = 'diff-content';

    // Render diff lines
    let html = '';
    for (const change of diff.changes) {
        const escapedContent = escapeHtml(change.content.replace(/\n$/, ''));
        const lineContent = escapedContent || '&nbsp;'; // Show empty lines

        switch (change.kind) {
            case 'Add':
                html += `<div class="diff-line diff-line-add"><span class="diff-marker">+</span>${lineContent}</div>`;
                break;
            case 'Remove':
                html += `<div class="diff-line diff-line-remove"><span class="diff-marker">-</span>${lineContent}</div>`;
                break;
            case 'Equal':
                html += `<div class="diff-line diff-line-equal"><span class="diff-marker"> </span>${lineContent}</div>`;
                break;
        }
    }
    content.innerHTML = html;

    overlay.appendChild(header);
    overlay.appendChild(content);
    document.body.appendChild(overlay);

    // Set up event listeners
    overlay.querySelector('.diff-close-btn').onclick = () => {
        hideDiffOverlay();
        diffOverlayVisible = false;
    };

    // Show with animation
    requestAnimationFrame(() => {
        overlay.classList.add('visible');
    });
}

/**
 * Hide the diff overlay.
 */
function hideDiffOverlay() {
    const overlay = document.getElementById('diff-overlay');
    if (overlay) {
        overlay.classList.remove('visible');
        setTimeout(() => {
            overlay.remove();
        }, 200);
    }
    diffOverlayVisible = false;
}

/**
 * Check if diff overlay is currently visible.
 */
function isDiffOverlayVisible() {
    return diffOverlayVisible;
}

// Export diff functions for use in platform-specific modules
window.setCurrentDiff = setCurrentDiff;
window.getCurrentDiff = getCurrentDiff;
window.toggleDiffOverlay = toggleDiffOverlay;
window.hideDiffOverlay = hideDiffOverlay;
window.isDiffOverlayVisible = isDiffOverlayVisible;
