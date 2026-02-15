// Fuzzy Finder - Search headings and full text
// Provides in-document search with fuzzy matching

function fuzzyMatch(pattern, text) {
    if (!pattern) return { score: 0, positions: [] };

    const patternLower = pattern.toLowerCase();
    const textLower = text.toLowerCase();

    let patternIdx = 0;
    let score = 0;
    let positions = [];
    let lastMatchIdx = -1;

    for (let i = 0; i < text.length && patternIdx < pattern.length; i++) {
        if (textLower[i] === patternLower[patternIdx]) {
            positions.push(i);

            if (lastMatchIdx === i - 1) {
                score += 10;
            }

            if (i === 0 || /[\s\-_./]/.test(text[i - 1])) {
                score += 5;
            }

            if (text[i] === pattern[patternIdx]) {
                score += 2;
            }

            score += 1;
            lastMatchIdx = i;
            patternIdx++;
        }
    }

    if (patternIdx !== pattern.length) {
        return { score: 0, positions: [] };
    }

    score += Math.max(0, 50 - text.length);

    return { score, positions };
}

function highlightMatches(text, positions) {
    if (!positions.length) return escapeHtml(text);

    let result = '';
    let lastIdx = 0;

    for (const pos of positions) {
        result += escapeHtml(text.slice(lastIdx, pos));
        result += `<span class="fuzzy-match">${escapeHtml(text[pos])}</span>`;
        lastIdx = pos + 1;
    }
    result += escapeHtml(text.slice(lastIdx));

    return result;
}

function buildFuzzyIndex() {
    const content = document.getElementById('content');
    if (!content) return;

    fuzzySearchIndex = { headings: [], text: [] };

    const headings = content.querySelectorAll('h1, h2, h3, h4, h5, h6');
    headings.forEach((heading, index) => {
        const text = heading.textContent.replace(/^\s*🔗?\s*/, '').trim();
        const level = heading.tagName.toLowerCase();
        fuzzySearchIndex.headings.push({
            type: level.toUpperCase(),
            text: text,
            element: heading,
            id: heading.id || `heading-${index}`
        });
    });

    const textElements = content.querySelectorAll('p, li, pre, blockquote, td, th');
    textElements.forEach((el, index) => {
        const text = el.textContent.trim();
        if (text.length > 10 && text.length < 500) {
            let context = '';
            let prevEl = el.previousElementSibling;
            while (prevEl) {
                if (/^H[1-6]$/.test(prevEl.tagName)) {
                    context = prevEl.textContent.replace(/^\s*🔗?\s*/, '').trim();
                    break;
                }
                prevEl = prevEl.previousElementSibling;
            }

            fuzzySearchIndex.text.push({
                type: el.tagName === 'PRE' ? 'CODE' : 'TEXT',
                text: text.slice(0, 200),
                fullText: text,
                element: el,
                context: context
            });
        }
    });
}

function fuzzySearch(query) {
    const index = fuzzySearchMode === 'headings' ? fuzzySearchIndex.headings : fuzzySearchIndex.text;

    if (!query) {
        fuzzyResults = index.slice(0, 50).map(item => ({
            ...item,
            score: 100,
            positions: []
        }));
    } else {
        fuzzyResults = index
            .map(item => {
                const { score, positions } = fuzzyMatch(query, item.text);
                return { ...item, score, positions };
            })
            .filter(item => item.score > 0)
            .sort((a, b) => b.score - a.score)
            .slice(0, 50);
    }

    fuzzySelectedIndex = 0;
    renderFuzzyResults();
}

function renderFuzzyResults() {
    const resultsContainer = document.getElementById('fuzzy-results');
    const countSpan = document.getElementById('fuzzy-result-count');

    if (!fuzzyResults.length) {
        resultsContainer.innerHTML = '<div class="fuzzy-no-results">No results found</div>';
        countSpan.textContent = '0 results';
        return;
    }

    countSpan.textContent = `${fuzzyResults.length} result${fuzzyResults.length !== 1 ? 's' : ''}`;

    resultsContainer.innerHTML = fuzzyResults.map((result, index) => {
        const isSelected = index === fuzzySelectedIndex;
        const highlightedText = highlightMatches(result.text, result.positions);
        const context = result.context ? `<div class="fuzzy-result-context">in: ${escapeHtml(result.context)}</div>` : '';

        return `
            <div class="fuzzy-result-item${isSelected ? ' selected' : ''}" data-index="${index}">
                <div class="fuzzy-result-title">
                    <span class="fuzzy-result-type">${result.type}</span>
                    ${highlightedText}
                </div>
                ${context}
            </div>
        `;
    }).join('');

    const selectedItem = resultsContainer.querySelector('.fuzzy-result-item.selected');
    if (selectedItem) {
        selectedItem.scrollIntoView({ block: 'nearest' });
    }
}

function openFuzzyFinder() {
    buildFuzzyIndex();
    fuzzyFinderOpen = true;
    fuzzySelectedIndex = 0;

    const overlay = document.getElementById('fuzzy-finder');
    const input = document.getElementById('fuzzy-input');

    overlay.classList.add('visible');
    input.value = '';
    input.placeholder = fuzzySearchMode === 'headings' ? 'Search headings...' : 'Search full text...';

    fuzzySearch('');

    setTimeout(() => input.focus(), 50);
}

function closeFuzzyFinder() {
    fuzzyFinderOpen = false;
    const overlay = document.getElementById('fuzzy-finder');
    overlay.classList.remove('visible');
}

function toggleFuzzyMode() {
    fuzzySearchMode = fuzzySearchMode === 'headings' ? 'text' : 'headings';

    const headingsBtn = document.getElementById('fuzzy-mode-headings');
    const textBtn = document.getElementById('fuzzy-mode-text');
    const input = document.getElementById('fuzzy-input');

    headingsBtn.classList.toggle('active', fuzzySearchMode === 'headings');
    textBtn.classList.toggle('active', fuzzySearchMode === 'text');
    input.placeholder = fuzzySearchMode === 'headings' ? 'Search headings...' : 'Search full text...';

    fuzzySearch(input.value);
}

function selectFuzzyResult() {
    if (fuzzyResults.length === 0) return;

    const result = fuzzyResults[fuzzySelectedIndex];
    if (result && result.element) {
        closeFuzzyFinder();

        if (result.id) {
            navigateToHeading(result.id);
        } else {
            result.element.scrollIntoView({ behavior: 'smooth', block: 'center' });

            result.element.style.backgroundColor = '#fff8c5';
            result.element.style.transition = 'background-color 0.3s';
            setTimeout(() => {
                result.element.style.backgroundColor = '';
            }, 1500);
        }
    }
}

function handleFuzzyKeydown(e) {
    if (!fuzzyFinderOpen) return;

    switch (e.key) {
        case 'Escape':
            e.preventDefault();
            closeFuzzyFinder();
            break;

        case 'ArrowDown':
            e.preventDefault();
            if (fuzzySelectedIndex < fuzzyResults.length - 1) {
                fuzzySelectedIndex++;
                renderFuzzyResults();
            }
            break;

        case 'ArrowUp':
            e.preventDefault();
            if (fuzzySelectedIndex > 0) {
                fuzzySelectedIndex--;
                renderFuzzyResults();
            }
            break;

        case 'Enter':
            e.preventDefault();
            selectFuzzyResult();
            break;

        case 'Tab':
            e.preventDefault();
            toggleFuzzyMode();
            break;
    }
}

function initFuzzyFinder() {
    const overlay = document.getElementById('fuzzy-finder');
    const input = document.getElementById('fuzzy-input');
    const resultsContainer = document.getElementById('fuzzy-results');
    const headingsBtn = document.getElementById('fuzzy-mode-headings');
    const textBtn = document.getElementById('fuzzy-mode-text');

    document.addEventListener('keydown', (e) => {
        // Skip when terminal is focused — let terminal handle all keys
        if (typeof isTerminalPanelFocused === 'function' && isTerminalPanelFocused()) return;

        if ((e.ctrlKey || e.metaKey) && e.key === 'p') {
            e.preventDefault();
            if (fuzzyFinderOpen) {
                closeFuzzyFinder();
            } else {
                openFuzzyFinder();
            }
        }

        if (fuzzyFinderOpen) {
            handleFuzzyKeydown(e);
        }
    });

    overlay.addEventListener('click', (e) => {
        if (e.target === overlay) {
            closeFuzzyFinder();
        }
    });

    input.addEventListener('input', (e) => {
        fuzzySearch(e.target.value);
    });

    resultsContainer.addEventListener('click', (e) => {
        const item = e.target.closest('.fuzzy-result-item');
        if (item) {
            fuzzySelectedIndex = parseInt(item.dataset.index);
            selectFuzzyResult();
        }
    });

    headingsBtn.addEventListener('click', () => {
        if (fuzzySearchMode !== 'headings') {
            toggleFuzzyMode();
        }
    });

    textBtn.addEventListener('click', () => {
        if (fuzzySearchMode !== 'text') {
            toggleFuzzyMode();
        }
    });
}
