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
