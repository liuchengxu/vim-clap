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
