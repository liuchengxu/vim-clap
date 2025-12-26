        let currentSourceLines = 0;
        let currentLineMap = [];  // Array mapping element index to source line number
        let tocVisible = false;
        let fontFamily = 'default';  // 'default', 'serif', 'mono', 'system'
        let readerMode = false;  // Reader mode toggle
        let currentFilePath = '';  // Current file being previewed
        let websocket = null;  // WebSocket connection
        let currentTheme = 'auto';  // 'auto', 'github-light', 'github-dark', 'dark', 'material-dark', 'one-dark', 'ulysses'

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

        // Generate Table of Contents from headings
        function generateTOC() {
            const content = document.getElementById('content');
            const tocContent = document.getElementById('toc-content');
            if (!content || !tocContent) return;

            const headings = content.querySelectorAll('h1, h2, h3, h4, h5, h6');
            tocContent.innerHTML = '';

            headings.forEach((heading, index) => {
                const level = heading.tagName.toLowerCase();
                const text = heading.textContent;
                const id = heading.id || `heading-${index}`;

                if (!heading.id) {
                    heading.id = id;
                }

                const link = document.createElement('a');
                link.href = `#${id}`;
                link.textContent = text;
                link.className = `toc-${level}`;
                link.onclick = (e) => {
                    e.preventDefault();
                    heading.scrollIntoView({ behavior: 'smooth', block: 'start' });
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
            content.classList.remove('font-serif', 'font-mono', 'font-system');

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

                // Create filename element
                const nameElement = document.createElement('div');
                nameElement.className = 'recent-file-name';
                nameElement.textContent = getFileBasename(file.path);

                // Create path element
                const pathElement = document.createElement('div');
                pathElement.className = 'recent-file-path';
                pathElement.textContent = getFileDirectory(file.path);
                pathElement.title = file.path;

                item.appendChild(nameElement);
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

            if (!websocket || websocket.readyState !== WebSocket.OPEN) {
                console.error('WebSocket not connected');
                return;
            }

            // Send switch file request to Rust backend
            const request = {
                type: 'switch_file',
                file_path: filePath
            };

            websocket.send(JSON.stringify(request));
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

            const webSocketUrl = 'ws://' + window.location.host;
            websocket = new WebSocket(webSocketUrl);
            var socket = websocket;  // Keep local reference for compatibility

            // Handle WebSocket events...
            socket.onmessage = function(event) {
                const message = JSON.parse(event.data);

                if (message.type === "update_content") {
                    document.getElementById('content').innerHTML = message.data;

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
                    // Apply line numbers with source line count and line map
                    applyLineNumberMode(lineNumberMode, message.source_lines, message.line_map);
                    // Regenerate TOC if visible
                    if (tocVisible) {
                        generateTOC();
                    }
                    // Update file path and recent files if provided
                    if (message.file_path) {
                        currentFilePath = message.file_path;
                        addToRecentFiles(currentFilePath);
                        renderRecentFiles();
                        updateFilePathBar(currentFilePath, message.git_root);
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
                    console.log(`Invalid message: {message}`)
                }
            }

            socket.onclose = function(event) {
                // Close the browser window when WebSocket closes
                // This happens when Vim exits or the server shuts down
                console.log(`WebSocket closed with code ${event.code}. Closing browser.`);
                window.open('', '_self', '');
                window.close();
            }

            socket.onerror = function(error) {
                console.error('WebSocket error:', error);
            }
        });
    </script>
