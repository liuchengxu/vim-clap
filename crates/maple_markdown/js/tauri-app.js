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

    // Get dialog API (Tauri 2.x)
    function getDialogOpen() {
        return window.__TAURI__.dialog?.open || window.__TAURI__.plugin?.dialog?.open;
    }

    // Open file dialog and load selected file
    async function openFileDialog() {
        const open = getDialogOpen();
        if (!open) {
            console.error('Dialog API not available');
            return null;
        }

        try {
            const selected = await open({
                multiple: false,
                filters: [{ name: 'Markdown', extensions: ['md', 'markdown', 'mdown', 'mkdn', 'mkd'] }]
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
            if (result && result.html) {
                handleFileOpened(result);
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
                return result;
            }
        } catch (e) {
            console.error('Failed to open URL:', e);
            showToast('Failed to open URL: ' + (e.message || e));
        }
        return null;
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

    // Load recent files from backend and render them
    async function loadRecentFilesFromBackend() {
        try {
            const files = await invoke('get_recent_files');
            // Convert backend format to the format expected by renderRecentFiles
            const recentFiles = files.map(path => ({ path, timestamp: Date.now() }));
            // Store in localStorage so renderRecentFiles can use it
            localStorage.setItem('recentFiles', JSON.stringify(recentFiles));
            renderRecentFiles(switchToFile);
        } catch (e) {
            console.error('Failed to load recent files from backend:', e);
        }
    }

    // Handle file opened result
    function handleFileOpened(result) {
        const content = document.getElementById('content');
        content.innerHTML = result.html;

        codeHighlight();
        renderMermaid();
        renderLatex();
        addHeadingAnchors();
        generateTOC();

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

        if (result.stats) {
            updateDocumentStats(result.stats);
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

        listen('menu-toggle-toc', () => {
            const tocModeSelect = document.getElementById('toc-mode');
            if (tocModeSelect) {
                const current = tocModeSelect.value;
                tocModeSelect.value = current === 'off' ? 'right' : 'off';
                tocModeSelect.dispatchEvent(new Event('change'));
            }
        });

        listen('menu-theme', (event) => {
            const themeSelect = document.getElementById('theme-select');
            if (themeSelect) {
                const themeMap = { 'light': 'github-light', 'dark': 'github-dark', 'auto': 'auto' };
                themeSelect.value = themeMap[event.payload] || event.payload;
                themeSelect.dispatchEvent(new Event('change'));
            }
        });

        // Listen for initial file from command line argument
        listen('open-initial-file', async (event) => {
            console.log('Opening initial file:', event.payload);
            await openFile(event.payload);
        });
    }

    // Set up drag and drop
    function setupDragAndDrop() {
        document.addEventListener('drop', async (e) => {
            e.preventDefault();
            const files = e.dataTransfer?.files;
            if (files && files.length > 0) {
                const file = files[0];
                if (file.name.match(/\.(md|markdown|mdown|mkdn|mkd)$/i)) {
                    const path = file.path || file.name;
                    await openFile(path);
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

    // Open path input modal
    function openPathInput() {
        if (pathInputVisible) return;
        pathInputVisible = true;

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
        input.value = '';
        autocompleteState.items = [];
        autocompleteState.selectedIndex = -1;
        const dropdown = document.getElementById('path-autocomplete');
        if (dropdown) dropdown.style.display = 'none';
        setTimeout(() => input.focus(), 50);
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
                }
            });
        }
    }

    // Initialize on DOM ready
    document.addEventListener('DOMContentLoaded', async function() {
        // Initialize core UI with file switch callback
        initCoreUI({
            onFileClick: switchToFile
        });

        // Load recent files from backend (persistent storage)
        await loadRecentFilesFromBackend();

        // Set up Tauri-specific features
        setupTauriListeners();
        setupDragAndDrop();
        setupWelcomeScreen();
        setupKeyboardShortcuts();
        setupClipboardMonitoring();
    });
})();
