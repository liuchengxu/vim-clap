// MEAD - Tauri Mode (Standalone App)
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

    async function showSettingsDialog() {
        if (settingsDialogVisible) return;
        settingsDialogVisible = true;

        const existing = document.getElementById('settings-dialog');
        if (existing) existing.remove();

        const dialog = document.createElement('div');
        dialog.id = 'settings-dialog';
        dialog.innerHTML = `
            <div class="settings-overlay">
                <div class="settings-container">
                    <div class="settings-header">
                        <div class="settings-title">Settings</div>
                    </div>
                    <div class="settings-body">
                        <div class="settings-section">
                            <h4>GitHub</h4>
                            <div class="settings-field">
                                <label for="settings-github-token">Personal Access Token</label>
                                <input type="password" id="settings-github-token" placeholder="ghp_xxxxxxxxxxxxxxxxxxxx" autocomplete="off" spellcheck="false">
                                <p class="settings-hint">Used for accessing private repositories. Falls back to GITHUB_TOKEN env var.</p>
                            </div>
                        </div>
                        <div class="settings-section">
                            <h4>AI Summaries</h4>
                            <div class="settings-field">
                                <label for="settings-ai-provider">Provider</label>
                                <select id="settings-ai-provider">
                                    <option value="">None (disabled)</option>
                                    <option value="ollama">Ollama (local)</option>
                                    <option value="anthropic">Anthropic (Claude)</option>
                                    <option value="openai">OpenAI</option>
                                </select>
                            </div>
                            <div class="settings-field" id="settings-model-field" style="display:none;">
                                <label for="settings-ai-model">Model</label>
                                <input type="text" id="settings-ai-model" placeholder="Leave blank for provider default">
                            </div>
                            <div class="settings-field" id="settings-api-key-field" style="display:none;">
                                <label for="settings-ai-api-key">API Key</label>
                                <input type="password" id="settings-ai-api-key" placeholder="sk-... or anthropic key" autocomplete="off" spellcheck="false">
                                <p class="settings-hint">Falls back to ANTHROPIC_API_KEY / OPENAI_API_KEY env var.</p>
                            </div>
                            <div class="settings-field" id="settings-ollama-url-field" style="display:none;">
                                <label for="settings-ollama-url">Ollama URL</label>
                                <input type="text" id="settings-ollama-url" placeholder="http://localhost:11434">
                                <p class="settings-hint">Falls back to OLLAMA_URL env var. Default: http://localhost:11434</p>
                            </div>
                        </div>
                        <div class="settings-section">
                            <h4>Offline Dictionaries</h4>
                            <div class="settings-field">
                                <label>Dictionary Directories</label>
                                <div id="settings-dict-dirs"></div>
                                <button id="settings-add-dict-dir" class="btn-small" type="button">+ Add Directory</button>
                                <p class="settings-hint">
                                    Add folders containing StarDict (.ifo/.idx/.dict) or MDict (.mdx) files.
                                </p>
                            </div>
                            <div id="settings-loaded-dicts-field" class="settings-field" style="display:none;">
                                <label>Loaded Dictionaries</label>
                                <div id="settings-dict-list"></div>
                            </div>
                        </div>
                    </div>
                    <div class="settings-footer">
                        <button id="settings-cancel" class="btn-cancel">Cancel</button>
                        <button id="settings-save" class="btn-submit">Save</button>
                    </div>
                </div>
            </div>
        `;
        document.body.appendChild(dialog);

        // Add styles if not already present
        if (!document.getElementById('settings-styles')) {
            const style = document.createElement('style');
            style.id = 'settings-styles';
            style.textContent = `
                .settings-overlay {
                    position: fixed;
                    top: 0; left: 0; right: 0; bottom: 0;
                    background: rgba(0, 0, 0, 0.5);
                    display: flex;
                    align-items: flex-start;
                    justify-content: center;
                    padding-top: 10vh;
                    z-index: 10001;
                }
                .settings-container {
                    background: #fff;
                    border-radius: 8px;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                    width: 560px;
                    max-width: 90vw;
                    max-height: 80vh;
                    display: flex;
                    flex-direction: column;
                }
                .settings-header {
                    padding: 16px 20px;
                    border-bottom: 1px solid #e1e4e8;
                }
                .settings-title {
                    font-weight: 600;
                    font-size: 16px;
                }
                .settings-body {
                    padding: 16px 20px;
                    overflow-y: auto;
                    flex: 1;
                }
                .settings-section {
                    margin-bottom: 20px;
                }
                .settings-section:last-child {
                    margin-bottom: 0;
                }
                .settings-section h4 {
                    font-size: 11px;
                    font-weight: 600;
                    text-transform: uppercase;
                    letter-spacing: 0.5px;
                    color: #6b7280;
                    margin: 0 0 12px 0;
                    padding-bottom: 6px;
                    border-bottom: 1px solid #e5e7eb;
                }
                .settings-field {
                    margin-bottom: 14px;
                }
                .settings-field:last-child {
                    margin-bottom: 0;
                }
                .settings-field label {
                    display: block;
                    font-size: 13px;
                    font-weight: 500;
                    margin-bottom: 4px;
                    color: #374151;
                }
                .settings-field input,
                .settings-field select {
                    width: 100%;
                    padding: 8px 10px;
                    border: 1px solid #d1d5db;
                    border-radius: 6px;
                    font-size: 13px;
                    font-family: ui-monospace, SFMono-Regular, monospace;
                    box-sizing: border-box;
                    background: #fff;
                }
                .settings-field input:focus,
                .settings-field select:focus {
                    outline: none;
                    border-color: #2563eb;
                    box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.1);
                }
                .settings-hint {
                    font-size: 11px;
                    color: #9ca3af;
                    margin: 4px 0 0 0;
                }
                .settings-footer {
                    padding: 12px 20px;
                    background: #f6f8fa;
                    display: flex;
                    justify-content: flex-end;
                    gap: 8px;
                    border-radius: 0 0 8px 8px;
                    border-top: 1px solid #e1e4e8;
                }
                .settings-footer button {
                    padding: 8px 16px;
                    border-radius: 6px;
                    font-size: 14px;
                    font-weight: 500;
                    cursor: pointer;
                }
                .settings-footer .btn-cancel {
                    background: #fff;
                    border: 1px solid #d1d5db;
                    color: #374151;
                }
                .settings-footer .btn-cancel:hover {
                    background: #f3f4f6;
                }
                .settings-footer .btn-submit {
                    background: #2563eb;
                    border: 1px solid #2563eb;
                    color: #fff;
                }
                .settings-footer .btn-submit:hover {
                    background: #1d4ed8;
                }
                @media (prefers-color-scheme: dark) {
                    .settings-container { background: #1e1e1e; }
                    .settings-header { border-color: #333; }
                    .settings-title { color: #fff; }
                    .settings-body { color: #fff; }
                    .settings-section h4 { color: #9ca3af; border-color: #333; }
                    .settings-field label { color: #d1d5db; }
                    .settings-field input,
                    .settings-field select { background: #2d2d2d; border-color: #444; color: #fff; }
                    .settings-hint { color: #6b7280; }
                    .settings-footer { background: #252525; border-color: #333; }
                    .settings-footer .btn-cancel { background: #333; border-color: #444; color: #fff; }
                }
            `;
            document.head.appendChild(style);
        }

        const providerSelect = document.getElementById('settings-ai-provider');
        const modelField = document.getElementById('settings-model-field');
        const apiKeyField = document.getElementById('settings-api-key-field');
        const ollamaUrlField = document.getElementById('settings-ollama-url-field');

        function updateFieldVisibility() {
            const provider = providerSelect.value;
            modelField.style.display = provider ? '' : 'none';
            apiKeyField.style.display = (provider === 'anthropic' || provider === 'openai') ? '' : 'none';
            ollamaUrlField.style.display = provider === 'ollama' ? '' : 'none';
        }

        providerSelect.addEventListener('change', updateFieldVisibility);

        // Dictionary directory management
        let dictDirs = [];
        const dictDirsContainer = document.getElementById('settings-dict-dirs');

        function renderDictDirs() {
            dictDirsContainer.innerHTML = dictDirs.map((dir, i) =>
                `<div class="dict-dir-row">
                    <span class="dict-dir-path">${escapeHtml(dir)}</span>
                    <button class="dict-dir-remove" data-idx="${i}" type="button">&times;</button>
                </div>`
            ).join('');
            dictDirsContainer.querySelectorAll('.dict-dir-remove').forEach(btn => {
                btn.addEventListener('click', () => {
                    dictDirs.splice(parseInt(btn.dataset.idx), 1);
                    renderDictDirs();
                });
            });
        }

        document.getElementById('settings-add-dict-dir').addEventListener('click', async () => {
            try {
                const selected = await window.__TAURI__.dialog.open({ directory: true, multiple: false });
                if (selected && !dictDirs.includes(selected)) {
                    dictDirs.push(selected);
                    renderDictDirs();
                }
            } catch (e) {
                console.error('Failed to open directory picker:', e);
            }
        });

        // Populate fields from backend
        try {
            const config = await invoke('get_app_config');
            document.getElementById('settings-github-token').value = config.github_token || '';
            providerSelect.value = config.ai_provider || '';
            document.getElementById('settings-ai-model').value = config.ai_model || '';
            document.getElementById('settings-ai-api-key').value = config.ai_api_key || '';
            document.getElementById('settings-ollama-url').value = config.ollama_url || '';
            dictDirs = config.dictionary_dirs || [];
            renderDictDirs();
            updateFieldVisibility();

            // Show loaded dictionaries if any dirs are configured
            if (dictDirs.length > 0) {
                try {
                    const loaded = await invoke('get_loaded_dictionaries');
                    if (loaded.length > 0) {
                        const listEl = document.getElementById('settings-dict-list');
                        const fieldEl = document.getElementById('settings-loaded-dicts-field');
                        fieldEl.style.display = '';
                        listEl.innerHTML = loaded.map(d =>
                            `<div class="dict-loaded-item">${escapeHtml(d.name)} <span class="dict-word-count">(${d.word_count.toLocaleString()} words)</span></div>`
                        ).join('');
                    }
                } catch (e) {
                    console.warn('Failed to load dictionaries:', e);
                }
            }
        } catch (e) {
            console.error('Failed to load config:', e);
        }

        function closeSettingsDialog() {
            settingsDialogVisible = false;
            dialog.remove();
        }

        async function saveSettings() {
            const config = {
                github_token: document.getElementById('settings-github-token').value.trim() || null,
                ai_provider: providerSelect.value || null,
                ai_model: document.getElementById('settings-ai-model').value.trim() || null,
                ai_api_key: document.getElementById('settings-ai-api-key').value.trim() || null,
                ollama_url: document.getElementById('settings-ollama-url').value.trim() || null,
                dictionary_dirs: dictDirs,
            };

            try {
                await invoke('set_app_config', { config });

                // Refresh loaded dictionaries display after save
                try {
                    const loaded = await invoke('get_loaded_dictionaries');
                    if (loaded.length > 0) {
                        const listEl = document.getElementById('settings-dict-list');
                        const fieldEl = document.getElementById('settings-loaded-dicts-field');
                        if (fieldEl && listEl) {
                            fieldEl.style.display = '';
                            listEl.innerHTML = loaded.map(d =>
                                `<div class="dict-loaded-item">${escapeHtml(d.name)} <span class="dict-word-count">(${d.word_count.toLocaleString()} words)</span></div>`
                            ).join('');
                        }
                    }
                } catch (_e) { /* non-fatal */ }

                closeSettingsDialog();
            } catch (e) {
                console.error('Failed to save settings:', e);
            }
        }

        document.getElementById('settings-cancel').onclick = closeSettingsDialog;
        document.getElementById('settings-save').onclick = saveSettings;

        // Keyboard handlers
        dialog.addEventListener('keydown', (e) => {
            if (e.key === 'Escape') {
                e.preventDefault();
                closeSettingsDialog();
            }
        });

        // Click outside to close
        dialog.querySelector('.settings-overlay').onclick = (e) => {
            if (e.target.classList.contains('settings-overlay')) {
                closeSettingsDialog();
            }
        };

        // Focus the first input
        document.getElementById('settings-github-token').focus();
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
                <span class="autocomplete-name">${escapeHtml(item.name)}</span>
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
    // ========================================
    // Dictionary Tool
    // ========================================

    let dictHistory = JSON.parse(localStorage.getItem('dictHistory') || '[]');
    let dictLastResult = null;
    let lookupCounter = 0;

    // AI request usage tracking
    function loadAiUsageStats() {
        try {
            return JSON.parse(localStorage.getItem('aiUsageStats') || '{}');
        } catch (_e) {
            return {};
        }
    }

    function saveAiUsageStats(stats) {
        localStorage.setItem('aiUsageStats', JSON.stringify(stats));
    }

    function recordAiRequest() {
        const stats = loadAiUsageStats();
        const now = new Date();
        const dayKey = now.toISOString().slice(0, 10);    // "2026-02-16"
        const monthKey = now.toISOString().slice(0, 7);    // "2026-02"

        stats.total = (stats.total || 0) + 1;
        if (!stats.monthly) stats.monthly = {};
        stats.monthly[monthKey] = (stats.monthly[monthKey] || 0) + 1;
        if (!stats.daily) stats.daily = {};
        stats.daily[dayKey] = (stats.daily[dayKey] || 0) + 1;

        saveAiUsageStats(stats);
        renderAiUsageStats();
    }

    function renderAiUsageStats() {
        const el = document.getElementById('dict-ai-stats');
        if (!el) return;

        const stats = loadAiUsageStats();
        const total = stats.total || 0;
        if (total === 0) {
            el.innerHTML = '';
            return;
        }

        const now = new Date();
        const dayKey = now.toISOString().slice(0, 10);
        const monthKey = now.toISOString().slice(0, 7);
        const today = (stats.daily && stats.daily[dayKey]) || 0;
        const month = (stats.monthly && stats.monthly[monthKey]) || 0;

        el.innerHTML = `<span class="ai-stats-label">AI requests:</span> `
            + `<span class="ai-stats-value">${today} today</span>`
            + `<span class="ai-stats-sep">/</span>`
            + `<span class="ai-stats-value">${month} this month</span>`
            + `<span class="ai-stats-sep">/</span>`
            + `<span class="ai-stats-value">${total} total</span>`
            + `<button class="ai-stats-reset" title="Reset counter">reset</button>`;

        el.querySelector('.ai-stats-reset').addEventListener('click', () => {
            if (confirm('Reset AI usage stats?')) {
                localStorage.removeItem('aiUsageStats');
                renderAiUsageStats();
            }
        });
    }

    function initDictionary() {
        const input = document.getElementById('dict-word-input');
        const btn = document.getElementById('dict-lookup-btn');
        if (!input || !btn) return;

        input.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') {
                e.preventDefault();
                performLookup(input.value.trim());
            }
        });

        btn.addEventListener('click', () => {
            performLookup(input.value.trim());
        });

        renderDictHistory();
        renderAiUsageStats();
    }

    async function performLookup(word) {
        if (!word) return;

        const thisLookupId = ++lookupCounter;
        const resultsEl = document.getElementById('dict-results');
        if (!resultsEl) return;

        const isStale = () => thisLookupId !== lookupCounter;

        // Show loading
        resultsEl.innerHTML = '<div class="dict-loading"><span class="dict-loading-spinner"></span> Looking up...</div>';
        addToDictHistory(word);

        const btn = document.getElementById('dict-lookup-btn');
        if (btn) btn.disabled = true;

        let hasOfflineResults = false;
        let hasOnlineResults = false;
        let hasAiResults = false;

        // Helper: returns true when we already have at least one source of results.
        const hasAnyResults = () => hasOfflineResults || hasOnlineResults;

        // 1. Offline lookup (fast, in-memory)
        try {
            const offlineResults = await invoke('lookup_word_offline', { word });
            if (isStale()) { if (btn) btn.disabled = false; return; }
            if (offlineResults.length > 0) {
                hasOfflineResults = true;
                resultsEl.innerHTML = `<div class="dict-word-header"><h1 class="dict-word-title">${escapeHtml(word)}</h1>`
                    + `<button class="dict-pronounce-btn" data-word="${escapeHtml(word)}" title="Pronounce">&#x1f50a;</button></div>`
                    + renderOfflineResults(offlineResults);
                wireUpPronounceButtons(resultsEl);
            }
        } catch (_err) {
            if (isStale()) { if (btn) btn.disabled = false; return; }
        }

        // 2. Online dictionary + Wiktionary etymology (parallel)
        let etymologyHtml = null;
        try {
            const [onlineResult, etymResult] = await Promise.allSettled([
                invoke('lookup_word_online', { word }),
                invoke('lookup_etymology', { word }),
            ]);
            if (isStale()) { if (btn) btn.disabled = false; return; }

            // Process online dictionary result
            if (onlineResult.status === 'fulfilled' && onlineResult.value) {
                const onlineEntry = onlineResult.value;
                hasOnlineResults = true;
                const onlineHtml = renderDictEntry(onlineEntry, 'online', onlineEntry.source || 'Online');
                if (hasOfflineResults) {
                    resultsEl.insertAdjacentHTML('beforeend', onlineHtml);
                } else {
                    resultsEl.innerHTML = onlineHtml;
                }
                wireUpDictTags(resultsEl);
                wireUpPronounceButtons(resultsEl);
            }

            // Process Wiktionary etymology result
            if (etymResult.status === 'fulfilled' && etymResult.value) {
                etymologyHtml = etymResult.value.etymology_html;
                const etymSection = '<div class="dict-etymology dict-etymology-wiktionary">'
                    + '<div class="dict-etymology-label">Etymology <span class="dict-etymology-source">'
                    + `(${escapeHtml(etymResult.value.source)})</span></div>`
                    + `<div class="dict-etymology-text">${sanitizeHtml(etymologyHtml)}</div></div>`;
                resultsEl.insertAdjacentHTML('beforeend', etymSection);
            }
        } catch (_err) {
            if (isStale()) { if (btn) btn.disabled = false; return; }
        }

        // 3. AI lookup (slow, async) — only if configured
        try {
            if (hasAnyResults()) {
                resultsEl.insertAdjacentHTML('beforeend',
                    '<div class="dict-ai-loading">Loading AI definition...</div>');
            }

            const response = await invoke('lookup_word', { word });
            if (isStale()) { if (btn) btn.disabled = false; return; }

            const entry = response;
            hasAiResults = true;
            dictLastResult = entry;
            if (!response.cached) {
                recordAiRequest();
            }

            const aiLoadingEl = resultsEl.querySelector('.dict-ai-loading');
            if (aiLoadingEl) aiLoadingEl.remove();

            const aiHtml = renderDictEntry(entry, 'ai', 'AI');
            if (hasAnyResults()) {
                resultsEl.insertAdjacentHTML('beforeend', aiHtml);
            } else {
                resultsEl.innerHTML = aiHtml;
            }
            wireUpDictTags(resultsEl);
            wireUpPronounceButtons(resultsEl);

            // Show AI etymology fallback if Wiktionary section is absent or empty
            const wikiEtymEl = resultsEl.querySelector('.dict-etymology-wiktionary .dict-etymology-text');
            const wikiEtymHasContent = wikiEtymEl && wikiEtymEl.textContent.trim().length > 0;
            if (!wikiEtymHasContent) {
                const emptyWiki = resultsEl.querySelector('.dict-etymology-wiktionary');
                if (emptyWiki) emptyWiki.remove();
                resultsEl.querySelectorAll('.dict-etymology-ai').forEach(el => el.style.display = '');
            }

            // Render Mermaid diagrams
            await renderDictDiagrams(resultsEl);
        } catch (err) {
            if (isStale()) { if (btn) btn.disabled = false; return; }
            const aiLoadingEl = resultsEl.querySelector('.dict-ai-loading');
            if (aiLoadingEl) aiLoadingEl.remove();
            if (!hasAnyResults()) {
                const errStr = String(err);
                if (errStr.includes('not configured')) {
                    resultsEl.innerHTML = '<div class="dict-empty-state">'
                        + '<p>No dictionaries matched.</p>'
                        + '<p>AI provider not configured. Set one in Settings.</p></div>';
                } else {
                    resultsEl.innerHTML = `<div class="dict-error">${escapeHtml(errStr)}</div>`;
                }
            }
        } finally {
            if (btn) btn.disabled = false;
        }

        if (isStale()) return;
        if (!hasAnyResults() && !hasAiResults) {
            resultsEl.innerHTML = '<div class="dict-empty-state">No results found</div>';
        }
    }

    function renderOfflineResults(results) {
        return results.map(r => {
            const sanitized = sanitizeHtml(r.html);
            return `<div class="dict-offline-entry">
                <div class="dict-entry-header">
                    <div class="dict-source-badge offline">${escapeHtml(r.dict_name)}</div>
                </div>
                <div class="dict-offline-content">${sanitized}</div>
            </div>`;
        }).join('');
    }

    /**
     * Render a structured dictionary entry (used for both online and AI sources).
     * @param {Object} entry - DictionaryEntry {word, phonetic, definitions, synonyms, antonyms}
     * @param {string} badgeClass - CSS class for the source badge (e.g. 'ai', 'online')
     * @param {string} badgeLabel - Display text for the source badge
     */
    function renderDictEntry(entry, badgeClass, badgeLabel) {
        let html = `<div class="dict-entry">`;

        // Word header with source badge, phonetic and pronounce button
        html += '<div class="dict-word-header">';
        html += `<div class="dict-source-badge ${escapeHtml(badgeClass)}">${escapeHtml(badgeLabel)}</div>`;
        html += `<h1 class="dict-word-title">${escapeHtml(entry.word)}</h1>`;
        html += `<button class="dict-pronounce-btn" data-word="${escapeHtml(entry.word)}" title="Pronounce">&#x1f50a;</button>`;
        if (entry.phonetic) {
            html += `<span class="dict-phonetic">${escapeHtml(entry.phonetic)}</span>`;
        }
        html += '</div>';

        // Group definitions by part of speech
        const grouped = {};
        for (const def of entry.definitions) {
            const pos = def.part_of_speech || 'other';
            if (!grouped[pos]) grouped[pos] = [];
            grouped[pos].push(def);
        }

        for (const [pos, defs] of Object.entries(grouped)) {
            html += '<div class="dict-section">';
            html += `<div class="dict-pos">${escapeHtml(pos)}</div>`;
            for (const def of defs) {
                html += '<div class="dict-definition">';
                html += `<div class="dict-meaning">${escapeHtml(def.meaning)}</div>`;
                if (def.example) {
                    html += `<div class="dict-example">"${escapeHtml(def.example)}"</div>`;
                }
                html += '</div>';
            }
            html += '</div>';
        }

        // Synonyms
        if (entry.synonyms && entry.synonyms.length > 0) {
            html += '<div class="dict-tags">';
            html += '<span class="dict-tag-label">Synonyms:</span>';
            for (const syn of entry.synonyms) {
                html += `<span class="dict-tag" data-word="${escapeHtml(syn)}">${escapeHtml(syn)}</span>`;
            }
            html += '</div>';
        }

        // Antonyms
        if (entry.antonyms && entry.antonyms.length > 0) {
            html += '<div class="dict-tags">';
            html += '<span class="dict-tag-label">Antonyms:</span>';
            for (const ant of entry.antonyms) {
                html += `<span class="dict-tag antonym" data-word="${escapeHtml(ant)}">${escapeHtml(ant)}</span>`;
            }
            html += '</div>';
        }

        // Mnemonic tip
        if (entry.mnemonic) {
            html += '<div class="dict-mnemonic">'
                + '<div class="dict-mnemonic-label">Memory Tip</div>'
                + `<div class="dict-mnemonic-text">${escapeHtml(entry.mnemonic)}</div></div>`;
        }

        // AI etymology fallback (hidden by default, shown if Wiktionary has none)
        if (entry.etymology) {
            html += '<div class="dict-etymology dict-etymology-ai" style="display:none">'
                + '<div class="dict-etymology-label">Etymology <span class="dict-etymology-source">(AI)</span></div>'
                + `<div class="dict-etymology-text">${escapeHtml(entry.etymology)}</div></div>`;
        }

        // Word Family diagram placeholder
        if (entry.word_family && entry.word_family.length > 0) {
            const familyJson = escapeHtml(JSON.stringify(entry.word_family));
            html += `<div class="dict-word-family" data-word="${escapeHtml(entry.word)}" data-family="${familyJson}"></div>`;
        }

        // Semantic Map diagram placeholder
        const hasSyns = entry.synonyms && entry.synonyms.length > 0;
        const hasAnts = entry.antonyms && entry.antonyms.length > 0;
        const hasRelated = entry.related_concepts && entry.related_concepts.length > 0;
        if (hasSyns || hasAnts || hasRelated) {
            const synsJson = escapeHtml(JSON.stringify(entry.synonyms || []));
            const antsJson = escapeHtml(JSON.stringify(entry.antonyms || []));
            const relJson = escapeHtml(JSON.stringify(entry.related_concepts || []));
            html += `<div class="dict-semantic-map" data-word="${escapeHtml(entry.word)}" data-synonyms="${synsJson}" data-antonyms="${antsJson}" data-related="${relJson}"></div>`;
        }

        html += '</div>';
        return html;
    }

    // -----------------------------------------------------------------------
    // Mermaid diagram helpers for dictionary memory aids
    // -----------------------------------------------------------------------

    /** Escape text for safe use as a Mermaid node label (wrap in quotes, escape inner quotes). */
    function mermaidEscape(text) {
        return '"' + String(text).replace(/"/g, '#quot;') + '"';
    }

    /** Render a Mermaid diagram definition into an SVG string. Returns '' on failure. */
    async function renderMermaidDiagram(id, definition) {
        if (!window.mermaid || !window.mermaid.render) return '';
        try {
            const { svg } = await window.mermaid.render(id, definition);
            return svg;
        } catch (_e) {
            return '';
        }
    }

    /** Build a Mermaid graph LR definition for a word family. */
    function buildWordFamilyGraph(rootWord, family) {
        let def = 'graph LR\n';
        const rootId = 'root';
        def += `  ${rootId}[${mermaidEscape(rootWord)}]\n`;
        family.forEach((member, i) => {
            const nodeId = `f${i}`;
            const label = member.part_of_speech
                ? `${member.word} (${member.part_of_speech})`
                : member.word;
            def += `  ${rootId} --> ${nodeId}[${mermaidEscape(label)}]\n`;
        });
        return def;
    }

    /** Build a Mermaid mindmap definition for a semantic map. */
    function buildSemanticMindmap(word, synonyms, antonyms, relatedConcepts) {
        let def = 'mindmap\n';
        def += `  root((${mermaidEscape(word)}))\n`;
        if (synonyms.length > 0) {
            def += '    Synonyms\n';
            for (const s of synonyms) {
                def += `      ${mermaidEscape(s)}\n`;
            }
        }
        if (antonyms.length > 0) {
            def += '    Antonyms\n';
            for (const a of antonyms) {
                def += `      ${mermaidEscape(a)}\n`;
            }
        }
        if (relatedConcepts.length > 0) {
            def += '    Related\n';
            for (const r of relatedConcepts) {
                def += `      ${mermaidEscape(r)}\n`;
            }
        }
        return def;
    }

    /** Find diagram placeholders inside `container` and render Mermaid SVGs into them. */
    async function renderDictDiagrams(container) {
        let diagramIdx = 0;

        // Word Family diagrams
        for (const el of container.querySelectorAll('.dict-word-family[data-family]')) {
            const word = el.dataset.word || '';
            let family;
            try { family = JSON.parse(el.dataset.family); } catch (_e) { continue; }
            if (!Array.isArray(family) || family.length === 0) continue;

            const def = buildWordFamilyGraph(word, family);
            const svg = await renderMermaidDiagram(`dict-wf-${diagramIdx++}`, def);
            if (svg) {
                el.innerHTML = '<div class="dict-diagram-label">Word Family</div>'
                    + `<div class="dict-diagram-container">${svg}</div>`;
            }
        }

        // Semantic Map diagrams
        for (const el of container.querySelectorAll('.dict-semantic-map[data-word]')) {
            const word = el.dataset.word || '';
            let synonyms, antonyms, related;
            try {
                synonyms = JSON.parse(el.dataset.synonyms || '[]');
                antonyms = JSON.parse(el.dataset.antonyms || '[]');
                related = JSON.parse(el.dataset.related || '[]');
            } catch (_e) { continue; }
            if (synonyms.length === 0 && antonyms.length === 0 && related.length === 0) continue;

            const def = buildSemanticMindmap(word, synonyms, antonyms, related);
            const svg = await renderMermaidDiagram(`dict-sm-${diagramIdx++}`, def);
            if (svg) {
                el.innerHTML = '<div class="dict-diagram-label">Semantic Map</div>'
                    + `<div class="dict-diagram-container">${svg}</div>`;
            }
        }
    }

    function pronounceWord(word) {
        if (!window.speechSynthesis) return;
        window.speechSynthesis.cancel();
        const utterance = new SpeechSynthesisUtterance(word);
        utterance.lang = 'en-US';
        window.speechSynthesis.speak(utterance);
    }

    function wireUpPronounceButtons(container) {
        container.querySelectorAll('.dict-pronounce-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                pronounceWord(btn.dataset.word);
            });
        });
    }

    function wireUpDictTags(container) {
        container.querySelectorAll('.dict-tag[data-word]').forEach(tag => {
            tag.addEventListener('click', () => {
                const tagWord = tag.dataset.word;
                const input = document.getElementById('dict-word-input');
                if (input) input.value = tagWord;
                performLookup(tagWord);
            });
        });
    }

    function addToDictHistory(word) {
        // Remove duplicate if exists
        dictHistory = dictHistory.filter(w => w.toLowerCase() !== word.toLowerCase());
        // Add to front
        dictHistory.unshift(word);
        // Keep max 50
        if (dictHistory.length > 50) dictHistory.length = 50;
        localStorage.setItem('dictHistory', JSON.stringify(dictHistory));
        renderDictHistory();
    }

    function renderDictHistory() {
        const container = document.getElementById('dict-history');
        if (!container) return;

        if (dictHistory.length === 0) {
            container.innerHTML = '<p style="color: #8b949e; font-size: 13px; padding: 8px;">No lookups yet</p>';
            return;
        }

        container.innerHTML = dictHistory.map(word =>
            `<div class="dict-history-item" data-word="${escapeHtml(word)}">${escapeHtml(word)}</div>`
        ).join('');

        container.querySelectorAll('.dict-history-item').forEach(el => {
            el.addEventListener('click', () => {
                const word = el.dataset.word;
                const input = document.getElementById('dict-word-input');
                if (input) input.value = word;
                switchTool('dictionary');
                performLookup(word);
            });
        });
    }

    function openDictionaryWithSelection() {
        const selection = window.getSelection();
        const selectedText = selection ? selection.toString().trim() : '';
        switchTool('dictionary');
        const input = document.getElementById('dict-word-input');
        if (input) {
            if (selectedText) {
                input.value = selectedText;
                performLookup(selectedText);
            }
            input.focus();
        }
    }

    // ========================================
    // Ask AI
    // ========================================

    let askAiHistory = JSON.parse(localStorage.getItem('askAiHistory') || '[]');
    let askAiCounter = 0;

    function initAskAi() {
        const input = document.getElementById('ask-ai-input');
        const btn = document.getElementById('ask-ai-btn');
        if (!input || !btn) return;

        input.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                performAskAi(input.value.trim());
            }
        });

        btn.addEventListener('click', () => {
            performAskAi(input.value.trim());
        });

        renderAskAiHistory();
        renderAskAiUsageStats();
    }

    async function performAskAi(question) {
        if (!question) return;

        const thisId = ++askAiCounter;
        const resultsEl = document.getElementById('ask-ai-results');
        if (!resultsEl) return;

        const isStale = () => thisId !== askAiCounter;

        resultsEl.innerHTML = '<div class="ask-ai-loading"><span class="dict-loading-spinner"></span> Thinking...</div>';
        addToAskAiHistory(question);

        const btn = document.getElementById('ask-ai-btn');
        if (btn) btn.disabled = true;

        try {
            const answer = await invoke('ask_ai', { question });
            if (isStale()) { if (btn) btn.disabled = false; return; }

            recordAiRequest();
            renderAskAiUsageStats();
            renderAskAiResponse(question, answer, resultsEl);
        } catch (err) {
            if (isStale()) { if (btn) btn.disabled = false; return; }
            const errStr = String(err);
            if (errStr.includes('not configured')) {
                resultsEl.innerHTML = '<div class="ask-ai-empty-state">'
                    + '<p>AI provider not configured. Set one in Settings (gear icon).</p></div>';
            } else {
                resultsEl.innerHTML = `<div class="dict-error">${escapeHtml(errStr)}</div>`;
            }
        } finally {
            if (btn) btn.disabled = false;
        }
    }

    function renderAskAiResponse(question, answer, container) {
        let html = '<div class="ask-ai-response">';
        html += `<div class="ask-ai-question"><strong>Q:</strong> ${escapeHtml(question)}</div>`;
        html += '<div class="ask-ai-answer markdown-body">';
        html += sanitizeHtml(simpleMarkdown(answer));
        html += '</div></div>';
        container.innerHTML = html;
    }

    /** Minimal markdown-to-HTML for AI responses (bold, italic, code, paragraphs, lists). */
    function simpleMarkdown(text) {
        let html = escapeHtml(text);
        // Code blocks: ```...```
        html = html.replace(/```(\w*)\n([\s\S]*?)```/g, '<pre><code>$2</code></pre>');
        // Inline code
        html = html.replace(/`([^`]+)`/g, '<code>$1</code>');
        // Bold
        html = html.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
        // Italic
        html = html.replace(/\*(.+?)\*/g, '<em>$1</em>');
        // Bullet lists
        html = html.replace(/^- (.+)$/gm, '<li>$1</li>');
        html = html.replace(/(<li>.*<\/li>\n?)+/g, '<ul>$&</ul>');
        // Numbered lists
        html = html.replace(/^\d+\. (.+)$/gm, '<li>$1</li>');
        // Headings (### ... )
        html = html.replace(/^### (.+)$/gm, '<h3>$1</h3>');
        html = html.replace(/^## (.+)$/gm, '<h2>$1</h2>');
        // Paragraphs (double newline)
        html = html.replace(/\n\n/g, '</p><p>');
        html = '<p>' + html + '</p>';
        // Clean up empty <p> tags around block elements
        html = html.replace(/<p>\s*(<(?:pre|ul|ol|h[1-6]|li))/g, '$1');
        html = html.replace(/(<\/(?:pre|ul|ol|h[1-6]|li)>)\s*<\/p>/g, '$1');
        return html;
    }

    function addToAskAiHistory(question) {
        askAiHistory = askAiHistory.filter(q => q.toLowerCase() !== question.toLowerCase());
        askAiHistory.unshift(question);
        if (askAiHistory.length > 30) askAiHistory.length = 30;
        localStorage.setItem('askAiHistory', JSON.stringify(askAiHistory));
        renderAskAiHistory();
    }

    function renderAskAiHistory() {
        const container = document.getElementById('ask-ai-history');
        if (!container) return;

        if (askAiHistory.length === 0) {
            container.innerHTML = '<p style="color: #8b949e; font-size: 13px; padding: 8px;">No questions yet</p>';
            return;
        }

        container.innerHTML = askAiHistory.map(q =>
            `<div class="dict-history-item" data-question="${escapeHtml(q)}" title="${escapeHtml(q)}">${escapeHtml(q.length > 60 ? q.slice(0, 57) + '...' : q)}</div>`
        ).join('');

        container.querySelectorAll('.dict-history-item').forEach(el => {
            el.addEventListener('click', () => {
                const q = el.dataset.question;
                const input = document.getElementById('ask-ai-input');
                if (input) input.value = q;
                switchTool('ask-ai');
                performAskAi(q);
            });
        });
    }

    function renderAskAiUsageStats() {
        const el = document.getElementById('ask-ai-stats');
        if (!el) return;

        const stats = loadAiUsageStats();
        const total = stats.total || 0;
        if (total === 0) {
            el.innerHTML = '';
            return;
        }

        const now = new Date();
        const dayKey = now.toISOString().slice(0, 10);
        const monthKey = now.toISOString().slice(0, 7);
        const today = (stats.daily && stats.daily[dayKey]) || 0;
        const month = (stats.monthly && stats.monthly[monthKey]) || 0;

        el.innerHTML = `<span class="ai-stats-label">AI requests:</span> `
            + `<span class="ai-stats-value">${today} today</span>`
            + `<span class="ai-stats-sep">/</span>`
            + `<span class="ai-stats-value">${month} this month</span>`
            + `<span class="ai-stats-sep">/</span>`
            + `<span class="ai-stats-value">${total} total</span>`;
    }

    function openAskAiWithSelection() {
        const selection = window.getSelection();
        const selectedText = selection ? selection.toString().trim() : '';
        switchTool('ask-ai');
        const input = document.getElementById('ask-ai-input');
        if (input) {
            if (selectedText) {
                input.value = selectedText;
            }
            input.focus();
        }
    }

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
})();
