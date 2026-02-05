// Markdown Preview - Tauri Mode (Standalone App)
// This module handles Tauri IPC communication for the standalone application

// Note: core.js must be loaded before this file

(function() {
    'use strict';

    // Check if we're running in Tauri
    if (typeof window.__TAURI__ === 'undefined') {
        console.log('Not running in Tauri mode');
        return;
    }

    console.log('Running in Tauri mode');

    const { invoke } = window.__TAURI__.core;
    const { listen } = window.__TAURI__.event;

    // ========================================
    // Extension support - single source of truth from backend
    // ========================================
    let SUPPORTED_EXTENSIONS = { by_type: {}, all: [] };

    // Initialize IMMEDIATELY - must happen before any event handlers
    const extensionsReady = initSupportedExtensions();

    async function initSupportedExtensions() {
        try {
            const result = await invoke('get_supported_extensions');
            SUPPORTED_EXTENSIONS = result;
            console.log('Loaded supported extensions:', SUPPORTED_EXTENSIONS);
        } catch (e) {
            console.error('Failed to load supported extensions:', e);
            // Fallback to hardcoded values if backend fails
            SUPPORTED_EXTENSIONS = {
                by_type: { markdown: ['md', 'markdown', 'mdown', 'mkdn', 'mkd'], pdf: ['pdf'] },
                all: ['md', 'markdown', 'mdown', 'mkdn', 'mkd', 'pdf']
            };
        }
    }

    // Check if a file is supported (for drag-drop validation)
    async function isSupported(filename) {
        await extensionsReady;
        const ext = filename.split('.').pop()?.toLowerCase();
        return ext && SUPPORTED_EXTENSIONS.all.includes(ext);
    }

    // Get dialog API (Tauri 2.x)
    function getDialogOpen() {
        return window.__TAURI__.dialog?.open || window.__TAURI__.plugin?.dialog?.open;
    }

    // Open file dialog and load selected file
    async function openFileDialog() {
        await extensionsReady;  // Readiness gate

        // Check clipboard first — if it contains a valid path or URL, open directly
        try {
            const clipboardApi = window.__TAURI__.clipboard
                || window.__TAURI__.clipboardManager
                || window.__TAURI__.plugin?.clipboardManager;
            if (clipboardApi && clipboardApi.readText) {
                const clipText = (await clipboardApi.readText() || '').trim();
                if (clipText) {
                    if (isUrl(clipText)) {
                        await openUrl(clipText);
                        return;
                    }
                    const validPath = await invoke('check_clipboard_for_markdown');
                    if (validPath) {
                        await openFile(validPath);
                        return;
                    }
                }
            }
        } catch (e) {
            console.debug('Clipboard check skipped:', e);
        }

        const open = getDialogOpen();
        if (!open) {
            console.error('Dialog API not available');
            return null;
        }

        try {
            // Build filters from backend-provided extensions
            const filters = [
                { name: 'All Documents', extensions: SUPPORTED_EXTENSIONS.all },
                ...Object.entries(SUPPORTED_EXTENSIONS.by_type).map(([name, exts]) => ({
                    name: name.charAt(0).toUpperCase() + name.slice(1),
                    extensions: exts
                }))
            ];

            // Determine default directory for the dialog
            let defaultPath;
            const currentPath = window.MarkdownPreviewCore.getCurrentFilePath();
            if (currentPath) {
                try {
                    const gitRoot = await invoke('get_current_git_root');
                    if (gitRoot) {
                        defaultPath = gitRoot;
                    }
                } catch (_) { /* no git root */ }
                if (!defaultPath) {
                    const sep = currentPath.includes('\\') ? '\\' : '/';
                    const lastSep = currentPath.lastIndexOf(sep);
                    if (lastSep > 0) {
                        defaultPath = currentPath.substring(0, lastSep);
                    }
                }
            }

            const selected = await open({
                multiple: false,
                filters: filters,
                ...(defaultPath ? { defaultPath } : {})
            });

            if (selected) {
                return await openFile(selected);
            }
        } catch (e) {
            console.error('Failed to open file dialog:', e);
        }

        return null;
    }

    // Open a file by path
    async function openFile(filePath) {
        try {
            const result = await invoke('open_file', { path: filePath });
            // Check for valid result - either HTML content (markdown) or output (PDF/other)
            if (result && (result.html || result.output)) {
                handleFileOpened(result);
                // Add to path history (use canonical path from result)
                if (result.file_path) {
                    addToPathHistory(result.file_path);
                }
                return result;
            }
        } catch (e) {
            console.error('Failed to open file:', e);
            showToast('Failed to open file: ' + (e.message || e));
        }
        return null;
    }

    // Check if a string is a URL
    function isUrl(str) {
        return str.startsWith('http://') || str.startsWith('https://');
    }

    // Open a URL
    async function openUrl(url) {
        try {
            showToast('Fetching from URL...');
            const result = await invoke('open_url', { url });
            if (result && result.html) {
                handleUrlOpened(result);
                // Add URL to path history
                if (result.file_path) {
                    addToPathHistory(result.file_path);
                }
                return result;
            }
        } catch (e) {
            const errorStr = '' + e;
            // Check if auth is required (private GitHub repo)
            if (errorStr.indexOf('AUTH_REQUIRED:') !== -1) {
                const message = errorStr.substring(errorStr.indexOf('AUTH_REQUIRED:') + 14);
                showGitHubTokenDialog(url, message);
            } else {
                console.error('Failed to open URL:', e);
                showToast('Failed to open URL: ' + (e.message || e));
            }
        }
        return null;
    }

    // Open a URL with a user-provided GitHub token
    // Returns { success: true, result } or { success: false, error }
    async function openUrlWithToken(url, token) {
        try {
            const result = await invoke('open_url_with_token', { url, token });
            if (result && result.html) {
                handleUrlOpened(result);
                if (result.file_path) {
                    addToPathHistory(result.file_path);
                }
                return { success: true, result };
            }
            return { success: false, error: 'No content returned' };
        } catch (e) {
            console.error('Failed to open URL with token:', e);
            return { success: false, error: '' + (e.message || e) };
        }
    }

    // Show dialog to prompt for GitHub token
    let tokenDialogVisible = false;
    function showGitHubTokenDialog(url, message) {
        if (tokenDialogVisible) return;
        tokenDialogVisible = true;

        const existing = document.getElementById('github-token-dialog');
        if (existing) existing.remove();

        const dialog = document.createElement('div');
        dialog.id = 'github-token-dialog';
        dialog.innerHTML = `
            <div class="github-token-overlay">
                <div class="github-token-container">
                    <div class="github-token-header">
                        <div class="github-token-title">GitHub Authentication Required</div>
                        <div class="github-token-message">${message}</div>
                    </div>
                    <div class="github-token-body">
                        <p>Enter a GitHub personal access token to access this private repository:</p>
                        <input type="password" id="github-token-input" placeholder="ghp_xxxxxxxxxxxxxxxxxxxx" autocomplete="off" spellcheck="false">
                        <p class="github-token-tip">Tip: Set GITHUB_TOKEN or GH_TOKEN environment variable to avoid this prompt.</p>
                    </div>
                    <div class="github-token-footer">
                        <button id="github-token-cancel" class="btn-cancel">Cancel</button>
                        <button id="github-token-submit" class="btn-submit">Fetch with Token</button>
                    </div>
                </div>
            </div>
        `;
        document.body.appendChild(dialog);

        // Add styles if not already present
        if (!document.getElementById('github-token-styles')) {
            const style = document.createElement('style');
            style.id = 'github-token-styles';
            style.textContent = `
                .github-token-overlay {
                    position: fixed;
                    top: 0; left: 0; right: 0; bottom: 0;
                    background: rgba(0, 0, 0, 0.5);
                    display: flex;
                    align-items: flex-start;
                    justify-content: center;
                    padding-top: 15vh;
                    z-index: 10001;
                }
                .github-token-container {
                    background: #fff;
                    border-radius: 8px;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                    width: 480px;
                    max-width: 90vw;
                }
                .github-token-header {
                    padding: 16px;
                    border-bottom: 1px solid #e1e4e8;
                }
                .github-token-title {
                    font-weight: 600;
                    font-size: 16px;
                    margin-bottom: 8px;
                }
                .github-token-message {
                    font-size: 13px;
                    color: #666;
                }
                .github-token-body {
                    padding: 16px;
                }
                .github-token-body p {
                    margin: 0 0 12px 0;
                    font-size: 14px;
                }
                .github-token-body input {
                    width: 100%;
                    padding: 10px 12px;
                    border: 1px solid #d1d5db;
                    border-radius: 6px;
                    font-size: 14px;
                    font-family: ui-monospace, SFMono-Regular, monospace;
                    box-sizing: border-box;
                }
                .github-token-body input:focus {
                    outline: none;
                    border-color: #2563eb;
                    box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.1);
                }
                .github-token-tip {
                    font-size: 12px !important;
                    color: #666 !important;
                    margin-top: 12px !important;
                }
                .github-token-footer {
                    padding: 12px 16px;
                    background: #f6f8fa;
                    display: flex;
                    justify-content: flex-end;
                    gap: 8px;
                    border-radius: 0 0 8px 8px;
                }
                .github-token-footer button {
                    padding: 8px 16px;
                    border-radius: 6px;
                    font-size: 14px;
                    font-weight: 500;
                    cursor: pointer;
                }
                .github-token-footer .btn-cancel {
                    background: #fff;
                    border: 1px solid #d1d5db;
                    color: #374151;
                }
                .github-token-footer .btn-cancel:hover {
                    background: #f3f4f6;
                }
                .github-token-footer .btn-submit {
                    background: #2563eb;
                    border: 1px solid #2563eb;
                    color: #fff;
                }
                .github-token-footer .btn-submit:hover {
                    background: #1d4ed8;
                }
                @media (prefers-color-scheme: dark) {
                    .github-token-container { background: #1e1e1e; }
                    .github-token-header { border-color: #333; }
                    .github-token-title { color: #fff; }
                    .github-token-message { color: #999; }
                    .github-token-body { color: #fff; }
                    .github-token-body input { background: #2d2d2d; border-color: #444; color: #fff; }
                    .github-token-tip { color: #999 !important; }
                    .github-token-footer { background: #252525; }
                    .github-token-footer .btn-cancel { background: #333; border-color: #444; color: #fff; }
                }
            `;
            document.head.appendChild(style);
        }

        const input = document.getElementById('github-token-input');
        const cancelBtn = document.getElementById('github-token-cancel');
        const submitBtn = document.getElementById('github-token-submit');
        const messageEl = dialog.querySelector('.github-token-message');

        function closeDialog() {
            tokenDialogVisible = false;
            dialog.remove();
        }

        function setError(errorMsg) {
            messageEl.textContent = errorMsg;
            messageEl.style.color = '#dc2626';
            input.style.borderColor = '#dc2626';
        }

        function setLoading(loading) {
            submitBtn.disabled = loading;
            submitBtn.textContent = loading ? 'Fetching...' : 'Fetch with Token';
            input.disabled = loading;
        }

        async function submitToken() {
            const token = input.value.trim();
            if (!token) {
                input.focus();
                setError('Please enter a token');
                return;
            }

            setLoading(true);
            const { success, error } = await openUrlWithToken(url, token);
            setLoading(false);

            if (success) {
                closeDialog();
            } else {
                setError(error || 'Failed to fetch. Please check your token.');
                input.select();
            }
        }

        cancelBtn.onclick = closeDialog;
        submitBtn.onclick = submitToken;

        input.onkeydown = (e) => {
            if (e.key === 'Enter') {
                e.preventDefault();
                submitToken();
            } else if (e.key === 'Escape') {
                e.preventDefault();
                closeDialog();
            }
        };

        // Click outside to close
        dialog.querySelector('.github-token-overlay').onclick = (e) => {
            if (e.target.classList.contains('github-token-overlay')) {
                closeDialog();
            }
        };

        input.focus();
    }

    // Handle URL opened result (similar to handleFileOpened but no file watching)
    function handleUrlOpened(result) {
        const content = document.getElementById('content');
        content.innerHTML = result.html;

        codeHighlight();
        renderMermaid();
        renderLatex();
        addHeadingAnchors();
        generateTOC();

        if (result.file_path) {
            updateFilePathBar(result.file_path, null);
            // Extract filename from URL for title
            try {
                const urlPath = new URL(result.file_path).pathname;
                const fileName = urlPath.split('/').pop() || 'Remote Markdown';
                document.title = fileName + ' - Markdown Preview';
            } catch {
                document.title = 'Remote Markdown - Markdown Preview';
            }
        }

        if (result.stats) {
            updateDocumentStats(result.stats);
        }

        // Update metadata bar (no modification time or git info for URLs)
        updateFileMetadata(null, result.stats, null, null, null);

        showToast('Loaded from URL');
    }

    // Open a path or URL (auto-detects)
    async function openPathOrUrl(input) {
        if (isUrl(input)) {
            return await openUrl(input);
        } else {
            return await openFile(input);
        }
    }

    // Remove a file from the backend recent files list
    async function removeRecentFileFromBackend(filePath) {
        try {
            await invoke('remove_recent_file', { path: filePath });
        } catch (e) {
            console.error('Failed to remove from backend:', e);
        }
    }

    // Load recent files from backend and render them
    async function loadRecentFilesFromBackend() {
        try {
            const files = await invoke('get_recent_files');
            // Convert backend format to the format expected by renderRecentFiles
            const recentFiles = files.map(path => ({ path, timestamp: Date.now() }));
            // Store in localStorage so renderRecentFiles can use it
            localStorage.setItem('recentFiles', JSON.stringify(recentFiles));
            renderRecentFiles(switchToFile, removeRecentFileFromBackend);
        } catch (e) {
            console.error('Failed to load recent files from backend:', e);
        }
    }

    // Handle file opened result
    function handleFileOpened(result) {
        const content = document.getElementById('content');

        // Get document type (required field from backend)
        const docType = result.document_type;
        const output = result.output;

        // Clean up PDF viewer if switching away from PDF
        if (docType !== 'pdf' && window.PdfViewer && window.PdfViewer.isActive()) {
            window.PdfViewer.cleanup();
        }

        if (docType === 'pdf' && output?.type === 'file_url') {
            // PDF: Use PDF.js viewer, reading file via Tauri fs plugin
            if (window.PdfViewer) {
                window.PdfViewer.onStatsUpdate = (stats) => {
                    updateDocumentStatsForType(stats, 'pdf');
                };

                // Open PDF with PDF.js viewer (pass file path, not URL)
                window.PdfViewer.open(output.path).catch(err => {
                    console.error('Failed to open PDF:', err);
                    content.innerHTML = `<div class="error" style="padding: 40px; text-align: center; color: #cf222e;">
                        <h2>Failed to load PDF</h2>
                        <p>${err.message || err}</p>
                    </div>`;
                });
            } else {
                console.error('PdfViewer not available');
                content.innerHTML = `<div class="error" style="padding: 40px; text-align: center; color: #cf222e;">
                    <h2>PDF viewer not available</h2>
                    <p>PDF.js failed to load</p>
                </div>`;
            }
        } else if (docType === 'markdown' || !docType) {
            // Markdown: use output.content if available, fallback to html for legacy
            const html = output?.type === 'html' ? output.content : result.html;
            content.innerHTML = html;

            codeHighlight();
            renderMermaid();
            renderLatex();
            addHeadingAnchors();
            generateTOC();

            if (result.stats) {
                updateDocumentStatsForType(result.stats, docType || 'markdown');
            }
        } else {
            // Unknown document type
            content.innerHTML = `<div class="error" style="padding: 40px; text-align: center; color: #cf222e;">
                <h2>Unsupported document type: ${docType}</h2>
            </div>`;
        }

        if (result.file_path) {
            // Update currentFilePath via the core module's setter
            window.MarkdownPreviewCore.setCurrentFilePath(result.file_path);
            updateFilePathBar(result.file_path, result.git_root);
            document.title = getFileBasename(result.file_path) + ' - Markdown Preview';

            // Refresh recent files from backend (backend already added the file)
            loadRecentFilesFromBackend();

            // Start watching the file for changes
            invoke('watch_file', { path: result.file_path }).catch(e => {
                console.error('Failed to watch file:', e);
            });
        }

        // Update metadata bar
        updateFileMetadata(result.modified_at, result.stats, result.git_branch, result.git_branch_url, result.git_last_author);
    }

    // Update document stats display based on document type
    function updateDocumentStatsForType(stats, docType) {
        if (!stats) return;

        if (docType === 'pdf') {
            // PDF-specific display
            const wordCountEl = document.getElementById('word-count');
            const metadataWords = document.getElementById('metadata-words');
            if (stats.pages) {
                wordCountEl.textContent = `${stats.pages} pages`;
                if (metadataWords) metadataWords.title = 'Page count';
            } else {
                wordCountEl.textContent = '-';
            }
            // Update reading time
            const readTimeEl = document.getElementById('read-time');
            if (stats.reading_minutes) {
                readTimeEl.textContent = `~${stats.reading_minutes} min`;
            } else {
                readTimeEl.textContent = '-';
            }
        } else {
            // Markdown display (existing logic)
            updateDocumentStats(stats);
        }
    }

    // Switch to a different file
    async function switchToFile(filePath) {
        const current = window.MarkdownPreviewCore.getCurrentFilePath();
        if (filePath === current) {
            return;
        }

        await openFile(filePath);
        console.log(`Switched to: ${filePath}`);
    }

    // Set up Tauri event listeners
    function setupTauriListeners() {
        // Listen for file-changed events from Rust (file watcher)
        listen('file-changed', (event) => {
            console.log('File changed:', event.payload);
            handleFileOpened(event.payload);
            // Trigger the shimmer animation on the metadata bar
            triggerFileChangedAnimation();
        });

        // Listen for menu events
        listen('menu-open', async () => {
            await openFileDialog();
        });

        listen('menu-reload', async () => {
            const current = window.MarkdownPreviewCore.getCurrentFilePath();
            if (current) {
                await openFile(current);
            }
        });

        listen('menu-toc', (event) => {
            // TOC controlled via View menu: off, left, right
            toggleTOC(event.payload);
        });

        listen('menu-theme', (event) => {
            // Theme controlled via Theme menu
            const themeMap = { 'light': 'github-light', 'dark': 'github-dark', 'auto': 'auto' };
            const theme = themeMap[event.payload] || event.payload;
            changeTheme(theme);
        });

        listen('menu-open-path', () => {
            openPathInput();
        });

        // Listen for initial file from command line argument
        listen('open-initial-file', async (event) => {
            console.log('Opening initial file:', event.payload);
            await openFile(event.payload);
        });
    }

    // Vim navigation state
    let lastKeyTime = 0;
    let lastKey = '';

    // Check if focus is in an input element
    function isInInputField() {
        const active = document.activeElement;
        return active && (
            active.tagName === 'INPUT' ||
            active.tagName === 'TEXTAREA' ||
            active.isContentEditable
        );
    }

    // Set up vim-style navigation
    function setupVimNavigation() {
        const mainContent = document.getElementById('main-content');

        document.addEventListener('keydown', (e) => {
            // Skip if in input field, modal open, or modifier keys pressed
            if (isInInputField() || pathInputVisible || e.ctrlKey || e.metaKey || e.altKey) {
                return;
            }

            // Skip if fuzzy finder is open
            const fuzzyFinder = document.getElementById('fuzzy-finder');
            if (fuzzyFinder && fuzzyFinder.classList.contains('visible')) {
                return;
            }

            const now = Date.now();
            const scrollContainer = mainContent;
            const scrollAmount = scrollContainer.clientHeight;

            switch (e.key) {
                // j - scroll down a little
                case 'j':
                    e.preventDefault();
                    scrollContainer.scrollBy({ top: 60, behavior: 'smooth' });
                    break;

                // k - scroll up a little
                case 'k':
                    e.preventDefault();
                    scrollContainer.scrollBy({ top: -60, behavior: 'smooth' });
                    break;

                // d - scroll down half page (like Ctrl+d in vim)
                case 'd':
                    e.preventDefault();
                    scrollContainer.scrollBy({ top: scrollAmount / 2, behavior: 'smooth' });
                    break;

                // u - scroll up half page (like Ctrl+u in vim)
                case 'u':
                    e.preventDefault();
                    scrollContainer.scrollBy({ top: -scrollAmount / 2, behavior: 'smooth' });
                    break;

                // f - scroll down full page (like Ctrl+f in vim)
                case 'f':
                    e.preventDefault();
                    scrollContainer.scrollBy({ top: scrollAmount - 50, behavior: 'smooth' });
                    break;

                // b - scroll up full page (like Ctrl+b in vim)
                case 'b':
                    e.preventDefault();
                    scrollContainer.scrollBy({ top: -(scrollAmount - 50), behavior: 'smooth' });
                    break;

                // G - go to bottom
                case 'G':
                    e.preventDefault();
                    scrollContainer.scrollTo({ top: scrollContainer.scrollHeight, behavior: 'smooth' });
                    break;

                // g - if gg (double g within 500ms), go to top
                case 'g':
                    e.preventDefault();
                    if (lastKey === 'g' && (now - lastKeyTime) < 500) {
                        scrollContainer.scrollTo({ top: 0, behavior: 'smooth' });
                        lastKey = '';
                    }
                    break;

                // / - open fuzzy finder (search)
                case '/':
                    e.preventDefault();
                    if (typeof openFuzzyFinder === 'function') {
                        openFuzzyFinder();
                    }
                    break;

                // n - next heading
                case 'n':
                    e.preventDefault();
                    navigateHeading(1);
                    break;

                // N - previous heading
                case 'N':
                    e.preventDefault();
                    navigateHeading(-1);
                    break;

                // ? - show help
                case '?':
                    e.preventDefault();
                    showHelpModal();
                    break;
            }

            lastKey = e.key;
            lastKeyTime = now;
        });
    }

    // Help modal state
    let helpModalVisible = false;

    // Show help modal with keybindings
    function showHelpModal() {
        if (helpModalVisible) {
            closeHelpModal();
            return;
        }
        helpModalVisible = true;

        let modal = document.getElementById('help-modal');
        if (!modal) {
            modal = document.createElement('div');
            modal.id = 'help-modal';
            modal.className = 'help-modal-overlay';
            modal.innerHTML = `
                <div class="help-modal-container">
                    <div class="help-modal-header">
                        <h3>Keyboard Shortcuts</h3>
                        <button class="help-modal-close">&times;</button>
                    </div>
                    <div class="help-modal-content">
                        <div class="help-section">
                            <h4>Navigation</h4>
                            <div class="help-row"><kbd>j</kbd> / <kbd>k</kbd><span>Scroll down / up</span></div>
                            <div class="help-row"><kbd>d</kbd> / <kbd>u</kbd><span>Half page down / up</span></div>
                            <div class="help-row"><kbd>f</kbd> / <kbd>b</kbd><span>Full page down / up</span></div>
                            <div class="help-row"><kbd>G</kbd><span>Go to bottom</span></div>
                            <div class="help-row"><kbd>gg</kbd><span>Go to top</span></div>
                            <div class="help-row"><kbd>n</kbd> / <kbd>N</kbd><span>Next / previous heading</span></div>
                        </div>
                        <div class="help-section">
                            <h4>File Operations</h4>
                            <div class="help-row"><kbd>Ctrl+O</kbd><span>Open file by path</span></div>
                            <div class="help-row"><kbd>Ctrl+Shift+O</kbd><span>Open file dialog</span></div>
                            <div class="help-row"><kbd>Ctrl+R</kbd> / <kbd>F5</kbd><span>Reload current file</span></div>
                        </div>
                        <div class="help-section">
                            <h4>Search & View</h4>
                            <div class="help-row"><kbd>/</kbd><span>Open fuzzy finder</span></div>
                            <div class="help-row"><kbd>Ctrl++</kbd> / <kbd>Ctrl+-</kbd><span>Zoom in / out</span></div>
                            <div class="help-row"><kbd>Ctrl+0</kbd><span>Reset zoom</span></div>
                        </div>
                        <div class="help-section">
                            <h4>Path Input (Ctrl+O)</h4>
                            <div class="help-row"><kbd>↑</kbd> / <kbd>↓</kbd><span>Navigate completions</span></div>
                            <div class="help-row"><kbd>Ctrl+k</kbd> / <kbd>Ctrl+j</kbd><span>Browse path history</span></div>
                            <div class="help-row"><kbd>Tab</kbd><span>Accept completion</span></div>
                        </div>
                        <div class="help-section">
                            <h4>Other</h4>
                            <div class="help-row"><kbd>?</kbd><span>Show this help</span></div>
                            <div class="help-row"><kbd>Esc</kbd><span>Close modal / cancel</span></div>
                            <div class="help-row"><kbd>Ctrl+Q</kbd><span>Quit application</span></div>
                        </div>
                    </div>
                </div>
            `;
            document.body.appendChild(modal);

            // Add styles
            const style = document.createElement('style');
            style.id = 'help-modal-styles';
            style.textContent = `
                .help-modal-overlay {
                    position: fixed;
                    top: 0;
                    left: 0;
                    right: 0;
                    bottom: 0;
                    background: rgba(0, 0, 0, 0.5);
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    z-index: 10000;
                    opacity: 0;
                    visibility: hidden;
                    transition: opacity 0.15s, visibility 0.15s;
                }
                .help-modal-overlay.visible {
                    opacity: 1;
                    visibility: visible;
                }
                .help-modal-container {
                    background: #fff;
                    border-radius: 12px;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                    width: 520px;
                    max-width: 90vw;
                    max-height: 80vh;
                    overflow: hidden;
                    display: flex;
                    flex-direction: column;
                }
                .help-modal-header {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    padding: 16px 20px;
                    border-bottom: 1px solid #e1e4e8;
                }
                .help-modal-header h3 {
                    margin: 0;
                    font-size: 16px;
                    font-weight: 600;
                }
                .help-modal-close {
                    background: none;
                    border: none;
                    font-size: 24px;
                    cursor: pointer;
                    color: #666;
                    padding: 0;
                    line-height: 1;
                }
                .help-modal-close:hover {
                    color: #333;
                }
                .help-modal-content {
                    padding: 16px 20px;
                    overflow-y: auto;
                    display: grid;
                    grid-template-columns: 1fr 1fr;
                    gap: 16px;
                }
                .help-section {
                    margin-bottom: 8px;
                }
                .help-section h4 {
                    margin: 0 0 8px 0;
                    font-size: 12px;
                    font-weight: 600;
                    color: #666;
                    text-transform: uppercase;
                    letter-spacing: 0.5px;
                }
                .help-row {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    padding: 4px 0;
                    font-size: 13px;
                }
                .help-row kbd {
                    background: #f0f0f0;
                    padding: 2px 6px;
                    border-radius: 4px;
                    font-family: ui-monospace, SFMono-Regular, monospace;
                    font-size: 12px;
                    border: 1px solid #ddd;
                    min-width: 20px;
                    text-align: center;
                }
                .help-row span {
                    color: #555;
                    text-align: right;
                }
                @media (max-width: 500px) {
                    .help-modal-content {
                        grid-template-columns: 1fr;
                    }
                }
                /* Dark mode styles */
                @media (prefers-color-scheme: dark) {
                    .help-modal-container { background: #1c1c1e; }
                    .help-modal-header { border-color: #3a3a3c; }
                    .help-modal-header h3 { color: #fff; }
                    .help-modal-close { color: #888; }
                    .help-modal-close:hover { color: #fff; }
                    .help-section h4 { color: #888; }
                    .help-row span { color: #aaa; }
                    .help-row kbd { background: #2c2c2e; border-color: #3a3a3c; color: #fff; }
                }
                body.theme-github-dark .help-modal-container,
                body.theme-dark .help-modal-container,
                body.theme-material-dark .help-modal-container,
                body.theme-one-dark .help-modal-container {
                    background: #1c1c1e;
                }
                body.theme-github-dark .help-modal-header,
                body.theme-dark .help-modal-header,
                body.theme-material-dark .help-modal-header,
                body.theme-one-dark .help-modal-header {
                    border-color: #3a3a3c;
                }
                body.theme-github-dark .help-modal-header h3,
                body.theme-dark .help-modal-header h3,
                body.theme-material-dark .help-modal-header h3,
                body.theme-one-dark .help-modal-header h3 {
                    color: #fff;
                }
                body.theme-github-dark .help-section h4,
                body.theme-dark .help-section h4,
                body.theme-material-dark .help-section h4,
                body.theme-one-dark .help-section h4 {
                    color: #888;
                }
                body.theme-github-dark .help-row span,
                body.theme-dark .help-row span,
                body.theme-material-dark .help-row span,
                body.theme-one-dark .help-row span {
                    color: #aaa;
                }
                body.theme-github-dark .help-row kbd,
                body.theme-dark .help-row kbd,
                body.theme-material-dark .help-row kbd,
                body.theme-one-dark .help-row kbd {
                    background: #2c2c2e;
                    border-color: #3a3a3c;
                    color: #fff;
                }
            `;
            document.head.appendChild(style);

            // Click outside to close
            modal.addEventListener('click', (e) => {
                if (e.target === modal) {
                    closeHelpModal();
                }
            });

            // Close button
            modal.querySelector('.help-modal-close').addEventListener('click', closeHelpModal);

            // Escape key to close
            document.addEventListener('keydown', function helpEscHandler(e) {
                if (e.key === 'Escape' && helpModalVisible) {
                    closeHelpModal();
                }
            });
        }

        modal.classList.add('visible');
    }

    // Close help modal
    function closeHelpModal() {
        helpModalVisible = false;
        const modal = document.getElementById('help-modal');
        if (modal) {
            modal.classList.remove('visible');
        }
    }

    // Navigate to next/previous heading
    function navigateHeading(direction) {
        const content = document.getElementById('content');
        if (!content) return;

        const headings = Array.from(content.querySelectorAll('h1, h2, h3, h4, h5, h6'));
        if (headings.length === 0) return;

        const mainContent = document.getElementById('main-content');
        const scrollTop = mainContent.scrollTop;
        const buffer = 10; // Small buffer for current position detection

        let currentIndex = -1;
        for (let i = 0; i < headings.length; i++) {
            const headingTop = headings[i].offsetTop;
            if (headingTop <= scrollTop + buffer) {
                currentIndex = i;
            } else {
                break;
            }
        }

        let targetIndex;
        if (direction > 0) {
            // Next heading
            targetIndex = Math.min(currentIndex + 1, headings.length - 1);
        } else {
            // Previous heading
            targetIndex = Math.max(currentIndex - 1, 0);
        }

        if (targetIndex >= 0 && targetIndex < headings.length) {
            headings[targetIndex].scrollIntoView({ behavior: 'smooth', block: 'start' });
        }
    }

    // Set up drag and drop
    function setupDragAndDrop() {
        document.addEventListener('drop', async (e) => {
            e.preventDefault();
            const files = e.dataTransfer?.files;
            if (files && files.length > 0) {
                const file = files[0];
                if (await isSupported(file.name)) {
                    const path = file.path || file.name;
                    await openFile(path);
                } else {
                    showToast('Unsupported file type');
                }
            }
        });

        document.addEventListener('dragover', (e) => {
            e.preventDefault();
        });
    }

    // Set up welcome screen
    function setupWelcomeScreen() {
        const welcomeOpenBtn = document.getElementById('welcome-open-btn');
        if (welcomeOpenBtn) {
            welcomeOpenBtn.addEventListener('click', async () => {
                console.log('Open File button clicked');
                await openFileDialog();
            });
        }
    }

    // Set up keyboard shortcuts
    function setupKeyboardShortcuts() {
        document.addEventListener('keydown', async (e) => {
            // Cmd+O / Ctrl+O to open file by path (with autocomplete)
            if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key === 'o') {
                e.preventDefault();
                openPathInput();
            }

            // Cmd+Shift+O / Ctrl+Shift+O to open file dialog
            if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'O' || e.key === 'o')) {
                e.preventDefault();
                await openFileDialog();
            }

            // F5 or Cmd+R / Ctrl+R to refresh
            if (e.key === 'F5' || ((e.ctrlKey || e.metaKey) && e.key === 'r')) {
                e.preventDefault();
                const current = window.MarkdownPreviewCore.getCurrentFilePath();
                if (current) {
                    await openFile(current);
                    showToast('Refreshed');
                }
            }

            // Escape to close path input
            if (e.key === 'Escape' && pathInputVisible) {
                closePathInput();
            }

            // Cmd+Q / Ctrl+Q to quit the app
            if ((e.ctrlKey || e.metaKey) && e.key === 'q') {
                e.preventDefault();
                const { getCurrentWindow } = window.__TAURI__.window;
                getCurrentWindow().close();
            }

            // Cmd+C / Ctrl+C to copy selected text
            if ((e.ctrlKey || e.metaKey) && e.key === 'c') {
                const selection = window.getSelection();
                const selectedText = selection ? selection.toString() : '';
                if (selectedText) {
                    e.preventDefault();
                    try {
                        const clipboardApi = window.__TAURI__.clipboard
                            || window.__TAURI__.clipboardManager
                            || window.__TAURI__.plugin?.clipboardManager;
                        if (clipboardApi && clipboardApi.writeText) {
                            await clipboardApi.writeText(selectedText);
                        }
                    } catch (err) {
                        console.error('Failed to copy:', err);
                    }
                }
            }
        });
    }

    // Path input state
    let pathInputVisible = false;
    let autocompleteState = {
        items: [],
        selectedIndex: -1,
        debounceTimer: null
    };

    // Path history state
    let pathHistoryState = {
        items: [],           // History items from frecency
        currentIndex: -1,    // Current position in history (-1 = not browsing)
        originalValue: '',   // Value before starting history navigation
        currentGitRoot: null // Git root of current file
    };

    // Fetch path completions from backend
    async function fetchCompletions(partial) {
        try {
            return await invoke('complete_path', { partial });
        } catch (e) {
            console.error('Failed to fetch completions:', e);
            return [];
        }
    }

    // Render autocomplete dropdown
    function renderAutocomplete(items) {
        const dropdown = document.getElementById('path-autocomplete');
        if (!dropdown) return;

        autocompleteState.items = items;
        autocompleteState.selectedIndex = items.length > 0 ? 0 : -1;

        if (items.length === 0) {
            dropdown.style.display = 'none';
            return;
        }

        dropdown.innerHTML = items.map((item, index) => `
            <div class="autocomplete-item ${index === 0 ? 'selected' : ''}" data-index="${index}">
                <span class="autocomplete-icon">${item.is_dir ? '📁' : '📄'}</span>
                <span class="autocomplete-name">${item.name}</span>
            </div>
        `).join('');

        dropdown.style.display = 'block';

        // Add click handlers
        dropdown.querySelectorAll('.autocomplete-item').forEach(el => {
            el.addEventListener('click', () => {
                const index = parseInt(el.dataset.index);
                selectAutocompleteItem(index);
            });
        });
    }

    // Select an autocomplete item
    function selectAutocompleteItem(index) {
        const item = autocompleteState.items[index];
        if (!item) return;

        const input = document.getElementById('path-input-field');
        input.value = item.path;
        input.focus();

        // If it's a directory, trigger another completion
        if (item.is_dir) {
            triggerAutocomplete(item.path);
        } else {
            // Hide dropdown for files
            const dropdown = document.getElementById('path-autocomplete');
            if (dropdown) dropdown.style.display = 'none';
        }
    }

    // Update selection highlight
    function updateAutocompleteSelection() {
        const dropdown = document.getElementById('path-autocomplete');
        if (!dropdown) return;

        dropdown.querySelectorAll('.autocomplete-item').forEach((el, index) => {
            el.classList.toggle('selected', index === autocompleteState.selectedIndex);
        });

        // Scroll selected item into view
        const selected = dropdown.querySelector('.autocomplete-item.selected');
        if (selected) {
            selected.scrollIntoView({ block: 'nearest' });
        }
    }

    // Trigger autocomplete with debouncing
    function triggerAutocomplete(value) {
        if (autocompleteState.debounceTimer) {
            clearTimeout(autocompleteState.debounceTimer);
        }

        // Reset history navigation when user types
        pathHistoryState.currentIndex = -1;

        autocompleteState.debounceTimer = setTimeout(async () => {
            if (value.length > 0) {
                const items = await fetchCompletions(value);
                renderAutocomplete(items);
            } else {
                const dropdown = document.getElementById('path-autocomplete');
                if (dropdown) dropdown.style.display = 'none';
            }
        }, 100);
    }

    // Load path history from backend
    async function loadPathHistory() {
        try {
            const gitRoot = pathHistoryState.currentGitRoot;
            pathHistoryState.items = await invoke('get_path_history', { gitRoot });
        } catch (e) {
            console.error('Failed to load path history:', e);
            pathHistoryState.items = [];
        }
    }

    // Add path to history
    async function addToPathHistory(path) {
        try {
            await invoke('add_path_to_history', { path });
        } catch (e) {
            console.error('Failed to add path to history:', e);
        }
    }

    // Navigate to previous history item
    function historyPrev(input) {
        if (pathHistoryState.items.length === 0) return false;

        // Save original value when starting navigation
        if (pathHistoryState.currentIndex === -1) {
            pathHistoryState.originalValue = input.value;
        }

        // Move to previous (older) item
        if (pathHistoryState.currentIndex < pathHistoryState.items.length - 1) {
            pathHistoryState.currentIndex++;
            input.value = pathHistoryState.items[pathHistoryState.currentIndex];
            return true;
        }
        return false;
    }

    // Navigate to next history item
    function historyNext(input) {
        if (pathHistoryState.currentIndex === -1) return false;

        pathHistoryState.currentIndex--;

        if (pathHistoryState.currentIndex === -1) {
            // Back to original value
            input.value = pathHistoryState.originalValue;
        } else {
            input.value = pathHistoryState.items[pathHistoryState.currentIndex];
        }
        return true;
    }

    // Open path input modal
    async function openPathInput() {
        if (pathInputVisible) return;
        pathInputVisible = true;

        // Get current git root for path history boost and default directory
        try {
            pathHistoryState.currentGitRoot = await invoke('get_current_git_root');
        } catch (e) {
            pathHistoryState.currentGitRoot = null;
        }

        // Load path history
        await loadPathHistory();

        // Create modal if it doesn't exist
        let modal = document.getElementById('path-input-modal');
        if (!modal) {
            modal = document.createElement('div');
            modal.id = 'path-input-modal';
            modal.className = 'path-input-overlay';
            modal.innerHTML = `
                <div class="path-input-container">
                    <div class="path-input-header">
                        <label>Open file or URL</label>
                        <span class="path-input-hint">Type path to autocomplete, or paste a GitHub URL</span>
                    </div>
                    <div class="path-input-wrapper">
                        <input type="text" id="path-input-field" class="path-input-field"
                               placeholder="/path/to/file.md or https://github.com/..." autocomplete="off" spellcheck="false">
                        <div id="path-autocomplete" class="path-autocomplete"></div>
                    </div>
                    <div class="path-input-footer">
                        <span class="key">↑↓</span> navigate
                        <span class="key">Ctrl+↑↓</span> history
                        <span class="key">Tab</span> complete
                        <span class="key">Enter</span> open
                        <span class="key">Esc</span> cancel
                    </div>
                </div>
            `;
            document.body.appendChild(modal);

            // Add styles
            const style = document.createElement('style');
            style.textContent = `
                .path-input-overlay {
                    position: fixed;
                    top: 0;
                    left: 0;
                    right: 0;
                    bottom: 0;
                    background: rgba(0, 0, 0, 0.5);
                    display: flex;
                    align-items: flex-start;
                    justify-content: center;
                    padding-top: 15vh;
                    z-index: 10000;
                    opacity: 0;
                    visibility: hidden;
                    transition: opacity 0.15s, visibility 0.15s;
                }
                .path-input-overlay.visible {
                    opacity: 1;
                    visibility: visible;
                }
                .path-input-container {
                    background: #fff;
                    border-radius: 8px;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                    width: 600px;
                    max-width: 90vw;
                    overflow: hidden;
                }
                .path-input-header {
                    padding: 12px 16px;
                    border-bottom: 1px solid #e1e4e8;
                }
                .path-input-header label {
                    font-weight: 600;
                    font-size: 14px;
                }
                .path-input-hint {
                    display: block;
                    font-size: 12px;
                    color: #666;
                    margin-top: 4px;
                }
                .path-input-wrapper {
                    position: relative;
                }
                .path-input-field {
                    width: 100%;
                    padding: 12px 16px;
                    border: none;
                    font-size: 14px;
                    font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace;
                    outline: none;
                    box-sizing: border-box;
                }
                .path-autocomplete {
                    display: none;
                    max-height: 240px;
                    overflow-y: auto;
                    border-top: 1px solid #e1e4e8;
                }
                .autocomplete-item {
                    padding: 8px 16px;
                    cursor: pointer;
                    display: flex;
                    align-items: center;
                    gap: 8px;
                    font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace;
                    font-size: 13px;
                }
                .autocomplete-item:hover,
                .autocomplete-item.selected {
                    background: #f0f6ff;
                }
                .autocomplete-icon {
                    font-size: 14px;
                    width: 20px;
                    text-align: center;
                }
                .autocomplete-name {
                    overflow: hidden;
                    text-overflow: ellipsis;
                    white-space: nowrap;
                }
                .path-input-footer {
                    padding: 8px 16px;
                    background: #f6f8fa;
                    font-size: 12px;
                    color: #666;
                }
                .path-input-footer .key {
                    background: #e1e4e8;
                    padding: 2px 6px;
                    border-radius: 3px;
                    font-family: ui-monospace, SFMono-Regular, monospace;
                    margin-right: 4px;
                }
                @media (prefers-color-scheme: dark) {
                    .path-input-container { background: #1c1c1e; }
                    .path-input-header { border-color: #3a3a3c; }
                    .path-input-header label { color: #fff; }
                    .path-input-hint { color: #98989f; }
                    .path-input-field { background: #1c1c1e; color: #fff; }
                    .path-autocomplete { border-color: #3a3a3c; }
                    .autocomplete-item { color: #fff; }
                    .autocomplete-item:hover,
                    .autocomplete-item.selected { background: #2c3e50; }
                    .path-input-footer { background: #2c2c2e; }
                    .path-input-footer .key { background: #3a3a3c; color: #fff; }
                }
                body.theme-github-dark .path-input-container,
                body.theme-dark .path-input-container,
                body.theme-material-dark .path-input-container,
                body.theme-one-dark .path-input-container {
                    background: #1c1c1e;
                }
                body.theme-github-dark .path-input-header,
                body.theme-dark .path-input-header,
                body.theme-material-dark .path-input-header,
                body.theme-one-dark .path-input-header {
                    border-color: #3a3a3c;
                }
                body.theme-github-dark .path-input-header label,
                body.theme-dark .path-input-header label,
                body.theme-material-dark .path-input-header label,
                body.theme-one-dark .path-input-header label {
                    color: #fff;
                }
                body.theme-github-dark .path-input-field,
                body.theme-dark .path-input-field,
                body.theme-material-dark .path-input-field,
                body.theme-one-dark .path-input-field {
                    background: #1c1c1e;
                    color: #fff;
                }
                body.theme-github-dark .path-autocomplete,
                body.theme-dark .path-autocomplete,
                body.theme-material-dark .path-autocomplete,
                body.theme-one-dark .path-autocomplete {
                    border-color: #3a3a3c;
                }
                body.theme-github-dark .autocomplete-item,
                body.theme-dark .autocomplete-item,
                body.theme-material-dark .autocomplete-item,
                body.theme-one-dark .autocomplete-item {
                    color: #fff;
                }
                body.theme-github-dark .autocomplete-item:hover,
                body.theme-github-dark .autocomplete-item.selected,
                body.theme-dark .autocomplete-item:hover,
                body.theme-dark .autocomplete-item.selected,
                body.theme-material-dark .autocomplete-item:hover,
                body.theme-material-dark .autocomplete-item.selected,
                body.theme-one-dark .autocomplete-item:hover,
                body.theme-one-dark .autocomplete-item.selected {
                    background: #2c3e50;
                }
                body.theme-github-dark .path-input-footer,
                body.theme-dark .path-input-footer,
                body.theme-material-dark .path-input-footer,
                body.theme-one-dark .path-input-footer {
                    background: #2c2c2e;
                }
            `;
            document.head.appendChild(style);

            // Click outside to close
            modal.addEventListener('click', (e) => {
                if (e.target === modal) {
                    closePathInput();
                }
            });

            // Handle input events
            const input = document.getElementById('path-input-field');

            // Input change - trigger autocomplete
            input.addEventListener('input', (e) => {
                triggerAutocomplete(e.target.value);
            });

            // Keyboard navigation
            input.addEventListener('keydown', async (e) => {
                const dropdown = document.getElementById('path-autocomplete');
                const isDropdownVisible = dropdown && dropdown.style.display !== 'none';

                // Ctrl+k/j: Navigate path history (Ctrl+Up/Down conflicts with macOS)
                if (e.ctrlKey && e.key === 'k') {
                    e.preventDefault();
                    historyPrev(input);
                    return;
                } else if (e.ctrlKey && e.key === 'j') {
                    e.preventDefault();
                    historyNext(input);
                    return;
                }

                if (e.key === 'ArrowDown') {
                    e.preventDefault();
                    if (isDropdownVisible && autocompleteState.items.length > 0) {
                        autocompleteState.selectedIndex = Math.min(
                            autocompleteState.selectedIndex + 1,
                            autocompleteState.items.length - 1
                        );
                        updateAutocompleteSelection();
                    }
                } else if (e.key === 'ArrowUp') {
                    e.preventDefault();
                    if (isDropdownVisible && autocompleteState.items.length > 0) {
                        autocompleteState.selectedIndex = Math.max(
                            autocompleteState.selectedIndex - 1,
                            0
                        );
                        updateAutocompleteSelection();
                    }
                } else if (e.key === 'Tab') {
                    e.preventDefault();
                    if (isDropdownVisible && autocompleteState.selectedIndex >= 0) {
                        selectAutocompleteItem(autocompleteState.selectedIndex);
                    }
                } else if (e.key === 'Enter') {
                    e.preventDefault();
                    // If dropdown visible and item selected, use that item
                    if (isDropdownVisible && autocompleteState.selectedIndex >= 0) {
                        const item = autocompleteState.items[autocompleteState.selectedIndex];
                        if (item && !item.is_dir) {
                            closePathInput();
                            await openFile(item.path);
                            return;
                        } else if (item && item.is_dir) {
                            selectAutocompleteItem(autocompleteState.selectedIndex);
                            return;
                        }
                    }
                    // Otherwise use the input value (could be path or URL)
                    const inputValue = input.value.trim();
                    if (inputValue) {
                        closePathInput();
                        await openPathOrUrl(inputValue);
                    }
                } else if (e.key === 'Escape') {
                    if (isDropdownVisible) {
                        dropdown.style.display = 'none';
                    } else {
                        closePathInput();
                    }
                } else if ((e.ctrlKey || e.metaKey) && e.key === 'v') {
                    // Handle paste using Tauri clipboard API
                    e.preventDefault();
                    try {
                        const clipboardApi = window.__TAURI__.clipboard
                            || window.__TAURI__.clipboardManager
                            || window.__TAURI__.plugin?.clipboardManager;
                        if (clipboardApi && clipboardApi.readText) {
                            const text = await clipboardApi.readText();
                            if (text) {
                                const start = input.selectionStart;
                                const end = input.selectionEnd;
                                const before = input.value.substring(0, start);
                                const after = input.value.substring(end);
                                input.value = before + text + after;
                                input.selectionStart = input.selectionEnd = start + text.length;
                                triggerAutocomplete(input.value);
                            }
                        }
                    } catch (err) {
                        console.error('Failed to paste:', err);
                    }
                }
            });
        }

        modal.classList.add('visible');
        const input = document.getElementById('path-input-field');

        // Pre-fill with git root directory if available
        if (pathHistoryState.currentGitRoot) {
            input.value = pathHistoryState.currentGitRoot + '/';
        } else {
            input.value = '';
        }

        // Reset state
        autocompleteState.items = [];
        autocompleteState.selectedIndex = -1;
        pathHistoryState.currentIndex = -1;
        pathHistoryState.originalValue = input.value;

        const dropdown = document.getElementById('path-autocomplete');
        if (dropdown) dropdown.style.display = 'none';

        setTimeout(() => {
            input.focus();
            // Trigger autocomplete if we have a pre-filled value
            if (input.value) {
                triggerAutocomplete(input.value);
            }
        }, 50);
    }

    // Close path input modal
    function closePathInput() {
        pathInputVisible = false;
        const modal = document.getElementById('path-input-modal');
        if (modal) {
            modal.classList.remove('visible');
        }
        // Clear autocomplete state
        autocompleteState.items = [];
        autocompleteState.selectedIndex = -1;
        if (autocompleteState.debounceTimer) {
            clearTimeout(autocompleteState.debounceTimer);
        }
        // Clear history navigation state
        pathHistoryState.currentIndex = -1;
        pathHistoryState.originalValue = '';
    }

    // Check clipboard for markdown file path (optional feature)
    async function checkClipboardForMarkdown() {
        try {
            const clipboardPath = await invoke('check_clipboard_for_markdown');
            if (clipboardPath) {
                console.log('Found markdown file in clipboard:', clipboardPath);
                return clipboardPath;
            }
        } catch (e) {
            console.error('Failed to check clipboard:', e);
        }
        return null;
    }

    // Show the metadata refresh indicator dot
    function showRefreshDot() {
        const dot = document.getElementById('metadata-refresh-dot');
        if (dot) {
            dot.classList.add('visible');
            // Auto-hide after 2 seconds
            setTimeout(() => {
                dot.classList.remove('visible');
            }, 2000);
        }
    }

    // Refresh file metadata when window gains focus
    async function refreshFileMetadata() {
        const currentPath = window.MarkdownPreviewCore.getCurrentFilePath();
        if (!currentPath) return;

        try {
            const metadata = await invoke('refresh_file_metadata');
            if (metadata) {
                updateFileMetadata(
                    metadata.modified_at,
                    metadata.stats,
                    metadata.git_branch,
                    metadata.git_branch_url,
                    metadata.git_last_author
                );
                showRefreshDot();
            }
        } catch (e) {
            console.error('Failed to refresh file metadata:', e);
        }
    }

    // Set up clipboard monitoring on window focus
    function setupClipboardMonitoring() {
        let lastClipboardPath = null;

        async function checkClipboard() {
            const current = window.MarkdownPreviewCore.getCurrentFilePath();
            // Only check if no file is currently open
            if (current) {
                return;
            }

            const clipboardPath = await checkClipboardForMarkdown();
            if (clipboardPath && clipboardPath !== lastClipboardPath) {
                lastClipboardPath = clipboardPath;
                await openFile(clipboardPath);
            }
        }

        // Use Tauri's window focus event
        if (window.__TAURI__.window) {
            const { getCurrentWindow } = window.__TAURI__.window;
            const currentWindow = getCurrentWindow();
            currentWindow.onFocusChanged(({ payload: focused }) => {
                if (focused) {
                    checkClipboard();
                    // Also refresh file metadata when window gains focus
                    refreshFileMetadata();
                }
            });
        }
    }

    // Initialize on DOM ready
    document.addEventListener('DOMContentLoaded', async function() {
        // Initialize core UI with file switch and remove callbacks
        initCoreUI({
            onFileClick: switchToFile,
            onRemove: removeRecentFileFromBackend
        });

        // Load recent files from backend (persistent storage)
        await loadRecentFilesFromBackend();

        // Set up Tauri-specific features
        setupTauriListeners();
        setupDragAndDrop();
        setupWelcomeScreen();
        setupKeyboardShortcuts();
        setupVimNavigation();
        setupClipboardMonitoring();
    });
})();
