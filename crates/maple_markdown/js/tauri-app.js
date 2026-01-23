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
            showToast('Failed to open file: ' + e.message);
        }
        return null;
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

            // Add to recent files
            addToRecentFiles(result.file_path);
            renderRecentFiles(switchToFile);

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
            // Cmd+O / Ctrl+O to open file dialog
            if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key === 'o') {
                e.preventDefault();
                await openFileDialog();
            }

            // Cmd+Shift+O / Ctrl+Shift+O to open file by path
            if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'O' || e.key === 'o')) {
                e.preventDefault();
                openPathInput();
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
        });
    }

    // Path input state
    let pathInputVisible = false;

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
                        <label>Open file by path</label>
                        <span class="path-input-hint">Enter absolute path to markdown file</span>
                    </div>
                    <input type="text" id="path-input-field" class="path-input-field"
                           placeholder="/path/to/file.md" autocomplete="off" spellcheck="false">
                    <div class="path-input-footer">
                        <span class="key">Enter</span> open
                        <span class="key">Esc</span> cancel
                        <span class="key">⌘V</span> paste
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
                    padding-top: 20vh;
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
                    width: 500px;
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
                .path-input-field {
                    width: 100%;
                    padding: 12px 16px;
                    border: none;
                    font-size: 14px;
                    font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace;
                    outline: none;
                    box-sizing: border-box;
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

            // Handle input
            const input = document.getElementById('path-input-field');
            input.addEventListener('keydown', async (e) => {
                if (e.key === 'Enter') {
                    e.preventDefault();
                    const path = input.value.trim();
                    if (path) {
                        closePathInput();
                        await openFile(path);
                    }
                } else if (e.key === 'Escape') {
                    closePathInput();
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
                                // Insert at cursor position
                                const start = input.selectionStart;
                                const end = input.selectionEnd;
                                const before = input.value.substring(0, start);
                                const after = input.value.substring(end);
                                input.value = before + text + after;
                                input.selectionStart = input.selectionEnd = start + text.length;
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
        setTimeout(() => input.focus(), 50);
    }

    // Close path input modal
    function closePathInput() {
        pathInputVisible = false;
        const modal = document.getElementById('path-input-modal');
        if (modal) {
            modal.classList.remove('visible');
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
    document.addEventListener('DOMContentLoaded', function() {
        // Initialize core UI with file switch callback
        initCoreUI({
            onFileClick: switchToFile
        });

        // Set up Tauri-specific features
        setupTauriListeners();
        setupDragAndDrop();
        setupWelcomeScreen();
        setupKeyboardShortcuts();
        setupClipboardMonitoring();
    });
})();
