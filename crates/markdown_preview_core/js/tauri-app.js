// MEAD - Tauri Mode (Standalone App)
// This module handles Tauri IPC communication for the standalone application

// Note: core.js must be loaded before this file

// Check if we're running in Tauri
if (typeof window.__TAURI__ !== 'undefined') {
    console.log('Running in Tauri mode');
} else {
    console.log('Not running in Tauri mode');
}

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
                    // Fetch diff for the file (async, don't block)
                    fetchFileDiff(result.file_path);
                }
                return result;
            }
        } catch (e) {
            console.error('Failed to open file:', e);
            showToast('Failed to open file: ' + (e.message || e));
        }
        return null;
    }

    // Fetch file diff from backend and store for later display
    async function fetchFileDiff(filePath) {
        try {
            const diff = await invoke('get_file_diff', { path: filePath });
            setCurrentDiff(diff);
            if (diff && diff.has_changes) {
                console.log('File has changes since last view');
            }
        } catch (e) {
            console.error('Failed to fetch file diff:', e);
            setCurrentDiff(null);
        }
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
                        <div class="github-token-message">${escapeHtml(message)}</div>
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
        content.innerHTML = sanitizeHtml(result.html);

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
                document.title = fileName + ' - MEAD';
            } catch {
                document.title = 'Remote Markdown - MEAD';
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
                        <p>${escapeHtml(String(err.message || err))}</p>
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
            content.innerHTML = sanitizeHtml(html);

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
                <h2>Unsupported document type: ${escapeHtml(String(docType))}</h2>
            </div>`;
        }

        if (result.file_path) {
            // Update currentFilePath via the core module's setter
            window.MarkdownPreviewCore.setCurrentFilePath(result.file_path);
            updateFilePathBar(result.file_path, result.git_root);
            document.title = getFileBasename(result.file_path) + ' - MEAD';

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

    // ========================================
    // Settings dialog
    // ========================================
    let settingsDialogVisible = false;

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

        listen('menu-toggle-terminal', () => {
            toggleTerminalPanel();
        });

        listen('menu-settings', () => {
            showSettingsDialog();
        });

        listen('menu-dictionary', () => {
            openDictionaryWithSelection();
        });

        // Listen for initial file from command line argument
        listen('open-initial-file', async (event) => {
            console.log('Opening initial file:', event.payload);
            await openFile(event.payload);
        });

        // Listen for AI summary progress events
        listen('ai-summary-progress', (event) => {
            const { status, filePath, completed, total } = event.payload;

            switch (status) {
                case 'batch_started':
                    showAiProgressIndicator(0, total);
                    break;
                case 'file_started':
                    showAiProgressIndicator(0, 1);
                    break;
                case 'file_done':
                    // Invalidate tooltip cache so next hover fetches fresh AI summary
                    if (filePath && typeof markdownTitleCache !== 'undefined') {
                        markdownTitleCache.delete(filePath);
                    }
                    if (total) {
                        showAiProgressIndicator(completed, total);
                    } else {
                        hideAiProgressIndicator();
                    }
                    break;
                case 'batch_done':
                    hideAiProgressIndicator();
                    break;
                case 'error':
                    showAiProgressError(event.payload.error);
                    break;
            }
        });
    }

    // Expose settings dialog globally so the activity bar can call it
    window.showSettingsDialog = showSettingsDialog;

    // ========================================
    // AI Summary Progress Indicator
    // ========================================

    let aiProgressElement = null;
    let aiProgressHideTimeout = null;

    function getOrCreateAiProgress() {
        if (aiProgressElement) return aiProgressElement;

        aiProgressElement = document.createElement('div');
        aiProgressElement.className = 'ai-progress-indicator';
        aiProgressElement.innerHTML = `
            <span class="ai-progress-spinner"></span>
            <span class="ai-progress-text"></span>
        `;

        // Insert into sidebar, after the Recent Previews header
        const section = document.getElementById('recent-files-section');
        const recentFiles = document.getElementById('recent-files');
        if (section && recentFiles) {
            section.insertBefore(aiProgressElement, recentFiles);
        }

        return aiProgressElement;
    }

    function showAiProgressIndicator(completed, total) {
        if (aiProgressHideTimeout) {
            clearTimeout(aiProgressHideTimeout);
            aiProgressHideTimeout = null;
        }
        aiLastError = null;

        const el = getOrCreateAiProgress();
        const text = el.querySelector('.ai-progress-text');

        if (total > 1) {
            text.textContent = `Summarizing ${completed}/${total}...`;
        } else {
            text.textContent = 'Summarizing...';
        }

        el.classList.add('visible');
        el.classList.remove('done', 'error');
    }

    function hideAiProgressIndicator() {
        if (!aiProgressElement) return;

        const text = aiProgressElement.querySelector('.ai-progress-text');
        text.textContent = 'Summaries ready';
        aiProgressElement.classList.add('done');

        // Hide after a brief delay
        aiProgressHideTimeout = setTimeout(() => {
            aiProgressElement.classList.remove('visible', 'done');
            aiProgressHideTimeout = null;
        }, 2000);
    }

    // Deduplicate errors — only show each unique message once per batch
    let aiLastError = null;

    function showAiProgressError(error) {
        if (error === aiLastError) return;
        aiLastError = error;

        const el = getOrCreateAiProgress();
        const text = el.querySelector('.ai-progress-text');
        text.textContent = error;
        el.classList.add('visible', 'error');
        el.classList.remove('done');

        // Auto-hide after 5s
        if (aiProgressHideTimeout) clearTimeout(aiProgressHideTimeout);
        aiProgressHideTimeout = setTimeout(() => {
            aiProgressElement.classList.remove('visible', 'error');
            aiLastError = null;
            aiProgressHideTimeout = null;
        }, 5000);
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
                            <div class="help-row"><kbd>Ctrl+,</kbd><span>Settings</span></div>
                        </div>
                        <div class="help-section">
                            <h4>Search & View</h4>
                            <div class="help-row"><kbd>/</kbd> / <kbd>Ctrl+P</kbd><span>Open fuzzy finder</span></div>
                            <div class="help-row"><kbd>Ctrl+D</kbd><span>Show file changes</span></div>
                            <div class="help-row"><kbd>Ctrl++</kbd> / <kbd>Ctrl+-</kbd><span>Zoom in / out</span></div>
                            <div class="help-row"><kbd>Ctrl+0</kbd><span>Reset zoom</span></div>
                        </div>
                        <div class="help-section">
                            <h4>Terminal</h4>
                            <div class="help-row"><kbd>Ctrl+\`</kbd><span>Toggle terminal panel</span></div>
                            <div class="help-row"><kbd>Esc</kbd><span>Close terminal (when focused)</span></div>
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

    // Refresh the currently open file
    async function refreshCurrentFile() {
        const current = window.MarkdownPreviewCore.getCurrentFilePath();
        if (current) {
            await openFile(current);
            showToast('Refreshed');
        }
    }

    // Quit the application
    function quitApp() {
        const { getCurrentWindow } = window.__TAURI__.window;
        getCurrentWindow().close();
    }

    // Set up keyboard shortcuts
    function setupKeyboardShortcuts() {
        const notInTerminal = () => !isTerminalPanelFocused();

        // Modifier shortcuts via registry (guarded to not fire while terminal is focused)
        registerShortcut('o', { ctrl: true }, () => openPathInput(), { when: notInTerminal });
        registerShortcut('o', { ctrl: true, shift: true }, () => openFileDialog(), { when: notInTerminal });
        registerShortcut('r', { ctrl: true }, () => refreshCurrentFile(), { when: notInTerminal });
        registerShortcut('d', { ctrl: true }, () => toggleDiffOverlay(), { when: notInTerminal });
        registerShortcut('q', { ctrl: true }, () => quitApp(), { when: notInTerminal });
        registerShortcut('`', { ctrl: true }, () => toggleTerminalPanel());
        registerShortcut('l', { ctrl: true }, () => openDictionaryWithSelection(), { when: notInTerminal });
        registerShortcut('k', { ctrl: true }, () => openAskAiWithSelection(), { when: notInTerminal });

        // Start the listener
        setupShortcutListener();

        // Manual handlers that don't go in the registry
        document.addEventListener('keydown', async (e) => {
            // Let the terminal handle all keys when it's focused
            // (except Ctrl+` which is the toggle shortcut, handled by the registry)
            if (isTerminalPanelFocused()) return;

            // F5 refresh (no modifier)
            if (e.key === 'F5') {
                e.preventDefault();
                await refreshCurrentFile();
            }

            // Ctrl+C with selection check (conditional preventDefault)
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

            // Escape priority chain
            if (e.key === 'Escape') {
                if (isTerminalPanelOpen()) { closeTerminalPanel(); return; }
                if (isDiffOverlayVisible()) { hideDiffOverlay(); return; }
                if (pathInputVisible) { closePathInput(); return; }
            }
        });
    }

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
        initDictionary();
        initAskAi();
    });
