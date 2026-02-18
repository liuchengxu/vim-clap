// Core - Shared UI functionality for markdown preview

// ============================================================================
// State Variables
// ============================================================================

let tocVisible = false;
let fontFamily = 'default';
let readerMode = false;
let contentWidth = 'default'; // narrow, default, wide, full
let currentFilePath = '';
let currentTheme = 'auto';
let zoomLevel = 100; // Percentage: 50-200

// New feature state
let presentationMode = false;
let presentationSlides = [];
let currentSlide = 0;

// Diff overlay state
let diffOverlayVisible = false;
let currentDiff = null;

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

// DOMPurify config: preserve MathML (KaTeX) and data-line (scroll sync)
const DOMPURIFY_CONFIG = {
    ADD_TAGS: ['math', 'semantics', 'mrow', 'mi', 'mo', 'mn', 'msup', 'msub',
               'mfrac', 'msqrt', 'mover', 'munder', 'mtable', 'mtr', 'mtd',
               'annotation', 'annotation-xml'],
    ADD_ATTR: ['encoding', 'data-line'],
};

/**
 * Sanitize HTML content using DOMPurify (fail-closed).
 */
function sanitizeHtml(html) {
    if (typeof DOMPurify === 'undefined') {
        return '<div class="error" style="padding: 40px; text-align: center; color: #cf222e;">'
             + '<h2>Content sanitizer not loaded</h2>'
             + '<p>DOMPurify failed to load. Cannot safely render content.</p></div>';
    }
    return DOMPurify.sanitize(html, DOMPURIFY_CONFIG);
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

        // Use PDF TOC if PDF viewer is active, otherwise use markdown TOC
        if (window.PdfViewer && window.PdfViewer.isActive()) {
            window.PdfViewer.refreshTOC();
        } else {
            generateTOC();
        }

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

function changeContentWidth(width) {
    contentWidth = width;
    const content = document.getElementById('content');
    if (!content) return;

    // Remove all width classes
    content.classList.remove('content-width-narrow', 'content-width-wide', 'content-width-full');

    // Add the selected width class (default uses the base max-width: 980px)
    if (width !== 'default') {
        content.classList.add('content-width-' + width);
    }

    localStorage.setItem('contentWidth', width);

    // If PDF viewer is active, recalculate its base scale after a brief delay
    // to allow the CSS width change to take effect
    if (window.PdfViewer && window.PdfViewer.isActive()) {
        setTimeout(() => {
            window.PdfViewer.recalculateBaseScale();
        }, 50);
    }
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
    // Don't reorder if already in the list
    if (recentFiles.some(f => f.path === filePath)) return;

    recentFiles.unshift({
        path: filePath,
        timestamp: Date.now()
    });
    recentFiles = recentFiles.slice(0, 10);
    localStorage.setItem('recentFiles', JSON.stringify(recentFiles));
}

function removeFromRecentFiles(filePath) {
    if (!filePath) return;
    let recentFiles = getRecentFiles();
    recentFiles = recentFiles.filter(f => f.path !== filePath);
    localStorage.setItem('recentFiles', JSON.stringify(recentFiles));
}


// Tooltips (symbol, link, path) are in tooltips.js


/**
 * Get file type icon SVG based on file extension
 */
function getFileTypeIcon(filePath) {
    const ext = filePath.split('.').pop()?.toLowerCase() || '';

    if (ext === 'pdf') {
        // PDF icon
        return `<svg class="recent-file-icon recent-file-icon-pdf" viewBox="0 0 16 16" fill="currentColor">
            <path d="M14.5 2H1.5C.67 2 0 2.67 0 3.5v9c0 .83.67 1.5 1.5 1.5h13c.83 0 1.5-.67 1.5-1.5v-9c0-.83-.67-1.5-1.5-1.5zM1.5 3h13c.28 0 .5.22.5.5V5H1V3.5c0-.28.22-.5.5-.5zM1 12.5V6h14v6.5c0 .28-.22.5-.5.5h-13c-.28 0-.5-.22-.5-.5z"/>
            <path d="M4 8h1.5c.55 0 1 .45 1 1s-.45 1-1 1H4.5v1H4V8zm.5.5v1h1c.28 0 .5-.22.5-.5s-.22-.5-.5-.5h-1zM7 8h1.2c.77 0 1.3.53 1.3 1.5S9 11 8.2 11H7V8zm.5.5v2h.7c.5 0 .8-.33.8-1s-.3-1-.8-1h-.7zM10 8h2.5v.5H10.5v.75h1.5v.5h-1.5V11H10V8z"/>
        </svg>`;
    } else {
        // Markdown icon (default)
        return `<svg class="recent-file-icon recent-file-icon-md" viewBox="0 0 16 16" fill="currentColor">
            <path fill-rule="evenodd" d="M14.85 3H1.15C.52 3 0 3.52 0 4.15v7.7C0 12.48.52 13 1.15 13h13.7c.63 0 1.15-.52 1.15-1.15v-7.7C16 3.52 15.48 3 14.85 3zM9 11H7V8L5.5 9.92 4 8v3H2V5h2l1.5 2L7 5h2v6zm2.99.5L9.5 8H11V5h2v3h1.5l-2.51 3.5z"/>
        </svg>`;
    }
}

function renderRecentFiles(onFileClick, onRemove) {
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

        // Create header row with icon and name
        const headerRow = document.createElement('div');
        headerRow.className = 'recent-file-header';

        const iconWrapper = document.createElement('span');
        iconWrapper.className = 'recent-file-icon-wrapper';
        iconWrapper.innerHTML = getFileTypeIcon(file.path);

        const nameElement = document.createElement('span');
        nameElement.className = 'recent-file-name';
        nameElement.textContent = getFileBasename(file.path);

        headerRow.appendChild(iconWrapper);
        headerRow.appendChild(nameElement);

        const pathElement = document.createElement('div');
        pathElement.className = 'recent-file-path';
        pathElement.textContent = getFileDirectory(file.path);

        item.appendChild(headerRow);
        item.appendChild(pathElement);

        // Add remove button
        const removeBtn = document.createElement('button');
        removeBtn.className = 'recent-file-remove-btn';
        removeBtn.title = 'Remove from list';
        removeBtn.innerHTML = '×';
        removeBtn.onclick = (e) => {
            e.preventDefault();
            e.stopPropagation();
            // Remove from localStorage
            removeFromRecentFiles(file.path);
            // Call backend removal if provided
            if (onRemove) {
                onRemove(file.path);
            }
            // Re-render the list
            renderRecentFiles(onFileClick, onRemove);
            showToast('Removed from recent files');
        };
        item.appendChild(removeBtn);

        // Custom tooltip on hover with title and modification time
        // Skip tooltip for the currently previewed file (user is already viewing it)
        item.addEventListener('mouseenter', () => {
            if (file.path === currentFilePath) {
                return;
            }
            tooltipHoveredElement = item;
            if (tooltipTimeout) clearTimeout(tooltipTimeout);
            tooltipTimeout = setTimeout(async () => {
                const previewInfo = await getFilePreviewInfo(file.path);
                // Guard: only show if mouse is still over this item
                if (tooltipHoveredElement === item) {
                    showPathTooltip(item, file.path, previewInfo);
                }
            }, 400);
        });

        item.addEventListener('mouseleave', () => {
            tooltipHoveredElement = null;
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
            pathBar.innerHTML = `<span class="git-root">${escapeHtml(gitRootName)}</span><span class="path-separator">/</span><span class="relative-path">${escapeHtml(relativePath.replace(/^\//, ''))}</span>`;
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
// Auto-scroll
// ============================================================================

const autoScroll = {
    active: false,
    speed: 1,        // pixels per tick
    intervalId: null,
    pausedByUser: false,
    TICK_MS: 30,     // ~33 fps
    MIN_SPEED: 0.5,
    MAX_SPEED: 5,
    SPEED_STEP: 0.5,
};

function toggleAutoScroll() {
    if (autoScroll.active) {
        stopAutoScroll();
    } else {
        startAutoScroll();
    }
}

function startAutoScroll() {
    const container = document.getElementById('main-content');
    if (!container) return;

    autoScroll.active = true;
    autoScroll.pausedByUser = false;
    updateAutoScrollUI();

    autoScroll.intervalId = setInterval(() => {
        // Stop if we've reached the bottom
        const atBottom = container.scrollTop + container.clientHeight >= container.scrollHeight - 1;
        if (atBottom) {
            stopAutoScroll();
            return;
        }
        container.scrollTop += autoScroll.speed;
    }, autoScroll.TICK_MS);

    // Listen for user scroll to auto-pause
    container.addEventListener('wheel', onUserScroll);
    container.addEventListener('mousedown', onUserScroll);
    container.addEventListener('touchstart', onUserScroll);
}

function stopAutoScroll() {
    autoScroll.active = false;
    if (autoScroll.intervalId) {
        clearInterval(autoScroll.intervalId);
        autoScroll.intervalId = null;
    }
    const container = document.getElementById('main-content');
    if (container) {
        container.removeEventListener('wheel', onUserScroll);
        container.removeEventListener('mousedown', onUserScroll);
        container.removeEventListener('touchstart', onUserScroll);
    }
    updateAutoScrollUI();
}

function onUserScroll() {
    if (autoScroll.active) {
        stopAutoScroll();
        autoScroll.pausedByUser = true;
    }
}

function adjustAutoScrollSpeed(delta) {
    const newSpeed = autoScroll.speed + delta;
    autoScroll.speed = Math.max(autoScroll.MIN_SPEED, Math.min(autoScroll.MAX_SPEED, newSpeed));
    updateAutoScrollUI();
}

function updateAutoScrollUI() {
    // Floating pill
    const pill = document.getElementById('autoscroll-float');
    const speedEl = document.getElementById('autoscroll-float-speed');
    if (pill) {
        if (autoScroll.active) {
            pill.classList.add('visible');
        } else {
            pill.classList.remove('visible');
        }
    }
    if (speedEl) {
        speedEl.textContent = autoScroll.speed.toFixed(1) + 'x';
    }
}

// Wire up click handlers once the DOM is ready
function initAutoScroll() {
    // Metadata bar button — starts auto-scroll
    const metaBtn = document.getElementById('metadata-autoscroll');
    if (metaBtn) {
        metaBtn.addEventListener('click', toggleAutoScroll);
    }

    // Floating pill buttons
    const pauseBtn = document.getElementById('autoscroll-float-pause');
    if (pauseBtn) {
        pauseBtn.addEventListener('click', () => stopAutoScroll());
    }
    const slowerBtn = document.getElementById('autoscroll-float-slower');
    if (slowerBtn) {
        slowerBtn.addEventListener('click', () => adjustAutoScrollSpeed(-autoScroll.SPEED_STEP));
    }
    const fasterBtn = document.getElementById('autoscroll-float-faster');
    if (fasterBtn) {
        fasterBtn.addEventListener('click', () => adjustAutoScrollSpeed(autoScroll.SPEED_STEP));
    }
}

// Initialize when DOM is ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initAutoScroll);
} else {
    initAutoScroll();
}

// Fuzzy Finder is in search.js

// ============================================================================
// Content Update Handler
// ============================================================================

function handleContentUpdate(message, options = {}) {
    const content = document.getElementById('content');
    content.innerHTML = sanitizeHtml(message.data || message.html);

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
        if (options.onFileClick || options.onRemove) {
            renderRecentFiles(options.onFileClick, options.onRemove);
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

    // Apply new feature enhancements to loaded content
    addCodeCopyButtons();
    setupImageLightbox();
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

    const savedContentWidth = localStorage.getItem('contentWidth') || 'default';
    contentWidth = savedContentWidth;
    const contentWidthEl = document.getElementById('content-width');
    if (contentWidthEl) {
        contentWidthEl.value = savedContentWidth;
    }

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

    const contentWidthSelect = document.getElementById('content-width');
    if (contentWidthSelect) {
        contentWidthSelect.addEventListener('change', (e) => {
            changeContentWidth(e.target.value);
        });
    }

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

    // Apply saved content width
    if (contentWidth !== 'default') {
        changeContentWidth(contentWidth);
    }

    changeTheme(currentTheme);

    // Apply saved zoom level
    applyZoom(zoomLevel);

    renderRecentFiles(options.onFileClick, options.onRemove);
    initToolTabs();
    initFuzzyFinder();

    // Initialize new features
    setupReadingProgress();
    setupPresentationMode();
    addCodeCopyButtons();

    // Set up tooltips on content area
    const content = document.querySelector('.markdown-body');
    if (content) {
        setupSymbolTooltips(content);
        setupLinkTooltips(content);
    }
}

// ============================================================================
// Code Block Copy Button
// ============================================================================

function addCodeCopyButtons() {
    document.querySelectorAll('.markdown-body pre > code').forEach(block => {
        const pre = block.parentElement;
        if (pre.querySelector('.code-copy-btn')) return;

        const btn = document.createElement('button');
        btn.className = 'code-copy-btn';
        btn.title = 'Copy code';
        btn.innerHTML = `<svg viewBox="0 0 16 16" fill="currentColor">
            <path d="M0 6.75C0 5.784.784 5 1.75 5h1.5a.75.75 0 0 1 0 1.5h-1.5a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-1.5a.75.75 0 0 1 1.5 0v1.5A1.75 1.75 0 0 1 9.25 16h-7.5A1.75 1.75 0 0 1 0 14.25Z"/>
            <path d="M5 1.75C5 .784 5.784 0 6.75 0h7.5C15.216 0 16 .784 16 1.75v7.5A1.75 1.75 0 0 1 14.25 11h-7.5A1.75 1.75 0 0 1 5 9.25Zm1.75-.25a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-7.5a.25.25 0 0 0-.25-.25Z"/>
        </svg>`;

        btn.onclick = async (e) => {
            e.stopPropagation();
            const code = block.textContent;
            try {
                await navigator.clipboard.writeText(code);
                btn.classList.add('copied');
                btn.innerHTML = `<svg viewBox="0 0 16 16" fill="currentColor">
                    <path d="M13.78 4.22a.75.75 0 0 1 0 1.06l-7.25 7.25a.75.75 0 0 1-1.06 0L2.22 9.28a.751.751 0 0 1 .018-1.042.751.751 0 0 1 1.042-.018L6 10.94l6.72-6.72a.75.75 0 0 1 1.06 0Z"/>
                </svg>`;
                setTimeout(() => {
                    btn.classList.remove('copied');
                    btn.innerHTML = `<svg viewBox="0 0 16 16" fill="currentColor">
                        <path d="M0 6.75C0 5.784.784 5 1.75 5h1.5a.75.75 0 0 1 0 1.5h-1.5a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-1.5a.75.75 0 0 1 1.5 0v1.5A1.75 1.75 0 0 1 9.25 16h-7.5A1.75 1.75 0 0 1 0 14.25Z"/>
                        <path d="M5 1.75C5 .784 5.784 0 6.75 0h7.5C15.216 0 16 .784 16 1.75v7.5A1.75 1.75 0 0 1 14.25 11h-7.5A1.75 1.75 0 0 1 5 9.25Zm1.75-.25a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-7.5a.25.25 0 0 0-.25-.25Z"/>
                    </svg>`;
                }, 2000);
            } catch (err) {
                console.error('Failed to copy code:', err);
            }
        };

        pre.appendChild(btn);
    });
}

// ============================================================================
// Reading Progress Bar
// ============================================================================

function setupReadingProgress() {
    const progressBar = document.getElementById('reading-progress');
    const mainContent = document.getElementById('main-content');
    if (!progressBar || !mainContent) return;

    mainContent.addEventListener('scroll', () => {
        const scrollHeight = mainContent.scrollHeight - mainContent.clientHeight;
        if (scrollHeight <= 0) {
            progressBar.style.width = '0%';
            return;
        }
        const progress = (mainContent.scrollTop / scrollHeight) * 100;
        progressBar.style.width = `${Math.min(100, Math.max(0, progress))}%`;
    });
}

// ============================================================================
// Image Lightbox
// ============================================================================

function setupImageLightbox() {
    const overlay = document.getElementById('lightbox-overlay');
    const lightboxImg = document.getElementById('lightbox-img');
    if (!overlay || !lightboxImg) return;

    const closeBtn = overlay.querySelector('.lightbox-close');

    document.querySelectorAll('.markdown-body img').forEach(img => {
        if (img.naturalWidth < 50 || img.naturalHeight < 50) {
            img.classList.add('lightbox-disabled');
            return;
        }
        if (img.closest('.mermaid') || img.closest('svg')) {
            img.classList.add('lightbox-disabled');
            return;
        }
        if (img.dataset.lightboxSetup) return;
        img.dataset.lightboxSetup = 'true';

        img.onclick = (e) => {
            e.preventDefault();
            lightboxImg.src = img.src;
            lightboxImg.alt = img.alt || 'Full-size image';
            overlay.classList.add('active');
        };
    });

    if (!overlay.dataset.setupDone) {
        overlay.dataset.setupDone = 'true';
        overlay.onclick = (e) => {
            if (e.target === overlay || e.target === lightboxImg) {
                overlay.classList.remove('active');
            }
        };
        if (closeBtn) {
            closeBtn.onclick = () => overlay.classList.remove('active');
        }
    }
}

function closeLightbox() {
    const overlay = document.getElementById('lightbox-overlay');
    if (overlay) overlay.classList.remove('active');
}

// ============================================================================
// Presentation Mode
// ============================================================================

function enterPresentationMode() {
    const content = document.getElementById('content');
    if (!content) return;

    const tempDiv = document.createElement('div');
    tempDiv.innerHTML = content.innerHTML;

    const slides = [];
    let currentSlideContent = [];

    Array.from(tempDiv.children).forEach(child => {
        if (child.tagName === 'HR') {
            if (currentSlideContent.length > 0) {
                slides.push(currentSlideContent.map(el => el.outerHTML).join(''));
                currentSlideContent = [];
            }
        } else {
            currentSlideContent.push(child);
        }
    });

    if (currentSlideContent.length > 0) {
        slides.push(currentSlideContent.map(el => el.outerHTML).join(''));
    }

    if (slides.length <= 1) {
        slides.length = 0;
        currentSlideContent = [];
        Array.from(tempDiv.children).forEach(child => {
            if (child.tagName === 'H1' && currentSlideContent.length > 0) {
                slides.push(currentSlideContent.map(el => el.outerHTML).join(''));
                currentSlideContent = [child];
            } else {
                currentSlideContent.push(child);
            }
        });
        if (currentSlideContent.length > 0) {
            slides.push(currentSlideContent.map(el => el.outerHTML).join(''));
        }
    }

    if (slides.length === 0) slides.push(content.innerHTML);

    presentationSlides = slides;
    currentSlide = 0;

    const slidesContainer = document.getElementById('presentation-slides');
    if (!slidesContainer) return;

    slidesContainer.innerHTML = slides.map((slideContent, index) =>
        `<div class="presentation-slide${index === 0 ? ' active' : ''}" data-index="${index}">
            <div class="slide-content">${slideContent}</div>
        </div>`
    ).join('');

    updateSlideCounter();
    presentationMode = true;
    document.body.classList.add('presentation-mode');

    if (typeof hljs !== 'undefined') {
        slidesContainer.querySelectorAll('pre code').forEach(el => hljs.highlightElement(el));
    }
}

function exitPresentationMode() {
    presentationMode = false;
    document.body.classList.remove('presentation-mode');
    presentationSlides = [];
    currentSlide = 0;
}

function nextSlide() {
    if (currentSlide < presentationSlides.length - 1) goToSlide(currentSlide + 1);
}

function prevSlide() {
    if (currentSlide > 0) goToSlide(currentSlide - 1);
}

function goToSlide(index) {
    if (index < 0 || index >= presentationSlides.length) return;
    document.querySelectorAll('.presentation-slide').forEach((slide, i) => {
        slide.classList.toggle('active', i === index);
    });
    currentSlide = index;
    updateSlideCounter();
}

function updateSlideCounter() {
    const currentEl = document.getElementById('pres-current');
    const totalEl = document.getElementById('pres-total');
    const prevBtn = document.getElementById('pres-prev');
    const nextBtn = document.getElementById('pres-next');

    if (currentEl) currentEl.textContent = currentSlide + 1;
    if (totalEl) totalEl.textContent = presentationSlides.length;
    if (prevBtn) prevBtn.disabled = currentSlide === 0;
    if (nextBtn) nextBtn.disabled = currentSlide === presentationSlides.length - 1;
}

function setupPresentationMode() {
    const prevBtn = document.getElementById('pres-prev');
    const nextBtn = document.getElementById('pres-next');
    const exitBtn = document.getElementById('pres-exit');

    if (prevBtn) prevBtn.onclick = prevSlide;
    if (nextBtn) nextBtn.onclick = nextSlide;
    if (exitBtn) exitBtn.onclick = exitPresentationMode;

    document.addEventListener('keydown', (e) => {
        if (!presentationMode) return;
        switch (e.key) {
            case 'ArrowRight':
            case ' ':
            case 'PageDown':
                e.preventDefault();
                nextSlide();
                break;
            case 'ArrowLeft':
            case 'PageUp':
                e.preventDefault();
                prevSlide();
                break;
            case 'Escape':
                e.preventDefault();
                exitPresentationMode();
                break;
        }
    });
}

// Diff Overlay is in diff.js

// ============================================================================
// Keyboard Shortcut Registry
// ============================================================================

const registeredShortcuts = [];

/**
 * Register a keyboard shortcut.
 * @param {string} key - Single character, compared lowercase.
 * @param {object} modifiers - { ctrl: bool, shift: bool } — ctrl means ctrlKey || metaKey.
 * @param {function} handler - Handler to call when shortcut matches.
 * @param {object} [options] - Optional config.
 * @param {function} [options.when] - Guard function; skip if returns false.
 */
function registerShortcut(key, modifiers, handler, options = {}) {
    registeredShortcuts.push({ key: key.toLowerCase(), modifiers, handler, options });
}

/**
 * Check if a keyboard event matches a shortcut definition.
 */
function matchesShortcut(e, s) {
    const ctrlMatch = (e.ctrlKey || e.metaKey) === !!s.modifiers.ctrl;
    const shiftMatch = e.shiftKey === !!s.modifiers.shift;
    const keyMatch = e.key.toLowerCase() === s.key;
    return ctrlMatch && shiftMatch && keyMatch;
}

/**
 * Set up the global keydown listener that dispatches registered shortcuts.
 */
function setupShortcutListener() {
    document.addEventListener('keydown', (e) => {
        for (const s of registeredShortcuts) {
            if (!matchesShortcut(e, s)) continue;
            if (s.options.when && !s.options.when()) continue;
            e.preventDefault();
            s.handler(e);
            return;
        }
    });
}

// ============================================================================
// Tool Navigation
// ============================================================================

let currentTool = localStorage.getItem('currentTool') || 'preview';
let sidebarCollapsed = localStorage.getItem('sidebarCollapsed') === 'true';

function setSidebarCollapsed(collapsed) {
    sidebarCollapsed = collapsed;
    const sidebar = document.getElementById('sidebar');
    if (!sidebar) return;

    if (collapsed) {
        sidebar.classList.add('collapsed');
    } else {
        sidebar.classList.remove('collapsed');
        // Restore saved width
        const savedWidth = localStorage.getItem('sidebarWidth');
        if (savedWidth) {
            const width = parseInt(savedWidth);
            if (width >= 180 && width <= 500) {
                sidebar.style.width = width + 'px';
            }
        }
    }
    localStorage.setItem('sidebarCollapsed', collapsed);
}

function switchTool(toolName) {
    const sidebar = document.getElementById('sidebar');

    // Clicking the already-active tool ensures sidebar is visible
    if (toolName === currentTool && sidebar) {
        if (sidebarCollapsed) {
            setSidebarCollapsed(false);
        }
        return;
    }

    currentTool = toolName;
    localStorage.setItem('currentTool', toolName);

    // If sidebar is collapsed, expand it when switching to a different tool
    if (sidebarCollapsed) {
        setSidebarCollapsed(false);
    }

    // Update activity icon active states + aria-pressed
    document.querySelectorAll('.activity-icon[data-tool]').forEach(icon => {
        const isActive = icon.dataset.tool === toolName;
        icon.classList.toggle('active', isActive);
        icon.setAttribute('aria-pressed', isActive ? 'true' : 'false');
    });

    // Toggle sidebar and main sections by convention: #tool-{name}-sidebar, #tool-{name}-main
    document.querySelectorAll('[id^="tool-"][id$="-sidebar"]').forEach(el => {
        const tool = el.id.replace('tool-', '').replace('-sidebar', '');
        el.style.display = tool === toolName ? '' : 'none';
    });
    document.querySelectorAll('[id^="tool-"][id$="-main"]').forEach(el => {
        const tool = el.id.replace('tool-', '').replace('-main', '');
        el.style.display = tool === toolName ? '' : 'none';
    });

    // Toggle preview-specific UI elements
    const tocPanel = document.getElementById('toc-panel');
    if (toolName === 'preview') {
        const savedTOC = localStorage.getItem('tocMode');
        if (savedTOC && savedTOC !== 'off' && tocPanel) {
            tocPanel.style.display = '';
        }
    } else {
        if (tocPanel) tocPanel.style.display = 'none';
    }
}

function initToolTabs() {
    // Bind activity icon click handlers
    document.querySelectorAll('.activity-icon[data-tool]').forEach(icon => {
        icon.addEventListener('click', () => {
            switchTool(icon.dataset.tool);
        });
    });

    // Wire settings button
    const settingsBtn = document.getElementById('activity-settings-btn');
    if (settingsBtn) {
        settingsBtn.addEventListener('click', () => {
            if (typeof window.showSettingsDialog === 'function') {
                window.showSettingsDialog();
            }
        });
    }

    // Restore collapsed state
    if (sidebarCollapsed) {
        setSidebarCollapsed(true);
    }

    // Apply saved tool state — temporarily reset currentTool so switchTool
    // runs the full show/hide logic instead of hitting the early return.
    if (currentTool !== 'preview') {
        const savedTool = currentTool;
        currentTool = null;
        switchTool(savedTool);
    }
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
    applyZoom,

    // Shortcut registry
    registerShortcut,
    setupShortcutListener,

    // Sanitization
    sanitizeHtml,

    // Tool navigation
    switchTool,
    initToolTabs,
    setSidebarCollapsed,
    getCurrentTool: () => currentTool,
    isSidebarCollapsed: () => sidebarCollapsed
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
window.removeFromRecentFiles = removeFromRecentFiles;
window.zoomIn = zoomIn;
window.zoomOut = zoomOut;
window.resetZoom = resetZoom;
window.applyZoom = applyZoom;
window.changeContentWidth = changeContentWidth;
window.toggleTOC = toggleTOC;
window.changeTheme = changeTheme;
window.addCodeCopyButtons = addCodeCopyButtons;
window.setupImageLightbox = setupImageLightbox;
window.closeLightbox = closeLightbox;
window.enterPresentationMode = enterPresentationMode;
window.exitPresentationMode = exitPresentationMode;
window.registerShortcut = registerShortcut;
window.setupShortcutListener = setupShortcutListener;
window.sanitizeHtml = sanitizeHtml;
window.switchTool = switchTool;
window.initToolTabs = initToolTabs;
window.setSidebarCollapsed = setSidebarCollapsed;
