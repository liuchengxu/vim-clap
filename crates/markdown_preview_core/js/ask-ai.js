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
