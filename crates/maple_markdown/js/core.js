// Markdown Preview Core - Shared UI functionality
// This module contains all UI-related functions shared between vim-clap (WebSocket) and Tauri modes

// ============================================================================
// State Variables
// ============================================================================

let tocVisible = false;
let fontFamily = 'default';
let readerMode = false;
let currentFilePath = '';
let currentTheme = 'auto';
let zoomLevel = 100; // Percentage: 50-200

// Fuzzy finder state
let fuzzyFinderOpen = false;
let fuzzySearchMode = 'headings';
let fuzzySelectedIndex = 0;
let fuzzyResults = [];
let fuzzySearchIndex = { headings: [], text: [] };

// ============================================================================
// Utility Functions
// ============================================================================

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

function formatNumber(num) {
    return num.toLocaleString();
}

function formatReadingTime(minutes) {
    if (minutes < 1) {
        return '< 1 min';
    } else if (minutes === 1) {
        return '1 min';
    } else if (minutes < 60) {
        return minutes + ' min';
    } else {
        const hours = Math.floor(minutes / 60);
        const remainingMinutes = minutes % 60;
        if (remainingMinutes === 0) {
            return hours + (hours === 1 ? ' hour' : ' hours');
        }
        return hours + (hours === 1 ? ' hour ' : ' hours ') + remainingMinutes + ' min';
    }
}

function getFileBasename(path) {
    if (!path) return '';
    const parts = path.split('/');
    return parts[parts.length - 1];
}

function getFileDirectory(path) {
    if (!path) return '';
    const parts = path.split('/');
    if (parts.length <= 1) return '';
    return parts.slice(0, -1).join('/');
}

// ============================================================================
// Toast Notifications
// ============================================================================

function showToast(message, subtitle = null, duration = 2000) {
    const toast = document.createElement('div');
    toast.className = 'toast';

    if (subtitle) {
        const titleDiv = document.createElement('div');
        titleDiv.className = 'toast-title';
        titleDiv.textContent = message;
        toast.appendChild(titleDiv);

        const pathDiv = document.createElement('div');
        pathDiv.className = 'toast-path';
        pathDiv.textContent = subtitle;
        toast.appendChild(pathDiv);
    } else {
        toast.textContent = message;
    }

    document.body.appendChild(toast);

    setTimeout(() => {
        toast.classList.add('show');
    }, 10);

    setTimeout(() => {
        toast.classList.remove('show');
        setTimeout(() => {
            document.body.removeChild(toast);
        }, 300);
    }, duration);
}

async function copyToClipboard(text) {
    // Try Tauri clipboard API first (multiple possible paths in Tauri 2.x)
    if (window.__TAURI__) {
        const clipboardApi = window.__TAURI__.clipboard
            || window.__TAURI__.clipboardManager
            || window.__TAURI__.plugin?.clipboardManager;

        if (clipboardApi && clipboardApi.writeText) {
            try {
                await clipboardApi.writeText(text);
                console.log('Copied to clipboard (Tauri):', text);
                showToast('Copied: ' + getFileBasename(text), text);
                return;
            } catch (err) {
                console.error('Failed to copy via Tauri clipboard:', err);
                // Fall through to browser API
            }
        } else {
            console.log('Tauri clipboard API not found, available:', Object.keys(window.__TAURI__));
        }
    }

    // Fall back to browser clipboard API
    if (navigator.clipboard && navigator.clipboard.writeText) {
        try {
            await navigator.clipboard.writeText(text);
            console.log('Copied to clipboard (browser):', text);
            showToast('Copied: ' + getFileBasename(text), text);
            return;
        } catch (err) {
            console.error('Failed to copy via browser clipboard:', err);
            // Fall through to legacy method
        }
    }

    // Legacy fallback using execCommand
    const textArea = document.createElement('textarea');
    textArea.value = text;
    textArea.style.position = 'fixed';
    textArea.style.left = '-999999px';
    document.body.appendChild(textArea);
    textArea.focus();
    textArea.select();
    try {
        document.execCommand('copy');
        console.log('Copied to clipboard (legacy):', text);
        showToast('Copied: ' + getFileBasename(text), text);
    } catch (err) {
        console.error('Failed to copy to clipboard (legacy):', err);
        showToast('Failed to copy to clipboard');
    }
    document.body.removeChild(textArea);
}

// ============================================================================
// Code Highlighting & Rendering
// ============================================================================

function codeHighlight() {
    if (typeof hljs !== 'undefined') {
        document.querySelectorAll('pre code').forEach((el) => {
            hljs.highlightElement(el);
        });
    }
}

async function renderMermaid() {
    if (window.mermaid !== undefined) {
        await window.mermaid.run({
            querySelector: '.language-mermaid'
        });
    }
}

function renderLatex() {
    if (typeof renderMathInElement !== 'undefined') {
        renderMathInElement(document.getElementById('content'), {
            delimiters: [
                {left: "$$", right: "$$", display: true},
                {left: "$", right: "$", display: false}
            ]
        });
    }
}

// ============================================================================
// Zoom
// ============================================================================

const ZOOM_MIN = 50;
const ZOOM_MAX = 200;
const ZOOM_STEP = 10;

function applyZoom(level) {
    const content = document.getElementById('content');
    if (!content) return;

    zoomLevel = Math.max(ZOOM_MIN, Math.min(ZOOM_MAX, level));
    content.style.fontSize = `${zoomLevel}%`;
    localStorage.setItem('zoomLevel', zoomLevel);
    updateZoomDisplay();
}

function updateZoomDisplay() {
    const display = document.getElementById('zoom-level-display');
    if (display) {
        display.textContent = `${zoomLevel}%`;
    }
}

function zoomIn() {
    // Check if PDF viewer is active
    if (window.PdfViewer && window.PdfViewer.isActive()) {
        window.PdfViewer.zoomIn();
        const scale = window.PdfViewer.scale;
        showToast(`Zoom: ${Math.round(scale * 100)}%`);
        return;
    }
    applyZoom(zoomLevel + ZOOM_STEP);
    showToast(`Zoom: ${zoomLevel}%`);
}

function zoomOut() {
    // Check if PDF viewer is active
    if (window.PdfViewer && window.PdfViewer.isActive()) {
        window.PdfViewer.zoomOut();
        const scale = window.PdfViewer.scale;
        showToast(`Zoom: ${Math.round(scale * 100)}%`);
        return;
    }
    applyZoom(zoomLevel - ZOOM_STEP);
    showToast(`Zoom: ${zoomLevel}%`);
}

function resetZoom() {
    // Check if PDF viewer is active
    if (window.PdfViewer && window.PdfViewer.isActive()) {
        window.PdfViewer.resetZoom();
        showToast('Zoom reset to 100%');
        return;
    }
    applyZoom(100);
    showToast('Zoom reset to 100%');
}

// ============================================================================
// Navigation
// ============================================================================

function scrollToSourceLine(lineNumber) {
    const content = document.getElementById('content');
    if (!content) return;

    const children = Array.from(content.children);
    let closestElement = null;
    let closestDiff = Infinity;

    children.forEach((child) => {
        const sourceLine = parseInt(child.getAttribute('data-source-line') || '0');
        const diff = Math.abs(sourceLine - lineNumber);
        if (diff < closestDiff) {
            closestDiff = diff;
            closestElement = child;
        }
    });

    if (closestElement) {
        closestElement.scrollIntoView({ behavior: 'smooth', block: 'center' });
        closestElement.style.backgroundColor = '#ffeb3b';
        setTimeout(() => {
            closestElement.style.backgroundColor = '';
        }, 1000);
    }
}

function navigateToHeading(id, smooth = true) {
    const heading = document.getElementById(id);
    if (heading) {
        history.pushState(null, '', `#${id}`);
        heading.scrollIntoView({ behavior: smooth ? 'smooth' : 'auto', block: 'start' });
    }
}

function scrollToHash() {
    const hash = window.location.hash.slice(1);
    if (hash) {
        setTimeout(() => {
            const element = document.getElementById(hash);
            if (element) {
                element.scrollIntoView({ behavior: 'auto', block: 'start' });
            }
        }, 100);
    }
}

// ============================================================================
// Heading Anchors
// ============================================================================

function addHeadingAnchors() {
    const content = document.getElementById('content');
    if (!content) return;

    const headings = content.querySelectorAll('h1, h2, h3, h4, h5, h6');

    headings.forEach((heading, index) => {
        if (!heading.id) {
            heading.id = `heading-${index}`;
        }

        if (heading.querySelector('.heading-anchor')) return;

        heading.style.display = 'flex';
        heading.style.alignItems = 'center';
        heading.style.flexWrap = 'wrap';

        const anchor = document.createElement('a');
        anchor.className = 'heading-anchor';
        anchor.href = `#${heading.id}`;
        anchor.setAttribute('aria-label', `Link to ${heading.textContent}`);
        anchor.innerHTML = '<svg class="octicon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><path fill-rule="evenodd" d="M7.775 3.275a.75.75 0 001.06 1.06l1.25-1.25a2 2 0 112.83 2.83l-2.5 2.5a2 2 0 01-2.83 0 .75.75 0 00-1.06 1.06 3.5 3.5 0 004.95 0l2.5-2.5a3.5 3.5 0 00-4.95-4.95l-1.25 1.25zm-4.69 9.64a2 2 0 010-2.83l2.5-2.5a2 2 0 012.83 0 .75.75 0 001.06-1.06 3.5 3.5 0 00-4.95 0l-2.5 2.5a3.5 3.5 0 004.95 4.95l1.25-1.25a.75.75 0 00-1.06-1.06l-1.25 1.25a2 2 0 01-2.83 0z"></path></svg>';

        anchor.onclick = (e) => {
            e.preventDefault();
            navigateToHeading(heading.id);
        };

        heading.insertBefore(anchor, heading.firstChild);
    });
}

// ============================================================================
// Table of Contents
// ============================================================================

function generateTOC() {
    const content = document.getElementById('content');
    const tocContent = document.getElementById('toc-content');
    if (!content || !tocContent) return;

    const headings = content.querySelectorAll('h1, h2, h3, h4, h5, h6');
    tocContent.innerHTML = '';

    headings.forEach((heading, index) => {
        const level = heading.tagName.toLowerCase();
        // Get HTML content except the heading-anchor element, preserving <code> etc.
        const htmlContent = Array.from(heading.childNodes)
            .filter(n => !(n.nodeType === Node.ELEMENT_NODE && n.classList.contains('heading-anchor')))
            .map(n => n.nodeType === Node.TEXT_NODE ? escapeHtml(n.textContent) : n.outerHTML)
            .join('')
            .trim();
        const id = heading.id || `heading-${index}`;

        if (!heading.id) {
            heading.id = id;
        }

        const link = document.createElement('a');
        link.href = `#${id}`;
        link.innerHTML = htmlContent;
        link.className = `toc-${level}`;
        link.onclick = (e) => {
            e.preventDefault();
            navigateToHeading(id);
        };

        tocContent.appendChild(link);
    });
}

function getDefaultTOCWidth() {
    const screenWidth = window.innerWidth;
    if (screenWidth >= 1920) {
        return 350;
    } else if (screenWidth >= 1440) {
        return 300;
    } else {
        return 250;
    }
}

function toggleTOC(mode) {
    const tocPanel = document.getElementById('toc-panel');

    if (mode === 'off') {
        tocVisible = false;
        tocPanel.classList.remove('visible', 'toc-left', 'toc-right');
    } else {
        tocVisible = true;
        tocPanel.classList.add('visible');

        if (mode === 'left') {
            tocPanel.classList.remove('toc-right');
            tocPanel.classList.add('toc-left');
        } else if (mode === 'right') {
            tocPanel.classList.remove('toc-left');
            tocPanel.classList.add('toc-right');
        }

        generateTOC();

        const savedWidth = localStorage.getItem('tocWidth');
        if (savedWidth) {
            tocPanel.style.width = savedWidth + 'px';
        } else {
            const defaultWidth = getDefaultTOCWidth();
            tocPanel.style.width = defaultWidth + 'px';
        }
    }

    localStorage.setItem('tocMode', mode);
}

function setupSidebarResize() {
    const sidebar = document.getElementById('sidebar');
    const resizeHandle = document.getElementById('sidebar-resize-handle');
    if (!resizeHandle) return;

    let isResizing = false;
    let startX = 0;
    let startWidth = 0;

    // Restore saved width
    const savedWidth = localStorage.getItem('sidebarWidth');
    if (savedWidth) {
        const width = parseInt(savedWidth);
        if (width >= 180 && width <= 500) {
            sidebar.style.width = width + 'px';
        }
    }

    resizeHandle.addEventListener('mousedown', (e) => {
        isResizing = true;
        startX = e.clientX;
        startWidth = sidebar.offsetWidth;
        resizeHandle.classList.add('resizing');
        document.body.classList.add('sidebar-resizing');
        e.preventDefault();
    });

    document.addEventListener('mousemove', (e) => {
        if (!isResizing) return;

        const delta = e.clientX - startX;
        const newWidth = startWidth + delta;
        const minWidth = 180;
        const maxWidth = 500;

        if (newWidth >= minWidth && newWidth <= maxWidth) {
            sidebar.style.width = newWidth + 'px';
        }
    });

    document.addEventListener('mouseup', () => {
        if (isResizing) {
            isResizing = false;
            resizeHandle.classList.remove('resizing');
            document.body.classList.remove('sidebar-resizing');
            localStorage.setItem('sidebarWidth', sidebar.offsetWidth);
        }
    });
}

function setupTOCResize() {
    const tocPanel = document.getElementById('toc-panel');
    const resizeHandle = document.querySelector('.toc-resize-handle');
    let isResizing = false;
    let startX = 0;
    let startWidth = 0;

    resizeHandle.addEventListener('mousedown', (e) => {
        isResizing = true;
        startX = e.clientX;
        startWidth = tocPanel.offsetWidth;
        resizeHandle.classList.add('resizing');
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
        e.preventDefault();
    });

    document.addEventListener('mousemove', (e) => {
        if (!isResizing) return;

        const delta = e.clientX - startX;
        const isRightSide = tocPanel.classList.contains('toc-right');
        const newWidth = isRightSide ? startWidth - delta : startWidth + delta;
        const minWidth = 150;
        const maxWidth = 500;

        if (newWidth >= minWidth && newWidth <= maxWidth) {
            tocPanel.style.width = newWidth + 'px';
        }
    });

    document.addEventListener('mouseup', () => {
        if (isResizing) {
            isResizing = false;
            resizeHandle.classList.remove('resizing');
            document.body.style.cursor = '';
            document.body.style.userSelect = '';
            localStorage.setItem('tocWidth', tocPanel.offsetWidth);
        }
    });
}

function setupRecentFilesResize() {
    const section = document.getElementById('recent-files-section');
    const resizeHandle = document.getElementById('recent-files-resize-handle');
    if (!section || !resizeHandle) return;

    let isResizing = false;
    let startY = 0;
    let startHeight = 0;

    // Restore saved height
    const savedHeight = localStorage.getItem('recentFilesSectionHeight');
    if (savedHeight) {
        const height = parseInt(savedHeight);
        if (height >= 80 && height <= 500) {
            section.style.height = height + 'px';
        }
    }

    resizeHandle.addEventListener('mousedown', (e) => {
        isResizing = true;
        startY = e.clientY;
        startHeight = section.offsetHeight;
        resizeHandle.classList.add('resizing');
        document.body.classList.add('section-resizing');
        e.preventDefault();
    });

    document.addEventListener('mousemove', (e) => {
        if (!isResizing) return;

        const delta = e.clientY - startY;
        const newHeight = startHeight + delta;
        const minHeight = 80;
        const maxHeight = 500;

        if (newHeight >= minHeight && newHeight <= maxHeight) {
            section.style.height = newHeight + 'px';
        }
    });

    document.addEventListener('mouseup', () => {
        if (isResizing) {
            isResizing = false;
            resizeHandle.classList.remove('resizing');
            document.body.classList.remove('section-resizing');
            localStorage.setItem('recentFilesSectionHeight', section.offsetHeight);
        }
    });
}

// ============================================================================
// Theme & Font
// ============================================================================

function changeFontFamily(family) {
    fontFamily = family;
    const content = document.getElementById('content');
    if (!content) return;

    content.classList.remove('font-serif', 'font-mono', 'font-system', 'font-inter', 'font-merriweather', 'font-ibm-plex', 'font-literata');

    if (family !== 'default') {
        content.classList.add('font-' + family);
    }

    localStorage.setItem('fontFamily', family);
}

function toggleReaderMode(enabled) {
    readerMode = enabled;
    const content = document.getElementById('content');
    if (!content) return;

    if (enabled) {
        content.classList.add('reader-mode');
    } else {
        content.classList.remove('reader-mode');
    }

    localStorage.setItem('readerMode', enabled);
}

function changeTheme(theme) {
    currentTheme = theme;

    document.body.classList.remove(
        'theme-dark',
        'theme-material-dark',
        'theme-one-dark',
        'theme-ulysses',
        'theme-github-light',
        'theme-github-dark'
    );

    if (theme === 'auto') {
        // Use system preference
    } else if (theme === 'github-light') {
        // Default light theme
    } else if (theme === 'github-dark') {
        document.body.classList.add('theme-github-dark');
    } else if (theme === 'dark') {
        document.body.classList.add('theme-dark');
    } else if (theme === 'material-dark') {
        document.body.classList.add('theme-material-dark');
    } else if (theme === 'one-dark') {
        document.body.classList.add('theme-one-dark');
    } else if (theme === 'ulysses') {
        document.body.classList.add('theme-ulysses');
    }

    localStorage.setItem('theme', theme);
}

// ============================================================================
// Recent Files (localStorage-based)
// ============================================================================

function getRecentFiles() {
    const recent = localStorage.getItem('recentFiles');
    return recent ? JSON.parse(recent) : [];
}

function addToRecentFiles(filePath) {
    if (!filePath) return;

    let recentFiles = getRecentFiles();
    recentFiles = recentFiles.filter(f => f.path !== filePath);
    recentFiles.unshift({
        path: filePath,
        timestamp: Date.now()
    });
    recentFiles = recentFiles.slice(0, 10);
    localStorage.setItem('recentFiles', JSON.stringify(recentFiles));
}

// Custom tooltip for recent files
let pathTooltip = null;
let tooltipTimeout = null;

function createPathTooltip() {
    if (pathTooltip) return pathTooltip;

    pathTooltip = document.createElement('div');
    pathTooltip.className = 'path-tooltip';
    pathTooltip.innerHTML = '<div class="path-tooltip-content"></div>';
    document.body.appendChild(pathTooltip);

    return pathTooltip;
}

function showPathTooltip(element, fullPath) {
    const tooltip = createPathTooltip();
    const content = tooltip.querySelector('.path-tooltip-content');

    // Format path with segments
    const segments = fullPath.split('/').filter(s => s);
    const formatted = '/' + segments.map((seg, i) => {
        const isLast = i === segments.length - 1;
        return isLast ? `<span class="path-tooltip-file">${seg}</span>` : seg;
    }).join('<span class="path-tooltip-sep">/</span>');

    content.innerHTML = formatted;

    // Position tooltip to the right of the element
    const rect = element.getBoundingClientRect();
    tooltip.style.left = `${rect.right + 8}px`;
    tooltip.style.top = `${rect.top}px`;
    tooltip.classList.add('visible');
}

function hidePathTooltip() {
    if (pathTooltip) {
        pathTooltip.classList.remove('visible');
    }
}

function renderRecentFiles(onFileClick) {
    const container = document.getElementById('recent-files');
    if (!container) return;

    const recentFiles = getRecentFiles();

    if (recentFiles.length === 0) {
        container.innerHTML = '<div class="no-recent-files">No recent files</div>';
        return;
    }

    container.innerHTML = '';
    recentFiles.forEach(file => {
        const item = document.createElement('div');
        item.className = 'recent-file-item';
        if (file.path === currentFilePath) {
            item.classList.add('current');
        }

        const nameElement = document.createElement('div');
        nameElement.className = 'recent-file-name';
        nameElement.textContent = getFileBasename(file.path);

        const pathElement = document.createElement('div');
        pathElement.className = 'recent-file-path';
        pathElement.textContent = getFileDirectory(file.path);

        item.appendChild(nameElement);
        item.appendChild(pathElement);

        // Custom tooltip on hover
        item.addEventListener('mouseenter', () => {
            if (tooltipTimeout) clearTimeout(tooltipTimeout);
            tooltipTimeout = setTimeout(() => {
                showPathTooltip(item, file.path);
            }, 400);
        });

        item.addEventListener('mouseleave', () => {
            if (tooltipTimeout) {
                clearTimeout(tooltipTimeout);
                tooltipTimeout = null;
            }
            hidePathTooltip();
        });

        item.onclick = (e) => {
            if (e.shiftKey) {
                copyToClipboard(file.path);
                e.stopPropagation();
                return;
            }
            if (onFileClick) {
                onFileClick(file.path);
            }
        };

        item.oncontextmenu = (e) => {
            e.preventDefault();
            copyToClipboard(file.path);
        };

        container.appendChild(item);
    });
}

// ============================================================================
// File Path Bar
// ============================================================================

function updateFilePathBar(filePath, gitRoot) {
    const pathBar = document.getElementById('file-path-bar');
    if (!pathBar) return;

    if (filePath) {
        pathBar.style.display = 'block';
        pathBar.title = `Click to copy: ${filePath}`;

        if (gitRoot && filePath.startsWith(gitRoot)) {
            const gitRootName = gitRoot.split('/').filter(p => p).pop() || gitRoot;
            const relativePath = filePath.substring(gitRoot.length);
            pathBar.innerHTML = `<span class="git-root">${gitRootName}</span><span class="path-separator">/</span><span class="relative-path">${relativePath.replace(/^\//, '')}</span>`;
        } else {
            pathBar.textContent = filePath;
        }
    } else {
        pathBar.style.display = 'none';
    }
}

// ============================================================================
// Document Stats
// ============================================================================

function updateDocumentStats(stats) {
    if (!stats) return;

    const wordsEl = document.getElementById('stat-words');
    const charsEl = document.getElementById('stat-characters');
    const linesEl = document.getElementById('stat-lines');
    const timeEl = document.getElementById('stat-reading-time');

    if (wordsEl) wordsEl.textContent = formatNumber(stats.words);
    if (charsEl) charsEl.textContent = formatNumber(stats.characters);
    if (linesEl) linesEl.textContent = formatNumber(stats.lines);
    if (timeEl) timeEl.textContent = formatReadingTime(stats.reading_minutes || stats.reading_time_minutes);
}

// ============================================================================
// File Metadata Bar
// ============================================================================

function formatRelativeTime(timestamp) {
    if (!timestamp) return null;

    const now = Date.now();
    const diff = now - timestamp;

    const seconds = Math.floor(diff / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);

    if (seconds < 5) return 'just now';
    if (seconds < 60) return `${seconds}s ago`;
    if (minutes < 60) return `${minutes} min ago`;
    if (hours < 24) return `${hours}h ago`;
    if (days < 7) return `${days}d ago`;

    // For older files, show the date
    const date = new Date(timestamp);
    return date.toLocaleDateString();
}

function formatWordCount(words) {
    if (words == null) return null;
    if (words >= 1000) {
        return (words / 1000).toFixed(1).replace(/\.0$/, '') + 'k words';
    }
    return words.toLocaleString() + ' words';
}

function updateFileMetadata(modifiedAt, stats, gitBranch, gitBranchUrl, gitLastAuthor) {
    const metadataBar = document.getElementById('file-metadata-bar');
    const modifiedEl = document.getElementById('modified-time');
    const readTimeEl = document.getElementById('read-time');
    const branchEl = document.getElementById('git-branch');
    const branchContainer = document.getElementById('metadata-branch');
    const authorEl = document.getElementById('git-author');
    const wordCountEl = document.getElementById('word-count');
    const wordCountContainer = document.getElementById('metadata-words');

    if (!metadataBar) return;

    // Show/hide based on whether we have data
    const hasModified = modifiedAt != null;
    const hasReadTime = stats && (stats.reading_minutes || stats.reading_time_minutes);
    const hasBranch = gitBranch != null;
    const hasAuthor = gitLastAuthor != null;
    const hasWords = stats && stats.words != null;

    if (hasModified || hasReadTime || hasBranch || hasAuthor || hasWords) {
        metadataBar.classList.add('visible');

        if (modifiedEl) {
            if (hasModified) {
                modifiedEl.textContent = formatRelativeTime(modifiedAt);
                // Show full timestamp on hover
                const fullDate = new Date(modifiedAt);
                modifiedEl.parentElement.title = fullDate.toLocaleString();
                modifiedEl.parentElement.style.display = '';
            } else {
                modifiedEl.parentElement.style.display = 'none';
            }
        }

        if (readTimeEl) {
            if (hasReadTime) {
                readTimeEl.textContent = formatReadingTime(stats.reading_minutes || stats.reading_time_minutes);
                readTimeEl.parentElement.style.display = '';
            } else {
                readTimeEl.parentElement.style.display = 'none';
            }
        }

        if (branchEl && branchContainer) {
            if (hasBranch) {
                branchEl.textContent = gitBranch;
                branchContainer.style.display = '';

                // Make branch clickable if URL is available
                if (gitBranchUrl) {
                    branchContainer.classList.add('clickable');
                    branchContainer.title = `Open ${gitBranch} on GitHub`;
                    branchContainer.onclick = () => {
                        // Use Tauri opener plugin if available, otherwise fall back to window.open
                        if (window.__TAURI__ && window.__TAURI__.opener) {
                            window.__TAURI__.opener.openUrl(gitBranchUrl);
                        } else {
                            window.open(gitBranchUrl, '_blank');
                        }
                    };
                } else {
                    branchContainer.classList.remove('clickable');
                    branchContainer.title = 'Git branch';
                    branchContainer.onclick = null;
                }
            } else {
                branchContainer.style.display = 'none';
            }
        }

        if (authorEl) {
            if (hasAuthor) {
                authorEl.textContent = gitLastAuthor;
                authorEl.parentElement.style.display = '';
            } else {
                authorEl.parentElement.style.display = 'none';
            }
        }

        if (wordCountEl && wordCountContainer) {
            if (hasWords) {
                wordCountEl.textContent = formatWordCount(stats.words);
                wordCountContainer.style.display = '';
            } else {
                wordCountContainer.style.display = 'none';
            }
        }
    } else {
        metadataBar.classList.remove('visible');
    }
}

function triggerFileChangedAnimation() {
    const metadataBar = document.getElementById('file-metadata-bar');
    if (!metadataBar) return;

    // Remove the class first to reset animation if it's already running
    metadataBar.classList.remove('file-changed');

    // Force a reflow to restart the animation
    void metadataBar.offsetWidth;

    // Add the class to trigger animation
    metadataBar.classList.add('file-changed');

    // Remove the class after animation completes
    setTimeout(() => {
        metadataBar.classList.remove('file-changed');
    }, 1200);
}

// ============================================================================
// Fuzzy Finder
// ============================================================================

function fuzzyMatch(pattern, text) {
    if (!pattern) return { score: 0, positions: [] };

    const patternLower = pattern.toLowerCase();
    const textLower = text.toLowerCase();

    let patternIdx = 0;
    let score = 0;
    let positions = [];
    let lastMatchIdx = -1;

    for (let i = 0; i < text.length && patternIdx < pattern.length; i++) {
        if (textLower[i] === patternLower[patternIdx]) {
            positions.push(i);

            if (lastMatchIdx === i - 1) {
                score += 10;
            }

            if (i === 0 || /[\s\-_./]/.test(text[i - 1])) {
                score += 5;
            }

            if (text[i] === pattern[patternIdx]) {
                score += 2;
            }

            score += 1;
            lastMatchIdx = i;
            patternIdx++;
        }
    }

    if (patternIdx !== pattern.length) {
        return { score: 0, positions: [] };
    }

    score += Math.max(0, 50 - text.length);

    return { score, positions };
}

function highlightMatches(text, positions) {
    if (!positions.length) return escapeHtml(text);

    let result = '';
    let lastIdx = 0;

    for (const pos of positions) {
        result += escapeHtml(text.slice(lastIdx, pos));
        result += `<span class="fuzzy-match">${escapeHtml(text[pos])}</span>`;
        lastIdx = pos + 1;
    }
    result += escapeHtml(text.slice(lastIdx));

    return result;
}

function buildFuzzyIndex() {
    const content = document.getElementById('content');
    if (!content) return;

    fuzzySearchIndex = { headings: [], text: [] };

    const headings = content.querySelectorAll('h1, h2, h3, h4, h5, h6');
    headings.forEach((heading, index) => {
        const text = heading.textContent.replace(/^\s*🔗?\s*/, '').trim();
        const level = heading.tagName.toLowerCase();
        fuzzySearchIndex.headings.push({
            type: level.toUpperCase(),
            text: text,
            element: heading,
            id: heading.id || `heading-${index}`
        });
    });

    const textElements = content.querySelectorAll('p, li, pre, blockquote, td, th');
    textElements.forEach((el, index) => {
        const text = el.textContent.trim();
        if (text.length > 10 && text.length < 500) {
            let context = '';
            let prevEl = el.previousElementSibling;
            while (prevEl) {
                if (/^H[1-6]$/.test(prevEl.tagName)) {
                    context = prevEl.textContent.replace(/^\s*🔗?\s*/, '').trim();
                    break;
                }
                prevEl = prevEl.previousElementSibling;
            }

            fuzzySearchIndex.text.push({
                type: el.tagName === 'PRE' ? 'CODE' : 'TEXT',
                text: text.slice(0, 200),
                fullText: text,
                element: el,
                context: context
            });
        }
    });
}

function fuzzySearch(query) {
    const index = fuzzySearchMode === 'headings' ? fuzzySearchIndex.headings : fuzzySearchIndex.text;

    if (!query) {
        fuzzyResults = index.slice(0, 50).map(item => ({
            ...item,
            score: 100,
            positions: []
        }));
    } else {
        fuzzyResults = index
            .map(item => {
                const { score, positions } = fuzzyMatch(query, item.text);
                return { ...item, score, positions };
            })
            .filter(item => item.score > 0)
            .sort((a, b) => b.score - a.score)
            .slice(0, 50);
    }

    fuzzySelectedIndex = 0;
    renderFuzzyResults();
}

function renderFuzzyResults() {
    const resultsContainer = document.getElementById('fuzzy-results');
    const countSpan = document.getElementById('fuzzy-result-count');

    if (!fuzzyResults.length) {
        resultsContainer.innerHTML = '<div class="fuzzy-no-results">No results found</div>';
        countSpan.textContent = '0 results';
        return;
    }

    countSpan.textContent = `${fuzzyResults.length} result${fuzzyResults.length !== 1 ? 's' : ''}`;

    resultsContainer.innerHTML = fuzzyResults.map((result, index) => {
        const isSelected = index === fuzzySelectedIndex;
        const highlightedText = highlightMatches(result.text, result.positions);
        const context = result.context ? `<div class="fuzzy-result-context">in: ${escapeHtml(result.context)}</div>` : '';

        return `
            <div class="fuzzy-result-item${isSelected ? ' selected' : ''}" data-index="${index}">
                <div class="fuzzy-result-title">
                    <span class="fuzzy-result-type">${result.type}</span>
                    ${highlightedText}
                </div>
                ${context}
            </div>
        `;
    }).join('');

    const selectedItem = resultsContainer.querySelector('.fuzzy-result-item.selected');
    if (selectedItem) {
        selectedItem.scrollIntoView({ block: 'nearest' });
    }
}

function openFuzzyFinder() {
    buildFuzzyIndex();
    fuzzyFinderOpen = true;
    fuzzySelectedIndex = 0;

    const overlay = document.getElementById('fuzzy-finder');
    const input = document.getElementById('fuzzy-input');

    overlay.classList.add('visible');
    input.value = '';
    input.placeholder = fuzzySearchMode === 'headings' ? 'Search headings...' : 'Search full text...';

    fuzzySearch('');

    setTimeout(() => input.focus(), 50);
}

function closeFuzzyFinder() {
    fuzzyFinderOpen = false;
    const overlay = document.getElementById('fuzzy-finder');
    overlay.classList.remove('visible');
}

function toggleFuzzyMode() {
    fuzzySearchMode = fuzzySearchMode === 'headings' ? 'text' : 'headings';

    const headingsBtn = document.getElementById('fuzzy-mode-headings');
    const textBtn = document.getElementById('fuzzy-mode-text');
    const input = document.getElementById('fuzzy-input');

    headingsBtn.classList.toggle('active', fuzzySearchMode === 'headings');
    textBtn.classList.toggle('active', fuzzySearchMode === 'text');
    input.placeholder = fuzzySearchMode === 'headings' ? 'Search headings...' : 'Search full text...';

    fuzzySearch(input.value);
}

function selectFuzzyResult() {
    if (fuzzyResults.length === 0) return;

    const result = fuzzyResults[fuzzySelectedIndex];
    if (result && result.element) {
        closeFuzzyFinder();

        if (result.id) {
            navigateToHeading(result.id);
        } else {
            result.element.scrollIntoView({ behavior: 'smooth', block: 'center' });

            result.element.style.backgroundColor = '#fff8c5';
            result.element.style.transition = 'background-color 0.3s';
            setTimeout(() => {
                result.element.style.backgroundColor = '';
            }, 1500);
        }
    }
}

function handleFuzzyKeydown(e) {
    if (!fuzzyFinderOpen) return;

    switch (e.key) {
        case 'Escape':
            e.preventDefault();
            closeFuzzyFinder();
            break;

        case 'ArrowDown':
            e.preventDefault();
            if (fuzzySelectedIndex < fuzzyResults.length - 1) {
                fuzzySelectedIndex++;
                renderFuzzyResults();
            }
            break;

        case 'ArrowUp':
            e.preventDefault();
            if (fuzzySelectedIndex > 0) {
                fuzzySelectedIndex--;
                renderFuzzyResults();
            }
            break;

        case 'Enter':
            e.preventDefault();
            selectFuzzyResult();
            break;

        case 'Tab':
            e.preventDefault();
            toggleFuzzyMode();
            break;
    }
}

function initFuzzyFinder() {
    const overlay = document.getElementById('fuzzy-finder');
    const input = document.getElementById('fuzzy-input');
    const resultsContainer = document.getElementById('fuzzy-results');
    const headingsBtn = document.getElementById('fuzzy-mode-headings');
    const textBtn = document.getElementById('fuzzy-mode-text');

    document.addEventListener('keydown', (e) => {
        if ((e.ctrlKey || e.metaKey) && e.key === 'p') {
            e.preventDefault();
            if (fuzzyFinderOpen) {
                closeFuzzyFinder();
            } else {
                openFuzzyFinder();
            }
        }

        if (fuzzyFinderOpen) {
            handleFuzzyKeydown(e);
        }
    });

    overlay.addEventListener('click', (e) => {
        if (e.target === overlay) {
            closeFuzzyFinder();
        }
    });

    input.addEventListener('input', (e) => {
        fuzzySearch(e.target.value);
    });

    resultsContainer.addEventListener('click', (e) => {
        const item = e.target.closest('.fuzzy-result-item');
        if (item) {
            fuzzySelectedIndex = parseInt(item.dataset.index);
            selectFuzzyResult();
        }
    });

    headingsBtn.addEventListener('click', () => {
        if (fuzzySearchMode !== 'headings') {
            toggleFuzzyMode();
        }
    });

    textBtn.addEventListener('click', () => {
        if (fuzzySearchMode !== 'text') {
            toggleFuzzyMode();
        }
    });
}

// ============================================================================
// Content Update Handler
// ============================================================================

function handleContentUpdate(message, options = {}) {
    const content = document.getElementById('content');
    content.innerHTML = message.data || message.html;

    // Add spacer at the end for better scrolling
    const spacer = document.createElement('div');
    spacer.style.height = '100px';
    spacer.style.pointerEvents = 'none';
    content.appendChild(spacer);

    codeHighlight();
    renderLatex();
    renderMermaid();
    addHeadingAnchors();

    if (tocVisible) {
        generateTOC();
    }

    scrollToHash();

    if (message.file_path) {
        currentFilePath = message.file_path;
        addToRecentFiles(currentFilePath);
        if (options.onFileClick) {
            renderRecentFiles(options.onFileClick);
        } else {
            renderRecentFiles();
        }
        updateFilePathBar(currentFilePath, message.git_root);
        document.title = getFileBasename(currentFilePath) + ' - Markdown Preview';

        if (options.onFileOpened) {
            options.onFileOpened(currentFilePath);
        }
    }

    if (message.stats) {
        updateDocumentStats(message.stats);
    }

    if (message.should_focus) {
        window.focus();
    }
}

// ============================================================================
// Initialize Core UI
// ============================================================================

function initCoreUI(options = {}) {
    // Restore saved preferences
    // Note: Line Numbers feature removed from UI (was buggy)
    // TOC and Theme controlled via menu, not sidebar

    const savedTOCMode = localStorage.getItem('tocMode');
    let tocMode = 'off';
    if (savedTOCMode) {
        tocMode = savedTOCMode;
    } else if (window.innerWidth >= 1440) {
        tocMode = 'right';
    }

    const savedFont = localStorage.getItem('fontFamily') || 'default';
    fontFamily = savedFont;
    document.getElementById('font-family').value = savedFont;

    const savedReaderMode = localStorage.getItem('readerMode') === 'true';
    readerMode = savedReaderMode;
    document.getElementById('reader-mode').value = savedReaderMode ? 'on' : 'off';

    const savedTheme = localStorage.getItem('theme') || 'auto';
    currentTheme = savedTheme;

    const savedZoom = parseInt(localStorage.getItem('zoomLevel')) || 100;
    zoomLevel = savedZoom;
    updateZoomDisplay();

    // Set up event listeners
    document.getElementById('font-family').addEventListener('change', (e) => {
        changeFontFamily(e.target.value);
    });

    document.getElementById('reader-mode').addEventListener('change', (e) => {
        toggleReaderMode(e.target.value === 'on');
    });

    // Zoom controls
    const zoomOutBtn = document.getElementById('zoom-out-btn');
    const zoomInBtn = document.getElementById('zoom-in-btn');
    const zoomResetBtn = document.getElementById('zoom-reset-btn');

    if (zoomOutBtn) zoomOutBtn.addEventListener('click', zoomOut);
    if (zoomInBtn) zoomInBtn.addEventListener('click', zoomIn);
    if (zoomResetBtn) zoomResetBtn.addEventListener('click', resetZoom);

    document.getElementById('file-path-bar').addEventListener('click', () => {
        if (currentFilePath) {
            copyToClipboard(currentFilePath);
        }
    });

    setupTOCResize();
    setupSidebarResize();
    setupRecentFilesResize();

    window.addEventListener('hashchange', () => {
        const hash = window.location.hash.slice(1);
        if (hash) {
            const element = document.getElementById(hash);
            if (element) {
                element.scrollIntoView({ behavior: 'smooth', block: 'start' });
            }
        }
    });

    // Initial rendering
    codeHighlight();
    renderMermaid();

    if (tocMode !== 'off') {
        toggleTOC(tocMode);
    }

    changeFontFamily(fontFamily);

    if (readerMode) {
        toggleReaderMode(true);
    }

    changeTheme(currentTheme);

    // Apply saved zoom level
    applyZoom(zoomLevel);

    renderRecentFiles(options.onFileClick);
    initFuzzyFinder();
}

// Export for use in platform-specific modules (browser global scope)
// These are needed because core.js uses 'let' which doesn't add to window
window.MarkdownPreviewCore = {
    // Getters/setters for state
    getCurrentFilePath: () => currentFilePath,
    setCurrentFilePath: (path) => { currentFilePath = path; },
    getZoomLevel: () => zoomLevel,

    // Core functions
    initCoreUI,
    handleContentUpdate,
    codeHighlight,
    renderMermaid,
    renderLatex,
    generateTOC,
    updateFilePathBar,
    updateDocumentStats,
    addHeadingAnchors,
    showToast,
    copyToClipboard,
    getFileBasename,
    renderRecentFiles,
    addToRecentFiles,

    // Zoom functions
    zoomIn,
    zoomOut,
    resetZoom,
    applyZoom
};

// Also expose commonly used functions directly for convenience
window.initCoreUI = initCoreUI;
window.handleContentUpdate = handleContentUpdate;
window.codeHighlight = codeHighlight;
window.renderMermaid = renderMermaid;
window.renderLatex = renderLatex;
window.generateTOC = generateTOC;
window.updateFilePathBar = updateFilePathBar;
window.updateDocumentStats = updateDocumentStats;
window.updateFileMetadata = updateFileMetadata;
window.triggerFileChangedAnimation = triggerFileChangedAnimation;
window.addHeadingAnchors = addHeadingAnchors;
window.showToast = showToast;
window.copyToClipboard = copyToClipboard;
window.getFileBasename = getFileBasename;
window.renderRecentFiles = renderRecentFiles;
window.addToRecentFiles = addToRecentFiles;
window.zoomIn = zoomIn;
window.zoomOut = zoomOut;
window.resetZoom = resetZoom;
window.applyZoom = applyZoom;
