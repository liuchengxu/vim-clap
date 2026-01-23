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
        pathElement.title = file.path;

        item.appendChild(nameElement);
        item.appendChild(pathElement);

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
    applyLineNumberMode(lineNumberMode, message.source_lines, message.line_map);

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
    const savedMode = localStorage.getItem('lineNumberMode') || 'off';
    lineNumberMode = savedMode;
    document.getElementById('line-numbers-mode').value = savedMode;

    const savedTOCMode = localStorage.getItem('tocMode');
    let tocMode = 'off';
    if (savedTOCMode) {
        tocMode = savedTOCMode;
    } else if (window.innerWidth >= 1440) {
        tocMode = 'right';
    }
    document.getElementById('toc-mode').value = tocMode;

    const savedFont = localStorage.getItem('fontFamily') || 'default';
    fontFamily = savedFont;
    document.getElementById('font-family').value = savedFont;

    const savedReaderMode = localStorage.getItem('readerMode') === 'true';
    readerMode = savedReaderMode;
    document.getElementById('reader-mode').value = savedReaderMode ? 'on' : 'off';

    const savedTheme = localStorage.getItem('theme') || 'auto';
    currentTheme = savedTheme;
    document.getElementById('theme-select').value = savedTheme;

    // Set up event listeners
    document.getElementById('line-numbers-mode').addEventListener('change', (e) => {
        changeLineNumberMode(e.target.value);
    });

    document.getElementById('toc-mode').addEventListener('change', (e) => {
        toggleTOC(e.target.value);
    });

    document.getElementById('font-family').addEventListener('change', (e) => {
        changeFontFamily(e.target.value);
    });

    document.getElementById('reader-mode').addEventListener('change', (e) => {
        toggleReaderMode(e.target.value === 'on');
    });

    document.getElementById('theme-select').addEventListener('change', (e) => {
        changeTheme(e.target.value);
    });

    document.getElementById('file-path-bar').addEventListener('click', () => {
        if (currentFilePath) {
            copyToClipboard(currentFilePath);
        }
    });

    setupTOCResize();

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
    applyLineNumberMode(lineNumberMode, currentSourceLines, currentLineMap);

    if (tocMode !== 'off') {
        toggleTOC(tocMode);
    }

    changeFontFamily(fontFamily);

    if (readerMode) {
        toggleReaderMode(true);
    }

    changeTheme(currentTheme);

    renderRecentFiles(options.onFileClick);
    initFuzzyFinder();
}

// Export for use in platform-specific modules (browser global scope)
// These are needed because core.js uses 'let' which doesn't add to window
window.MarkdownPreviewCore = {
    // Getters/setters for state
    getCurrentFilePath: () => currentFilePath,
    setCurrentFilePath: (path) => { currentFilePath = path; },

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
    addToRecentFiles
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
window.addHeadingAnchors = addHeadingAnchors;
window.showToast = showToast;
window.copyToClipboard = copyToClipboard;
window.getFileBasename = getFileBasename;
window.renderRecentFiles = renderRecentFiles;
window.addToRecentFiles = addToRecentFiles;
