// Markdown Preview Core - Shared UI functionality
// This module contains all UI-related functions shared between vim-clap (WebSocket) and Tauri modes

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
    recentFiles = recentFiles.filter(f => f.path !== filePath);
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

// Custom tooltip for recent files
let pathTooltip = null;
let tooltipTimeout = null;
let tooltipHoveredElement = null;  // tracks which item the mouse is currently over
// Cache for markdown titles to avoid repeated file reads
const markdownTitleCache = new Map();

function createPathTooltip() {
    if (pathTooltip) return pathTooltip;

    pathTooltip = document.createElement('div');
    pathTooltip.className = 'path-tooltip';
    pathTooltip.innerHTML = '<div class="path-tooltip-content"></div>';
    document.body.appendChild(pathTooltip);

    return pathTooltip;
}

// Check if a path is a markdown file
function isMarkdownFile(path) {
    const ext = path.split('.').pop().toLowerCase();
    return ['md', 'markdown', 'mdown', 'mkdn', 'mkd'].includes(ext);
}

// Fetch file preview info with caching (Tauri only)
// Returns { title, modified_at } for the tooltip
async function getFilePreviewInfo(path) {
    // Return cached value if available
    if (markdownTitleCache.has(path)) {
        return markdownTitleCache.get(path);
    }

    try {
        // Check if we're in Tauri environment
        if (window.__TAURI__ && window.__TAURI__.core) {
            const info = await window.__TAURI__.core.invoke('get_file_preview_info', { path });
            markdownTitleCache.set(path, info);
            return info;
        }
    } catch (e) {
        console.error('Failed to get file preview info:', e);
    }

    return { title: null, modified_at: null };
}

function showPathTooltip(element, fullPath, previewInfo = {}) {
    const tooltip = createPathTooltip();
    const content = tooltip.querySelector('.path-tooltip-content');

    const { title, digest, modified_at } = previewInfo;

    // Format path with segments
    const segments = fullPath.split('/').filter(s => s);
    const formatted = '/' + segments.map((seg, i) => {
        const isLast = i === segments.length - 1;
        return isLast ? `<span class="path-tooltip-file">${seg}</span>` : seg;
    }).join('<span class="path-tooltip-sep">/</span>');

    // Build tooltip content with optional title
    let html = '';
    if (title) {
        html += `<div class="path-tooltip-title">${escapeHtml(title)}</div>`;
    }
    html += formatted;

    // Add modification time if available
    if (modified_at) {
        const relativeTime = formatRelativeTime(modified_at);
        const fullDate = new Date(modified_at).toLocaleString();
        html += `<div class="path-tooltip-modified" title="${fullDate}">Modified ${relativeTime}</div>`;
    }

    // Add digest (structural preview) if available
    if (digest) {
        const digestLines = digest.split('\n').map(line => {
            if (line.startsWith('# ')) {
                return `<div class="path-tooltip-digest-heading">${escapeHtml(line.slice(2))}</div>`;
            }
            return `<div class="path-tooltip-digest-text">${escapeHtml(line)}</div>`;
        }).join('');
        html += `<div class="path-tooltip-digest">${digestLines}</div>`;
    }

    content.innerHTML = html;

    // Position tooltip to the right of the element (offset to avoid sidebar border)
    const rect = element.getBoundingClientRect();
    tooltip.style.left = `${rect.right + 20}px`;
    tooltip.style.top = `${rect.top}px`;
    tooltip.classList.add('visible');
}

function hidePathTooltip() {
    if (pathTooltip) {
        pathTooltip.classList.remove('visible');
    }
}

// Greek letter tooltip for unfamiliar characters
const GREEK_LETTERS = {
    // Lowercase
    'α': 'alpha (AL-fuh)',
    'β': 'beta (BAY-tuh)',
    'γ': 'gamma (GAM-uh)',
    'δ': 'delta (DEL-tuh)',
    'ε': 'epsilon (EP-sih-lon)',
    'ζ': 'zeta (ZAY-tuh)',
    'η': 'eta (AY-tuh)',
    'θ': 'theta (THAY-tuh)',
    'ι': 'iota (eye-OH-tuh)',
    'κ': 'kappa (KAP-uh)',
    'λ': 'lambda (LAM-duh)',
    'μ': 'mu (MYOO)',
    'ν': 'nu (NOO)',
    'ξ': 'xi (KSEE / ZYE)',
    'ο': 'omicron (OM-ih-kron)',
    'π': 'pi (PIE)',
    'ρ': 'rho (ROH)',
    'σ': 'sigma (SIG-muh)',
    'ς': 'sigma final (SIG-muh)',
    'τ': 'tau (TAW / TOW)',
    'υ': 'upsilon (OOP-sih-lon)',
    'φ': 'phi (FYE / FEE)',
    'χ': 'chi (KYE / KHEE)',
    'ψ': 'psi (PSYE / SEE)',
    'ω': 'omega (oh-MAY-guh)',
    // Uppercase
    'Α': 'Alpha (AL-fuh)',
    'Β': 'Beta (BAY-tuh)',
    'Γ': 'Gamma (GAM-uh)',
    'Δ': 'Delta (DEL-tuh)',
    'Ε': 'Epsilon (EP-sih-lon)',
    'Ζ': 'Zeta (ZAY-tuh)',
    'Η': 'Eta (AY-tuh)',
    'Θ': 'Theta (THAY-tuh)',
    'Ι': 'Iota (eye-OH-tuh)',
    'Κ': 'Kappa (KAP-uh)',
    'Λ': 'Lambda (LAM-duh)',
    'Μ': 'Mu (MYOO)',
    'Ν': 'Nu (NOO)',
    'Ξ': 'Xi (KSEE / ZYE)',
    'Ο': 'Omicron (OM-ih-kron)',
    'Π': 'Pi (PIE)',
    'Ρ': 'Rho (ROH)',
    'Σ': 'Sigma (SIG-muh)',
    'Τ': 'Tau (TAW / TOW)',
    'Υ': 'Upsilon (OOP-sih-lon)',
    'Φ': 'Phi (FYE / FEE)',
    'Χ': 'Chi (KYE / KHEE)',
    'Ψ': 'Psi (PSYE / SEE)',
    'Ω': 'Omega (oh-MAY-guh)',
    // Common math variants
    'ϕ': 'phi variant (FYE / FEE)',
    'ϑ': 'theta variant (THAY-tuh)',
    'ϵ': 'epsilon variant (EP-sih-lon)',
    'ϰ': 'kappa variant (KAP-uh)',
    'ϱ': 'rho variant (ROH)',
    'ϖ': 'pi variant (PIE)',
    // Hebrew (common in math)
    'ℵ': 'aleph (AH-lef)',
    'ℶ': 'beth (BET)',
    'ℷ': 'gimel (GIM-el)',
    'ℸ': 'daleth (DAH-let)',
    // Common math symbols
    '∞': 'infinity',
    '∂': 'partial derivative',
    '∇': 'nabla / del',
    '∑': 'summation',
    '∏': 'product',
    '∫': 'integral',
    '∮': 'contour integral',
    '√': 'square root',
    '∝': 'proportional to',
    '∈': 'element of',
    '∉': 'not element of',
    '⊂': 'subset of',
    '⊃': 'superset of',
    '⊆': 'subset or equal',
    '⊇': 'superset or equal',
    '∪': 'union',
    '∩': 'intersection',
    '∅': 'empty set',
    '∀': 'for all',
    '∃': 'there exists',
    '∄': 'there does not exist',
    '∧': 'logical and',
    '∨': 'logical or',
    '¬': 'logical not',
    '⊕': 'xor / direct sum',
    '⊗': 'tensor product',
    '≈': 'approximately equal',
    '≠': 'not equal',
    '≤': 'less than or equal',
    '≥': 'greater than or equal',
    '≪': 'much less than',
    '≫': 'much greater than',
    '≡': 'identical / congruent',
    '≢': 'not identical',
    '⟨': 'left angle bracket',
    '⟩': 'right angle bracket',
    '†': 'dagger / adjoint',
    '‡': 'double dagger',
    '⊥': 'perpendicular / bottom',
    '∥': 'parallel',
    '∠': 'angle',
    '°': 'degree',
    '′': 'prime',
    '″': 'double prime',
    'ℏ': 'h-bar (Planck constant)',
    'ℓ': 'script l',
    'ℜ': 'real part',
    'ℑ': 'imaginary part',
    '℘': 'Weierstrass p',
};

let symbolTooltip = null;

function createSymbolTooltip() {
    if (!symbolTooltip) {
        symbolTooltip = document.createElement('div');
        symbolTooltip.className = 'symbol-tooltip';
        document.body.appendChild(symbolTooltip);
    }
    return symbolTooltip;
}

function showSymbolTooltip(event, char, description) {
    const tooltip = createSymbolTooltip();
    tooltip.innerHTML = `<span class="symbol-char">${char}</span> — ${description}`;

    // Position near cursor
    const x = event.clientX + 12;
    const y = event.clientY + 12;

    tooltip.style.left = `${x}px`;
    tooltip.style.top = `${y}px`;
    tooltip.classList.add('visible');

    // Ensure tooltip stays in viewport
    requestAnimationFrame(() => {
        const rect = tooltip.getBoundingClientRect();
        if (rect.right > window.innerWidth) {
            tooltip.style.left = `${event.clientX - rect.width - 8}px`;
        }
        if (rect.bottom > window.innerHeight) {
            tooltip.style.top = `${event.clientY - rect.height - 8}px`;
        }
    });
}

function hideSymbolTooltip() {
    if (symbolTooltip) {
        symbolTooltip.classList.remove('visible');
    }
}

function setupSymbolTooltips(container) {
    let currentChar = null;

    container.addEventListener('mousemove', (e) => {
        // Get character under cursor using document.caretPositionFromPoint or caretRangeFromPoint
        let range;
        if (document.caretPositionFromPoint) {
            const pos = document.caretPositionFromPoint(e.clientX, e.clientY);
            if (pos && pos.offsetNode && pos.offsetNode.nodeType === Node.TEXT_NODE) {
                range = document.createRange();
                range.setStart(pos.offsetNode, pos.offset);
                range.setEnd(pos.offsetNode, Math.min(pos.offset + 1, pos.offsetNode.length));
            }
        } else if (document.caretRangeFromPoint) {
            range = document.caretRangeFromPoint(e.clientX, e.clientY);
            if (range && range.startContainer.nodeType === Node.TEXT_NODE) {
                range.setEnd(range.startContainer, Math.min(range.startOffset + 1, range.startContainer.length));
            }
        }

        if (range && range.startContainer.nodeType === Node.TEXT_NODE) {
            const text = range.startContainer.textContent;
            const offset = range.startOffset;
            if (offset < text.length) {
                const char = text[offset];
                const description = GREEK_LETTERS[char];
                if (description) {
                    if (char !== currentChar) {
                        currentChar = char;
                        showSymbolTooltip(e, char, description);
                    }
                    return;
                }
            }
        }

        // No matching symbol under cursor
        if (currentChar) {
            currentChar = null;
            hideSymbolTooltip();
        }
    });

    container.addEventListener('mouseleave', () => {
        currentChar = null;
        hideSymbolTooltip();
    });
}

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
    initFuzzyFinder();

    // Initialize new features
    setupReadingProgress();
    setupPresentationMode();
    addCodeCopyButtons();

    // Set up Greek/math symbol tooltips on content area
    const content = document.querySelector('.markdown-body');
    if (content) {
        setupSymbolTooltips(content);
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

// ============================================================================
// Diff Overlay
// ============================================================================

/**
 * Store the current diff result for later display.
 * Called by tauri-app.js after fetching diff from backend.
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
window.setCurrentDiff = setCurrentDiff;
window.getCurrentDiff = getCurrentDiff;
window.toggleDiffOverlay = toggleDiffOverlay;
window.hideDiffOverlay = hideDiffOverlay;
window.isDiffOverlayVisible = isDiffOverlayVisible;
