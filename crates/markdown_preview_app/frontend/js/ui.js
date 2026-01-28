// UI Components - TOC, themes, recent files, fuzzy finder, initialization

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
    const savedTOCMode = localStorage.getItem('tocMode');
    let tocMode = 'off';
    if (savedTOCMode) {
        tocMode = savedTOCMode;
    } else if (window.innerWidth >= 1440) {
        tocMode = 'right';
    }
    const tocModeSelect = document.getElementById('toc-mode');
    if (tocModeSelect) tocModeSelect.value = tocMode;

    const savedFont = localStorage.getItem('fontFamily') || 'default';
    fontFamily = savedFont;
    document.getElementById('font-family').value = savedFont;

    const savedReaderMode = localStorage.getItem('readerMode') === 'true';
    readerMode = savedReaderMode;
    document.getElementById('reader-mode').value = savedReaderMode ? 'on' : 'off';

    const savedTheme = localStorage.getItem('theme') || 'auto';
    currentTheme = savedTheme;
    const themeSelect = document.getElementById('theme-select');
    if (themeSelect) themeSelect.value = savedTheme;

    const savedZoom = parseInt(localStorage.getItem('zoomLevel')) || 100;
    zoomLevel = savedZoom;
    updateZoomDisplay();

    // Set up event listeners
    if (tocModeSelect) {
        tocModeSelect.addEventListener('change', (e) => {
            toggleTOC(e.target.value);
        });
    }

    document.getElementById('font-family').addEventListener('change', (e) => {
        changeFontFamily(e.target.value);
    });

    document.getElementById('reader-mode').addEventListener('change', (e) => {
        toggleReaderMode(e.target.value === 'on');
    });

    if (themeSelect) {
        themeSelect.addEventListener('change', (e) => {
            changeTheme(e.target.value);
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

