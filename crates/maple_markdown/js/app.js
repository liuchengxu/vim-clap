        let currentSourceLines = 0;
        let currentLineMap = [];  // Array mapping element index to source line number
        let tocVisible = false;
        let fontFamily = 'default';  // 'default', 'serif', 'mono', 'system'
        let readerMode = false;  // Reader mode toggle
        let currentFilePath = '';  // Current file being previewed
        let websocket = null;  // WebSocket connection
        let currentTheme = 'auto';  // 'auto', 'github-light', 'github-dark', 'dark', 'material-dark', 'one-dark', 'ulysses'

        // Tauri detection and abstraction layer
        var isTauri = window.__TAURI__ !== undefined;

        // Unified communication bridge
        const AppBridge = {
            isTauri: isTauri,

            // Send a message (WebSocket or Tauri IPC)
            async send(message) {
                if (isTauri) {
                    const { invoke } = window.__TAURI__.core;
                    return invoke('handle_message', { message: JSON.stringify(message) });
                } else if (websocket && websocket.readyState === WebSocket.OPEN) {
                    websocket.send(JSON.stringify(message));
                }
            },

            // Request a file to be opened
            async openFile(filePath) {
                if (isTauri) {
                    const { invoke } = window.__TAURI__.core;
                    return invoke('open_file', { path: filePath });
                } else {
                    this.send({ type: 'switch_file', file_path: filePath });
                }
            },

            // Open file dialog (Tauri only)
            async showOpenDialog() {
                if (isTauri) {
                    const { open } = window.__TAURI__.plugin.dialog;
                    const selected = await open({
                        multiple: false,
                        filters: [{ name: 'Markdown', extensions: ['md', 'markdown', 'mdown', 'mkdn', 'mkd'] }]
                    });
                    return selected;
                }
                return null;
            },

            // Get recent files
            async getRecentFiles() {
                if (isTauri) {
                    const { invoke } = window.__TAURI__.core;
                    return invoke('get_recent_files');
                } else {
                    // Use localStorage for browser mode
                    const recent = localStorage.getItem('recentFiles');
                    return recent ? JSON.parse(recent) : [];
                }
            },

            // Watch a file for changes (Tauri only)
            async watchFile(filePath) {
                if (isTauri) {
                    const { invoke } = window.__TAURI__.core;
                    return invoke('watch_file', { path: filePath });
                }
            },

            // Stop watching file (Tauri only)
            async unwatchFile() {
                if (isTauri) {
                    const { invoke } = window.__TAURI__.core;
                    return invoke('unwatch_file');
                }
            },

            // Set up event listeners for Tauri
            setupTauriListeners(onMessage) {
                if (isTauri) {
                    const { listen } = window.__TAURI__.event;

                    // Listen for file-changed events from Rust
                    listen('file-changed', (event) => {
                        onMessage({ type: 'update_content', ...event.payload });
                    });

                    // Listen for menu events
                    listen('menu-open', async () => {
                        const filePath = await this.showOpenDialog();
                        if (filePath) {
                            const response = await this.openFile(filePath);
                            onMessage({ type: 'update_content', ...response });
                        }
                    });

                    listen('menu-reload', async () => {
                        if (currentFilePath) {
                            const response = await this.openFile(currentFilePath);
                            onMessage({ type: 'update_content', ...response });
                        }
                    });

                    listen('menu-toggle-toc', () => {
                        tocVisible = !tocVisible;
                        if (tocVisible) {
                            generateTOC();
                            document.body.classList.add('toc-visible');
                        } else {
                            document.body.classList.remove('toc-visible');
                        }
                    });

                    listen('menu-theme', (event) => {
                        changeTheme(event.payload);
                    });
                }
            }
        };

        // Fuzzy finder state
        let fuzzyFinderOpen = false;
        let fuzzySearchMode = 'headings';  // 'headings' or 'text'
        let fuzzySelectedIndex = 0;
        let fuzzyResults = [];
        let fuzzySearchIndex = { headings: [], text: [] };

        // Fuzzy matching algorithm - returns score and match positions
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

                    // Bonus for consecutive matches
                    if (lastMatchIdx === i - 1) {
                        score += 10;
                    }

                    // Bonus for matching at word boundaries
                    if (i === 0 || /[\s\-_./]/.test(text[i - 1])) {
                        score += 5;
                    }

                    // Bonus for exact case match
                    if (text[i] === pattern[patternIdx]) {
                        score += 2;
                    }

                    score += 1;
                    lastMatchIdx = i;
                    patternIdx++;
                }
            }

            // Must match all characters
            if (patternIdx !== pattern.length) {
                return { score: 0, positions: [] };
            }

            // Bonus for shorter strings (more relevant matches)
            score += Math.max(0, 50 - text.length);

            return { score, positions };
        }

        // Highlight matched characters in text
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

        // Escape HTML special characters
        function escapeHtml(text) {
            const div = document.createElement('div');
            div.textContent = text;
            return div.innerHTML;
        }

        // Build search index from current content
        function buildFuzzyIndex() {
            const content = document.getElementById('content');
            if (!content) return;

            fuzzySearchIndex = { headings: [], text: [] };

            // Index headings
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

            // Index text blocks (paragraphs, list items, code blocks)
            const textElements = content.querySelectorAll('p, li, pre, blockquote, td, th');
            textElements.forEach((el, index) => {
                const text = el.textContent.trim();
                if (text.length > 10 && text.length < 500) {  // Skip very short or very long texts
                    // Find the nearest heading for context
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
                        text: text.slice(0, 200),  // Truncate for display
                        fullText: text,
                        element: el,
                        context: context
                    });
                }
            });
        }

        // Perform fuzzy search
        function fuzzySearch(query) {
            const index = fuzzySearchMode === 'headings' ? fuzzySearchIndex.headings : fuzzySearchIndex.text;

            if (!query) {
                // Show all items when no query (limited)
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
                    .slice(0, 50);  // Limit results
            }

            fuzzySelectedIndex = 0;
            renderFuzzyResults();
        }

        // Render fuzzy search results
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

            // Scroll selected item into view
            const selectedItem = resultsContainer.querySelector('.fuzzy-result-item.selected');
            if (selectedItem) {
                selectedItem.scrollIntoView({ block: 'nearest' });
            }
        }

        // Open fuzzy finder
        function openFuzzyFinder() {
            buildFuzzyIndex();
            fuzzyFinderOpen = true;
            fuzzySelectedIndex = 0;

            const overlay = document.getElementById('fuzzy-finder');
            const input = document.getElementById('fuzzy-input');

            overlay.classList.add('visible');
            input.value = '';
            input.placeholder = fuzzySearchMode === 'headings' ? 'Search headings...' : 'Search full text...';

            // Show initial results
            fuzzySearch('');

            // Focus input
            setTimeout(() => input.focus(), 50);
        }

        // Close fuzzy finder
        function closeFuzzyFinder() {
            fuzzyFinderOpen = false;
            const overlay = document.getElementById('fuzzy-finder');
            overlay.classList.remove('visible');
        }

        // Toggle search mode
        function toggleFuzzyMode() {
            fuzzySearchMode = fuzzySearchMode === 'headings' ? 'text' : 'headings';

            const headingsBtn = document.getElementById('fuzzy-mode-headings');
            const textBtn = document.getElementById('fuzzy-mode-text');
            const input = document.getElementById('fuzzy-input');

            headingsBtn.classList.toggle('active', fuzzySearchMode === 'headings');
            textBtn.classList.toggle('active', fuzzySearchMode === 'text');
            input.placeholder = fuzzySearchMode === 'headings' ? 'Search headings...' : 'Search full text...';

            // Re-search with current query
            fuzzySearch(input.value);
        }

        // Navigate to selected result
        function selectFuzzyResult() {
            if (fuzzyResults.length === 0) return;

            const result = fuzzyResults[fuzzySelectedIndex];
            if (result && result.element) {
                closeFuzzyFinder();

                // Navigate to the element
                if (result.id) {
                    navigateToHeading(result.id);
                } else {
                    result.element.scrollIntoView({ behavior: 'smooth', block: 'center' });

                    // Highlight briefly
                    result.element.style.backgroundColor = '#fff8c5';
                    result.element.style.transition = 'background-color 0.3s';
                    setTimeout(() => {
                        result.element.style.backgroundColor = '';
                    }, 1500);
                }
            }
        }

        // Handle fuzzy finder keyboard navigation
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

        // Request manual refresh from server
        function requestRefresh() {
            if (currentFilePath) {
                AppBridge.send({
                    type: 'switch_file',
                    file_path: currentFilePath
                });
                showToast('Refreshing...');
            }
        }

        // Initialize fuzzy finder event listeners
        function initFuzzyFinder() {
            const overlay = document.getElementById('fuzzy-finder');
            const input = document.getElementById('fuzzy-input');
            const resultsContainer = document.getElementById('fuzzy-results');
            const headingsBtn = document.getElementById('fuzzy-mode-headings');
            const textBtn = document.getElementById('fuzzy-mode-text');

            // Global keyboard shortcut (Ctrl+P / Cmd+P)
            document.addEventListener('keydown', (e) => {
                // Fuzzy finder: Ctrl+P / Cmd+P
                if ((e.ctrlKey || e.metaKey) && e.key === 'p') {
                    e.preventDefault();
                    if (fuzzyFinderOpen) {
                        closeFuzzyFinder();
                    } else {
                        openFuzzyFinder();
                    }
                }

                // Manual refresh: F5 or Ctrl+R / Cmd+R
                if (e.key === 'F5' || ((e.ctrlKey || e.metaKey) && e.key === 'r')) {
                    e.preventDefault();
                    requestRefresh();
                }

                // Handle navigation when finder is open
                if (fuzzyFinderOpen) {
                    handleFuzzyKeydown(e);
                }
            });

            // Close on overlay click
            overlay.addEventListener('click', (e) => {
                if (e.target === overlay) {
                    closeFuzzyFinder();
                }
            });

            // Search on input
            input.addEventListener('input', (e) => {
                fuzzySearch(e.target.value);
            });

            // Click on result
            resultsContainer.addEventListener('click', (e) => {
                const item = e.target.closest('.fuzzy-result-item');
                if (item) {
                    fuzzySelectedIndex = parseInt(item.dataset.index);
                    selectFuzzyResult();
                }
            });

            // Mode toggle buttons
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

        function codeHighlight() {
            if (hljs !== undefined) {
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

        function applyLineNumberMode(mode, sourceLines, lineMap) {
            const content = document.getElementById('content');
            if (!content) return;

            // Update current source lines if provided
            if (sourceLines) {
                currentSourceLines = sourceLines;
            }

            // Update line map if provided
            if (lineMap) {
                currentLineMap = lineMap;
            }

            // Remove all line number classes and attributes
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
                // Show simple 1, 2, 3... numbering
                content.classList.add('show-line-numbers');
                children.forEach((child, index) => {
                    const renderedNum = index + 1;
                    child.setAttribute('data-line-number', renderedNum);
                    // Use line map for scrolling if available, otherwise use rendered number
                    const sourceNum = currentLineMap[index] || renderedNum;
                    child.setAttribute('data-source-line', sourceNum);
                });
            } else if (mode === 'source') {
                // Show actual source file line numbers from line map
                content.classList.add('show-line-numbers');
                children.forEach((child, index) => {
                    // Use line map if available, otherwise fall back to naive calculation
                    const sourceNum = currentLineMap[index] || Math.min(
                        Math.floor((index / Math.max(1, children.length)) * currentSourceLines) + 1,
                        currentSourceLines
                    );
                    child.setAttribute('data-line-number', sourceNum);
                    child.setAttribute('data-source-line', sourceNum);
                });
            } else if (mode === 'both') {
                // Show both rendered/source
                content.classList.add('show-line-numbers-both');
                children.forEach((child, index) => {
                    const renderedNum = index + 1;
                    // Use line map if available, otherwise fall back to naive calculation
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

        // Scroll to a specific source line
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
                // Highlight briefly
                closestElement.style.backgroundColor = '#ffeb3b';
                setTimeout(() => {
                    closestElement.style.backgroundColor = '';
                }, 1000);
            }
        }

        // Navigate to a heading by ID and update URL hash
        function navigateToHeading(id, smooth = true) {
            const heading = document.getElementById(id);
            if (heading) {
                // Update URL hash without triggering hashchange scroll
                history.pushState(null, '', `#${id}`);
                heading.scrollIntoView({ behavior: smooth ? 'smooth' : 'auto', block: 'start' });
            }
        }

        // Scroll to hash on page load or hashchange
        function scrollToHash() {
            const hash = window.location.hash.slice(1);
            if (hash) {
                // Small delay to ensure content is rendered
                setTimeout(() => {
                    const element = document.getElementById(hash);
                    if (element) {
                        element.scrollIntoView({ behavior: 'auto', block: 'start' });
                    }
                }, 100);
            }
        }

        // Add GitHub-style anchor links to all headings
        function addHeadingAnchors() {
            const content = document.getElementById('content');
            if (!content) return;

            const headings = content.querySelectorAll('h1, h2, h3, h4, h5, h6');

            headings.forEach((heading, index) => {
                // Ensure heading has an ID
                if (!heading.id) {
                    heading.id = `heading-${index}`;
                }

                // Skip if anchor already exists
                if (heading.querySelector('.heading-anchor')) return;

                // Make heading a flex container for proper alignment
                heading.style.display = 'flex';
                heading.style.alignItems = 'center';
                heading.style.flexWrap = 'wrap';

                // Create anchor link (GitHub-style)
                const anchor = document.createElement('a');
                anchor.className = 'heading-anchor';
                anchor.href = `#${heading.id}`;
                anchor.setAttribute('aria-label', `Link to ${heading.textContent}`);
                anchor.innerHTML = '<svg class="octicon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><path fill-rule="evenodd" d="M7.775 3.275a.75.75 0 001.06 1.06l1.25-1.25a2 2 0 112.83 2.83l-2.5 2.5a2 2 0 01-2.83 0 .75.75 0 00-1.06 1.06 3.5 3.5 0 004.95 0l2.5-2.5a3.5 3.5 0 00-4.95-4.95l-1.25 1.25zm-4.69 9.64a2 2 0 010-2.83l2.5-2.5a2 2 0 012.83 0 .75.75 0 001.06-1.06 3.5 3.5 0 00-4.95 0l-2.5 2.5a3.5 3.5 0 004.95 4.95l1.25-1.25a.75.75 0 00-1.06-1.06l-1.25 1.25a2 2 0 01-2.83 0z"></path></svg>';

                anchor.onclick = (e) => {
                    e.preventDefault();
                    navigateToHeading(heading.id);
                };

                // Insert anchor before heading content
                heading.insertBefore(anchor, heading.firstChild);
            });
        }

        // Generate Table of Contents from headings
        function generateTOC() {
            const content = document.getElementById('content');
            const tocContent = document.getElementById('toc-content');
            if (!content || !tocContent) return;

            const headings = content.querySelectorAll('h1, h2, h3, h4, h5, h6');
            tocContent.innerHTML = '';

            headings.forEach((heading, index) => {
                const level = heading.tagName.toLowerCase();
                // Get text content excluding the anchor link
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

        // Get smart default TOC width based on screen size
        function getDefaultTOCWidth() {
            const screenWidth = window.innerWidth;
            // For wide monitors (>= 1920px), use 350px
            // For medium screens (>= 1440px), use 300px
            // For smaller screens, use 250px
            if (screenWidth >= 1920) {
                return 350;
            } else if (screenWidth >= 1440) {
                return 300;
            } else {
                return 250;
            }
        }

        // Toggle TOC visibility and position
        function toggleTOC(mode) {
            // mode can be 'off', 'left', or 'right'
            const tocPanel = document.getElementById('toc-panel');

            if (mode === 'off') {
                tocVisible = false;
                tocPanel.classList.remove('visible', 'toc-left', 'toc-right');
            } else {
                tocVisible = true;
                tocPanel.classList.add('visible');

                // Remove old position classes and add new one
                if (mode === 'left') {
                    tocPanel.classList.remove('toc-right');
                    tocPanel.classList.add('toc-left');
                } else if (mode === 'right') {
                    tocPanel.classList.remove('toc-left');
                    tocPanel.classList.add('toc-right');
                }

                generateTOC();

                // Restore saved width if available, otherwise use smart default
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

        // Change font family
        function changeFontFamily(family) {
            fontFamily = family;
            const content = document.getElementById('content');
            if (!content) return;

            // Remove all font classes
            content.classList.remove('font-serif', 'font-mono', 'font-system', 'font-inter', 'font-merriweather', 'font-ibm-plex', 'font-literata');

            // Add new font class if not default
            if (family !== 'default') {
                content.classList.add('font-' + family);
            }

            localStorage.setItem('fontFamily', family);
        }

        // Toggle Reader Mode
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

        // Theme switching
        function changeTheme(theme) {
            currentTheme = theme;

            // Remove all theme classes from body
            document.body.classList.remove(
                'theme-dark',
                'theme-material-dark',
                'theme-one-dark',
                'theme-ulysses',
                'theme-github-light',
                'theme-github-dark'
            );

            // Apply the selected theme
            if (theme === 'auto') {
                // Use system preference - no class needed, CSS media queries handle it
            } else if (theme === 'github-light') {
                // Default light theme - no class needed
            } else if (theme === 'github-dark') {
                // Apply dark mode styles via class that mimics prefers-color-scheme: dark
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

        // Recent files management
        function getRecentFiles() {
            const recent = localStorage.getItem('recentFiles');
            return recent ? JSON.parse(recent) : [];
        }

        function addToRecentFiles(filePath) {
            if (!filePath) return;

            let recentFiles = getRecentFiles();
            // Remove if already exists
            recentFiles = recentFiles.filter(f => f.path !== filePath);
            // Add to front with timestamp
            recentFiles.unshift({
                path: filePath,
                timestamp: Date.now()
            });
            // Limit to 10 most recent
            recentFiles = recentFiles.slice(0, 10);
            localStorage.setItem('recentFiles', JSON.stringify(recentFiles));
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

        function updateFilePathBar(filePath, gitRoot) {
            const pathBar = document.getElementById('file-path-bar');
            if (!pathBar) return;

            if (filePath) {
                pathBar.style.display = 'block';
                pathBar.title = `Click to copy: ${filePath}`;

                if (gitRoot && filePath.startsWith(gitRoot)) {
                    // Extract git root directory name
                    const gitRootName = gitRoot.split('/').filter(p => p).pop() || gitRoot;
                    // Get relative path from git root
                    const relativePath = filePath.substring(gitRoot.length);

                    // Build HTML with styled parts
                    pathBar.innerHTML = `<span class="git-root">${gitRootName}</span><span class="path-separator">/</span><span class="relative-path">${relativePath.replace(/^\//, '')}</span>`;
                } else {
                    // No git root found, show full path
                    pathBar.textContent = filePath;
                }
            } else {
                pathBar.style.display = 'none';
            }
        }

        function showToast(message, subtitle = null, duration = 2000) {
            // Create toast element
            const toast = document.createElement('div');
            toast.className = 'toast';

            if (subtitle) {
                // Create title element
                const titleDiv = document.createElement('div');
                titleDiv.className = 'toast-title';
                titleDiv.textContent = message;
                toast.appendChild(titleDiv);

                // Create path element
                const pathDiv = document.createElement('div');
                pathDiv.className = 'toast-path';
                pathDiv.textContent = subtitle;
                toast.appendChild(pathDiv);
            } else {
                toast.textContent = message;
            }

            document.body.appendChild(toast);

            // Show toast with animation
            setTimeout(() => {
                toast.classList.add('show');
            }, 10);

            // Hide and remove toast after duration
            setTimeout(() => {
                toast.classList.remove('show');
                setTimeout(() => {
                    document.body.removeChild(toast);
                }, 300);
            }, duration);
        }

        function copyToClipboard(text) {
            // Use the Clipboard API if available
            if (navigator.clipboard && navigator.clipboard.writeText) {
                navigator.clipboard.writeText(text).then(() => {
                    console.log('Copied to clipboard:', text);
                    showToast('Copied: ' + getFileBasename(text), text);
                }).catch(err => {
                    console.error('Failed to copy to clipboard:', err);
                    showToast('Failed to copy to clipboard');
                });
            } else {
                // Fallback for older browsers
                const textArea = document.createElement('textarea');
                textArea.value = text;
                textArea.style.position = 'fixed';
                textArea.style.left = '-999999px';
                document.body.appendChild(textArea);
                textArea.focus();
                textArea.select();
                try {
                    document.execCommand('copy');
                    console.log('Copied to clipboard:', text);
                    showToast('Copied: ' + getFileBasename(text), text);
                } catch (err) {
                    console.error('Failed to copy to clipboard:', err);
                    showToast('Failed to copy to clipboard');
                }
                document.body.removeChild(textArea);
            }
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

        function renderRecentFiles() {
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

                // Create path element
                const pathElement = document.createElement('div');
                pathElement.className = 'recent-file-path';
                pathElement.textContent = getFileDirectory(file.path);
                pathElement.title = file.path;

                item.appendChild(headerRow);
                item.appendChild(pathElement);

                // Left click to switch file
                item.onclick = (e) => {
                    // If shift key is held, copy to clipboard instead of switching
                    if (e.shiftKey) {
                        copyToClipboard(file.path);
                        e.stopPropagation();
                        return;
                    }
                    switchToFile(file.path);
                };

                // Right click to copy to clipboard
                item.oncontextmenu = (e) => {
                    e.preventDefault();
                    copyToClipboard(file.path);
                };

                container.appendChild(item);
            });
        }

        function switchToFile(filePath) {
            if (filePath === currentFilePath) {
                return;  // Already viewing this file
            }

            // Send switch file request via AppBridge (works for both WebSocket and Tauri)
            AppBridge.openFile(filePath);
            console.log(`Switching to: ${filePath}`);
        }

        // Format number with thousands separator
        function formatNumber(num) {
            return num.toLocaleString();
        }

        // Format reading time in a human-readable way
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

        // Update document stats display
        function updateDocumentStats(stats) {
            if (!stats) return;

            document.getElementById('stat-words').textContent = formatNumber(stats.words);
            document.getElementById('stat-characters').textContent = formatNumber(stats.characters);
            document.getElementById('stat-lines').textContent = formatNumber(stats.lines);
            document.getElementById('stat-reading-time').textContent = formatReadingTime(stats.reading_minutes);
        }

        // Set up TOC resize functionality
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
                // For right-side TOC, delta should be negative to increase width
                // For left-side TOC, delta should be positive to increase width
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
                    // Save the new width
                    localStorage.setItem('tocWidth', tocPanel.offsetWidth);
                }
            });
        }

        document.addEventListener('DOMContentLoaded', (event) => {
            // Restore saved preferences
            const savedMode = localStorage.getItem('lineNumberMode') || 'off';
            lineNumberMode = savedMode;
            document.getElementById('line-numbers-mode').value = savedMode;

            // For TOC, use saved preference if exists, otherwise default to right for wide monitors
            const savedTOCMode = localStorage.getItem('tocMode');
            let tocMode = 'off';
            if (savedTOCMode) {
                tocMode = savedTOCMode;
            } else if (window.innerWidth >= 1440) {
                // Default to right for monitors >= 1440px
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

            // Set up line number mode change listener
            document.getElementById('line-numbers-mode').addEventListener('change', (e) => {
                changeLineNumberMode(e.target.value);
            });

            // Set up TOC toggle listener
            document.getElementById('toc-mode').addEventListener('change', (e) => {
                toggleTOC(e.target.value);
            });

            // Set up font family change listener
            document.getElementById('font-family').addEventListener('change', (e) => {
                changeFontFamily(e.target.value);
            });

            // Set up reader mode change listener
            document.getElementById('reader-mode').addEventListener('change', (e) => {
                toggleReaderMode(e.target.value === 'on');
            });

            // Set up theme change listener
            document.getElementById('theme-select').addEventListener('change', (e) => {
                changeTheme(e.target.value);
            });

            // Set up file path bar click-to-copy
            document.getElementById('file-path-bar').addEventListener('click', () => {
                if (currentFilePath) {
                    copyToClipboard(currentFilePath);
                }
            });

            // Set up TOC resize functionality
            setupTOCResize();

            // Handle browser back/forward navigation with hash
            window.addEventListener('hashchange', () => {
                const hash = window.location.hash.slice(1);
                if (hash) {
                    const element = document.getElementById(hash);
                    if (element) {
                        element.scrollIntoView({ behavior: 'smooth', block: 'start' });
                    }
                }
            });

            document.querySelectorAll('pre code').forEach((el) => {
                hljs.highlightElement(el);
            });

            codeHighlight();
            renderMermaid();
            applyLineNumberMode(lineNumberMode, currentSourceLines, currentLineMap);

            // Initialize TOC with saved mode
            if (tocMode !== 'off') {
                toggleTOC(tocMode);
            }

            // Apply saved font family
            changeFontFamily(fontFamily);

            // Apply saved reader mode
            if (readerMode) {
                toggleReaderMode(true);
            }

            // Apply saved theme
            changeTheme(currentTheme);

            // Initialize recent files display
            renderRecentFiles();

            // Initialize fuzzy finder
            initFuzzyFinder();

            // Set up welcome screen "Open File" button (Tauri only)
            const welcomeOpenBtn = document.getElementById('welcome-open-btn');
            if (welcomeOpenBtn && isTauri) {
                console.log('[Welcome] Setting up Open File button, Tauri API:', window.__TAURI__);
                welcomeOpenBtn.addEventListener('click', async () => {
                    console.log('[Welcome] Open File clicked');
                    try {
                        // Tauri 2.x plugin dialog API
                        const open = window.__TAURI__.dialog?.open || window.__TAURI__.plugin?.dialog?.open;
                        console.log('[Welcome] Dialog open function:', open);
                        if (!open) {
                            console.error('[Welcome] Dialog API not found. Available:', Object.keys(window.__TAURI__));
                            alert('Dialog API not available');
                            return;
                        }
                        const selected = await open({
                            multiple: false,
                            filters: [{ name: 'Markdown', extensions: ['md', 'markdown', 'mdown', 'mkdn', 'mkd'] }]
                        });
                        console.log('[Welcome] Selected file:', selected);
                        if (selected) {
                            const { invoke } = window.__TAURI__.core;
                            const result = await invoke('open_file', { path: selected });
                            console.log('[Welcome] Open file result:', result);
                            if (result && result.html) {
                                document.getElementById('content').innerHTML = result.html;
                                codeHighlight();
                                renderMermaid();
                                generateTOC();
                                if (result.file_path) {
                                    currentFilePath = result.file_path;
                                    updateFilePathBar(result.file_path, result.git_root);
                                }
                                if (result.stats) {
                                    updateDocumentStats(result.stats);
                                }
                            }
                        }
                    } catch (e) {
                        console.error('[Welcome] Failed to open file:', e);
                        alert('Error: ' + e.message);
                    }
                });
            } else {
                console.log('[Welcome] Button not found or not Tauri:', { btn: !!welcomeOpenBtn, isTauri });
            }

            // Unified message handler for both WebSocket and Tauri
            function handleMessage(message) {
                if (message.type === "update_content") {
                    document.getElementById('content').innerHTML = message.data || message.html;

                    // Add spacer at the end for better scrolling
                    const spacer = document.createElement('div');
                    spacer.style.height = '100px';
                    spacer.style.pointerEvents = 'none';
                    document.getElementById('content').appendChild(spacer);

                    // Debug: log first 10 children
                    const debugChildren = Array.from(document.getElementById('content').children).slice(0, 10);
                    console.log('First 10 HTML children:', debugChildren.map((c, i) => `${i}: ${c.tagName}`));
                    console.log('Line map (first 10):', message.line_map?.slice(0, 10));

                    codeHighlight();
                    // Render LaTeX inside content
                    renderMathInElement(document.getElementById('content'), {
                        delimiters: [
                            {left: "$$", right: "$$", display: true},
                            {left: "$", right: "$", display: false}
                        ]
                    });
                    // Render mermaid diagrams
                    renderMermaid();
                    // Add GitHub-style anchor links to headings
                    addHeadingAnchors();
                    // Apply line numbers with source line count and line map
                    applyLineNumberMode(lineNumberMode, message.source_lines, message.line_map);
                    // Regenerate TOC if visible
                    if (tocVisible) {
                        generateTOC();
                    }
                    // Scroll to hash if present in URL
                    scrollToHash();
                    // Update file path and recent files if provided
                    if (message.file_path) {
                        currentFilePath = message.file_path;
                        addToRecentFiles(currentFilePath);
                        renderRecentFiles();
                        updateFilePathBar(currentFilePath, message.git_root);
                        // Update page title with filename
                        document.title = getFileBasename(currentFilePath) + ' - Markdown Preview';
                        // Start watching file for changes in Tauri mode
                        if (isTauri) {
                            AppBridge.watchFile(currentFilePath);
                        }
                    }
                    // Update document stats if provided
                    if (message.stats) {
                        updateDocumentStats(message.stats);
                    }
                    // Focus window if requested
                    if (message.should_focus) {
                        window.focus();
                    }
                } else if (message.type === "scroll") {
                    const scrollPercent = message.data;

                    // Calculate the absolute scroll position based on the percentage
                    const windowHeight = window.innerHeight;
                    const totalScrollHeight = document.documentElement.scrollHeight - windowHeight;
                    const absoluteScrollPosition = totalScrollHeight * (scrollPercent / 100);

                    // Scroll the page to the calculated absolute position
                    window.scrollTo(0, absoluteScrollPosition);
                } else if (message.type === "focus_window") {
                    // Bring the browser window to the foreground
                    window.focus();
                } else {
                    console.log(`Invalid message: ${JSON.stringify(message)}`);
                }
            }

            // Initialize communication based on mode
            if (isTauri) {
                console.log('Running in Tauri mode');
                AppBridge.setupTauriListeners(handleMessage);

                // Handle drag and drop for files
                document.addEventListener('drop', async (e) => {
                    e.preventDefault();
                    const files = e.dataTransfer?.files;
                    if (files && files.length > 0) {
                        const file = files[0];
                        if (file.name.match(/\.(md|markdown|mdown|mkdn|mkd)$/i)) {
                            // In Tauri, we get the path from the file
                            const path = file.path || file.name;
                            const response = await AppBridge.openFile(path);
                            handleMessage({ type: 'update_content', ...response });
                        }
                    }
                });

                document.addEventListener('dragover', (e) => {
                    e.preventDefault();
                });
            } else {
                console.log('Running in WebSocket mode');
                const webSocketUrl = 'ws://' + window.location.host;
                websocket = new WebSocket(webSocketUrl);
                var socket = websocket;  // Keep local reference for compatibility

                socket.onmessage = function(event) {
                    const message = JSON.parse(event.data);
                    handleMessage(message);
                };

                socket.onclose = function(event) {
                    // Close the browser window when WebSocket closes
                    // This happens when Vim exits or the server shuts down
                    console.log(`WebSocket closed with code ${event.code}. Closing browser.`);
                    window.open('', '_self', '');
                    window.close();
                };

                socket.onerror = function(error) {
                    console.error('WebSocket error:', error);
                };
            }
        });
