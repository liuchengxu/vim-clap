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
