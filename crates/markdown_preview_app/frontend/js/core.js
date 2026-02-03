// Markdown Preview Core - Shared UI functionality
// This module contains all UI-related functions shared between vim-clap (WebSocket) and Tauri modes

// ============================================================================
// State Variables
// ============================================================================

let currentSourceLines = 0;
let currentLineMap = [];
let tocVisible = false;
let fontFamily = 'default';
let readerMode = false;
let currentFilePath = '';
let currentTheme = 'auto';
let lineNumberMode = 'off';
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
// Line Numbers
// ============================================================================

function applyLineNumberMode(mode, sourceLines, lineMap) {
    const content = document.getElementById('content');
    if (!content) return;

    if (sourceLines) {
        currentSourceLines = sourceLines;
    }
    if (lineMap) {
        currentLineMap = lineMap;
    }

    content.classList.remove('show-line-numbers', 'show-line-numbers-both');
    const children = Array.from(content.children);
    children.forEach((child) => {
        child.removeAttribute('data-line-number');
        child.removeAttribute('data-source-line');
        child.removeAttribute('data-rendered-number');
        child.removeAttribute('data-source-number');
    });

    if (mode === 'off') {
        return;
    }

    if (mode === 'rendered') {
        content.classList.add('show-line-numbers');
        children.forEach((child, index) => {
            const renderedNum = index + 1;
            child.setAttribute('data-line-number', renderedNum);
            const sourceNum = currentLineMap[index] || renderedNum;
            child.setAttribute('data-source-line', sourceNum);
        });
    } else if (mode === 'source') {
        content.classList.add('show-line-numbers');
        children.forEach((child, index) => {
            const sourceNum = currentLineMap[index] || Math.min(
                Math.floor((index / Math.max(1, children.length)) * currentSourceLines) + 1,
                currentSourceLines
            );
            child.setAttribute('data-line-number', sourceNum);
            child.setAttribute('data-source-line', sourceNum);
        });
    } else if (mode === 'both') {
        content.classList.add('show-line-numbers-both');
        children.forEach((child, index) => {
            const renderedNum = index + 1;
            const sourceNum = currentLineMap[index] || Math.min(
                Math.floor((index / Math.max(1, children.length)) * currentSourceLines) + 1,
                currentSourceLines
            );
            child.setAttribute('data-rendered-number', renderedNum);
            child.setAttribute('data-source-number', sourceNum);
            child.setAttribute('data-source-line', sourceNum);
        });
    }
}

function changeLineNumberMode(newMode) {
    lineNumberMode = newMode;
    applyLineNumberMode(lineNumberMode, currentSourceLines, currentLineMap);
    localStorage.setItem('lineNumberMode', lineNumberMode);
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
    applyZoom(zoomLevel + ZOOM_STEP);
    showToast(`Zoom: ${zoomLevel}%`);
}

function zoomOut() {
    applyZoom(zoomLevel - ZOOM_STEP);
    showToast(`Zoom: ${zoomLevel}%`);
}

function resetZoom() {
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
        const textNode = Array.from(heading.childNodes).find(n => n.nodeType === Node.TEXT_NODE || (n.nodeType === Node.ELEMENT_NODE && !n.classList.contains('heading-anchor')));
        const text = textNode ? textNode.textContent : heading.textContent;
        const id = heading.id || `heading-${index}`;

        if (!heading.id) {
            heading.id = id;
        }

        const link = document.createElement('a');
        link.href = `#${id}`;
        link.textContent = text.trim();
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
// Focus Mode
// ============================================================================

let focusModeEnabled = false;

function toggleFocusMode(enabled) {
    focusModeEnabled = enabled;
    document.body.classList.toggle('focus-mode', enabled);

    if (enabled) {
        setupFocusTracking();
    } else {
        document.querySelectorAll('.focus-active').forEach(el => {
            el.classList.remove('focus-active');
        });
    }
    localStorage.setItem('focusMode', enabled);
}

function setupFocusTracking() {
    const content = document.getElementById('content');
    const mainContent = document.getElementById('main-content');
    if (!content || !mainContent) return;

    const updateFocus = () => {
        if (!focusModeEnabled) return;

        const contentRect = mainContent.getBoundingClientRect();
        const centerY = contentRect.top + contentRect.height / 2;
        const elements = content.querySelectorAll('p, h1, h2, h3, h4, h5, h6, li, pre, blockquote, table');

        elements.forEach(el => el.classList.remove('focus-active'));

        let closestElement = null;
        let closestDistance = Infinity;

        for (const el of elements) {
            const rect = el.getBoundingClientRect();
            const elCenterY = rect.top + rect.height / 2;
            const distance = Math.abs(elCenterY - centerY);
            if (distance < closestDistance) {
                closestDistance = distance;
                closestElement = el;
            }
        }

        if (closestElement) closestElement.classList.add('focus-active');
    };

    updateFocus();

    if (!mainContent.dataset.focusSetup) {
        mainContent.dataset.focusSetup = 'true';
        mainContent.addEventListener('scroll', updateFocus, { passive: true });
    }
}

// ============================================================================
// Presentation Mode
// ============================================================================

let presentationMode = false;
let presentationSlides = [];
let currentSlide = 0;

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
