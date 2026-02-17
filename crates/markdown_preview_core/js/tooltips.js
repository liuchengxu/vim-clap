// Tooltips - Symbol identification, link previews, and file path tooltips
// Provides hover tooltips for Greek letters, math symbols, links, and recent file paths

function renderInlineCode(text) {
    return escapeHtml(text).replace(/`([^`]+)`/g, '<code>$1</code>');
}

// Custom tooltip for recent files
let pathTooltip = null;
let tooltipTimeout = null;
let tooltipHoveredElement = null;  // tracks which item the mouse is currently over
// Cache for markdown titles to avoid repeated file reads
const markdownTitleCache = new Map();

function createPathTooltip() {
    if (pathTooltip) return pathTooltip;

    pathTooltip = document.createElement('div');
    pathTooltip.className = 'path-tooltip';
    pathTooltip.innerHTML = '<div class="path-tooltip-content"></div>';
    document.body.appendChild(pathTooltip);

    return pathTooltip;
}

// Check if a path is a markdown file
function isMarkdownFile(path) {
    const ext = path.split('.').pop().toLowerCase();
    return ['md', 'markdown', 'mdown', 'mkdn', 'mkd'].includes(ext);
}

// Fetch file preview info with caching (Tauri only)
// Returns { title, modified_at } for the tooltip
async function getFilePreviewInfo(path) {
    // Return cached value if available
    if (markdownTitleCache.has(path)) {
        return markdownTitleCache.get(path);
    }

    try {
        // Check if we're in Tauri environment
        if (window.__TAURI__ && window.__TAURI__.core) {
            const info = await window.__TAURI__.core.invoke('get_file_preview_info', { path });
            markdownTitleCache.set(path, info);
            return info;
        }
    } catch (e) {
        console.error('Failed to get file preview info:', e);
    }

    return { title: null, modified_at: null };
}

function showPathTooltip(element, fullPath, previewInfo = {}) {
    const tooltip = createPathTooltip();
    const content = tooltip.querySelector('.path-tooltip-content');

    const { title, digest, modified_at } = previewInfo;

    // Build tooltip content
    let html = '';
    if (title) {
        // Render backtick-wrapped text as inline code
        const rendered = renderInlineCode(title);
        html += `<div class="path-tooltip-title">${rendered}</div>`;
    }
    html += `<div class="path-tooltip-path">${escapeHtml(fullPath)}</div>`;

    // Add modification time if available
    if (modified_at) {
        const relativeTime = formatRelativeTime(modified_at);
        const fullDate = new Date(modified_at).toLocaleString();
        html += `<div class="path-tooltip-modified" title="${fullDate}">Modified ${relativeTime}</div>`;
    }

    // Add digest if available (digest is { text, source })
    if (digest && digest.text) {
        const isAi = digest.source === 'ai';

        if (isAi) {
            // AI summary: single paragraph, show with AI badge
            html += `<div class="path-tooltip-digest path-tooltip-digest-ai">`;
            html += `<span class="path-tooltip-ai-badge">AI</span>`;
            html += `<span class="path-tooltip-digest-text">${renderInlineCode(digest.text)}</span>`;
            html += `</div>`;
        } else {
            // Text-based digest: multi-line with headings
            const digestLines = digest.text.split('\n').map(line => {
                if (line.startsWith('# ')) {
                    return `<div class="path-tooltip-digest-heading">${renderInlineCode(line.slice(2))}</div>`;
                }
                return `<div class="path-tooltip-digest-text">${renderInlineCode(line)}</div>`;
            }).join('');
            html += `<div class="path-tooltip-digest">${digestLines}</div>`;
        }
    }

    content.innerHTML = html;

    // Position tooltip to the right of the element (offset to avoid sidebar border)
    const rect = element.getBoundingClientRect();
    const gap = 20;
    const margin = 16;
    const availableWidth = window.innerWidth - rect.right - gap - margin;
    // Use ~40% of viewport as ideal width, clamped to available space
    const idealWidth = Math.round(window.innerWidth * 0.4);
    content.style.width = `${Math.min(idealWidth, availableWidth)}px`;
    tooltip.style.left = `${rect.right + gap}px`;
    tooltip.style.top = `${rect.top}px`;
    tooltip.classList.add('visible');
}

function hidePathTooltip() {
    if (pathTooltip) {
        pathTooltip.classList.remove('visible');
    }
}

// Greek letter tooltip for unfamiliar characters
const GREEK_LETTERS = {
    // Lowercase
    '\u03b1': 'alpha (AL-fuh)',
    '\u03b2': 'beta (BAY-tuh)',
    '\u03b3': 'gamma (GAM-uh)',
    '\u03b4': 'delta (DEL-tuh)',
    '\u03b5': 'epsilon (EP-sih-lon)',
    '\u03b6': 'zeta (ZAY-tuh)',
    '\u03b7': 'eta (AY-tuh)',
    '\u03b8': 'theta (THAY-tuh)',
    '\u03b9': 'iota (eye-OH-tuh)',
    '\u03ba': 'kappa (KAP-uh)',
    '\u03bb': 'lambda (LAM-duh)',
    '\u03bc': 'mu (MYOO)',
    '\u03bd': 'nu (NOO)',
    '\u03be': 'xi (KSEE / ZYE)',
    '\u03bf': 'omicron (OM-ih-kron)',
    '\u03c0': 'pi (PIE)',
    '\u03c1': 'rho (ROH)',
    '\u03c3': 'sigma (SIG-muh)',
    '\u03c2': 'sigma final (SIG-muh)',
    '\u03c4': 'tau (TAW / TOW)',
    '\u03c5': 'upsilon (OOP-sih-lon)',
    '\u03c6': 'phi (FYE / FEE)',
    '\u03c7': 'chi (KYE / KHEE)',
    '\u03c8': 'psi (PSYE / SEE)',
    '\u03c9': 'omega (oh-MAY-guh)',
    // Uppercase
    '\u0391': 'Alpha (AL-fuh)',
    '\u0392': 'Beta (BAY-tuh)',
    '\u0393': 'Gamma (GAM-uh)',
    '\u0394': 'Delta (DEL-tuh)',
    '\u0395': 'Epsilon (EP-sih-lon)',
    '\u0396': 'Zeta (ZAY-tuh)',
    '\u0397': 'Eta (AY-tuh)',
    '\u0398': 'Theta (THAY-tuh)',
    '\u0399': 'Iota (eye-OH-tuh)',
    '\u039a': 'Kappa (KAP-uh)',
    '\u039b': 'Lambda (LAM-duh)',
    '\u039c': 'Mu (MYOO)',
    '\u039d': 'Nu (NOO)',
    '\u039e': 'Xi (KSEE / ZYE)',
    '\u039f': 'Omicron (OM-ih-kron)',
    '\u03a0': 'Pi (PIE)',
    '\u03a1': 'Rho (ROH)',
    '\u03a3': 'Sigma (SIG-muh)',
    '\u03a4': 'Tau (TAW / TOW)',
    '\u03a5': 'Upsilon (OOP-sih-lon)',
    '\u03a6': 'Phi (FYE / FEE)',
    '\u03a7': 'Chi (KYE / KHEE)',
    '\u03a8': 'Psi (PSYE / SEE)',
    '\u03a9': 'Omega (oh-MAY-guh)',
    // Common math variants
    '\u03d5': 'phi variant (FYE / FEE)',
    '\u03d1': 'theta variant (THAY-tuh)',
    '\u03f5': 'epsilon variant (EP-sih-lon)',
    '\u03f0': 'kappa variant (KAP-uh)',
    '\u03f1': 'rho variant (ROH)',
    '\u03d6': 'pi variant (PIE)',
    // Hebrew (common in math)
    '\u2135': 'aleph (AH-lef)',
    '\u2136': 'beth (BET)',
    '\u2137': 'gimel (GIM-el)',
    '\u2138': 'daleth (DAH-let)',
    // Common math symbols
    '\u221e': 'infinity',
    '\u2202': 'partial derivative',
    '\u2207': 'nabla / del',
    '\u2211': 'summation',
    '\u220f': 'product',
    '\u222b': 'integral',
    '\u222e': 'contour integral',
    '\u221a': 'square root',
    '\u221d': 'proportional to',
    '\u2208': 'element of',
    '\u2209': 'not element of',
    '\u2282': 'subset of',
    '\u2283': 'superset of',
    '\u2286': 'subset or equal',
    '\u2287': 'superset or equal',
    '\u222a': 'union',
    '\u2229': 'intersection',
    '\u2205': 'empty set',
    '\u2200': 'for all',
    '\u2203': 'there exists',
    '\u2204': 'there does not exist',
    '\u2227': 'logical and',
    '\u2228': 'logical or',
    '\u00ac': 'logical not',
    '\u2295': 'xor / direct sum',
    '\u2297': 'tensor product',
    '\u2248': 'approximately equal',
    '\u2260': 'not equal',
    '\u2264': 'less than or equal',
    '\u2265': 'greater than or equal',
    '\u226a': 'much less than',
    '\u226b': 'much greater than',
    '\u2261': 'identical / congruent',
    '\u2262': 'not identical',
    '\u27e8': 'left angle bracket',
    '\u27e9': 'right angle bracket',
    '\u2020': 'dagger / adjoint',
    '\u2021': 'double dagger',
    '\u22a5': 'perpendicular / bottom',
    '\u2225': 'parallel',
    '\u2220': 'angle',
    '\u00b0': 'degree',
    '\u2032': 'prime',
    '\u2033': 'double prime',
    '\u210f': 'h-bar (Planck constant)',
    '\u2113': 'script l',
    '\u211c': 'real part',
    '\u2111': 'imaginary part',
    '\u2118': 'Weierstrass p',
};

let symbolTooltip = null;

function createSymbolTooltip() {
    if (!symbolTooltip) {
        symbolTooltip = document.createElement('div');
        symbolTooltip.className = 'symbol-tooltip';
        document.body.appendChild(symbolTooltip);
    }
    return symbolTooltip;
}

function showSymbolTooltip(event, char, description) {
    const tooltip = createSymbolTooltip();
    tooltip.innerHTML = `<span class="symbol-char">${char}</span> — ${description}`;

    // Position near cursor
    const x = event.clientX + 12;
    const y = event.clientY + 12;

    tooltip.style.left = `${x}px`;
    tooltip.style.top = `${y}px`;
    tooltip.classList.add('visible');

    // Ensure tooltip stays in viewport
    requestAnimationFrame(() => {
        const rect = tooltip.getBoundingClientRect();
        if (rect.right > window.innerWidth) {
            tooltip.style.left = `${event.clientX - rect.width - 8}px`;
        }
        if (rect.bottom > window.innerHeight) {
            tooltip.style.top = `${event.clientY - rect.height - 8}px`;
        }
    });
}

function hideSymbolTooltip() {
    if (symbolTooltip) {
        symbolTooltip.classList.remove('visible');
    }
}

function setupSymbolTooltips(container) {
    let currentChar = null;

    container.addEventListener('mousemove', (e) => {
        // Get character under cursor using document.caretPositionFromPoint or caretRangeFromPoint
        let range;
        if (document.caretPositionFromPoint) {
            const pos = document.caretPositionFromPoint(e.clientX, e.clientY);
            if (pos && pos.offsetNode && pos.offsetNode.nodeType === Node.TEXT_NODE) {
                range = document.createRange();
                range.setStart(pos.offsetNode, pos.offset);
                range.setEnd(pos.offsetNode, Math.min(pos.offset + 1, pos.offsetNode.length));
            }
        } else if (document.caretRangeFromPoint) {
            range = document.caretRangeFromPoint(e.clientX, e.clientY);
            if (range && range.startContainer.nodeType === Node.TEXT_NODE) {
                range.setEnd(range.startContainer, Math.min(range.startOffset + 1, range.startContainer.length));
            }
        }

        if (range && range.startContainer.nodeType === Node.TEXT_NODE) {
            const text = range.startContainer.textContent;
            const offset = range.startOffset;
            if (offset < text.length) {
                const char = text[offset];
                const description = GREEK_LETTERS[char];
                if (description) {
                    if (char !== currentChar) {
                        currentChar = char;
                        showSymbolTooltip(e, char, description);
                    }
                    return;
                }
            }
        }

        // No matching symbol under cursor
        if (currentChar) {
            currentChar = null;
            hideSymbolTooltip();
        }
    });

    container.addEventListener('mouseleave', () => {
        currentChar = null;
        hideSymbolTooltip();
    });
}

// ============================================================================
// Link Hover Tooltips
// ============================================================================

let linkTooltip = null;
let linkTooltipTimeout = null;

function createLinkTooltip() {
    if (!linkTooltip) {
        linkTooltip = document.createElement('div');
        linkTooltip.className = 'link-tooltip';
        document.body.appendChild(linkTooltip);
    }
    return linkTooltip;
}

function classifyUrl(href) {
    if (!href) return { type: 'unknown', display: '' };

    // Anchor link
    if (href.startsWith('#')) {
        const id = href.slice(1);
        const target = document.getElementById(id);
        const heading = target ? target.textContent.trim() : id;
        return { type: 'anchor', display: heading, icon: '#' };
    }

    try {
        const url = new URL(href, window.location.href);

        // External link
        if (url.protocol === 'http:' || url.protocol === 'https:') {
            // GitHub
            const gh = url.hostname === 'github.com' || url.hostname === 'www.github.com';
            if (gh) {
                const parts = url.pathname.split('/').filter(Boolean);
                if (parts.length >= 2) {
                    const repo = `${parts[0]}/${parts[1]}`;
                    const rest = parts.slice(2).join('/');
                    return { type: 'github', display: rest ? `${repo} / ${rest}` : repo, url: href };
                }
            }

            // Generic external
            const path = url.pathname === '/' ? '' : url.pathname;
            const display = url.hostname + path + (url.hash || '');
            return { type: 'external', display, url: href };
        }

        // mailto
        if (url.protocol === 'mailto:') {
            return { type: 'email', display: href.replace('mailto:', ''), icon: 'mail' };
        }
    } catch {
        // Relative path / file link
    }

    // Relative link (file path)
    return { type: 'file', display: href, icon: 'file' };
}

function showLinkTooltip(anchor, event) {
    const href = anchor.getAttribute('href');
    if (!href) return;

    const info = classifyUrl(href);
    const tooltip = createLinkTooltip();

    // Build tooltip content
    let icon = '';
    switch (info.type) {
        case 'anchor':
            icon = '<svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor"><path fill-rule="evenodd" d="M7.775 3.275a.75.75 0 001.06 1.06l1.25-1.25a2 2 0 112.83 2.83l-2.5 2.5a2 2 0 01-2.83 0 .75.75 0 00-1.06 1.06 3.5 3.5 0 004.95 0l2.5-2.5a3.5 3.5 0 00-4.95-4.95l-1.25 1.25zm-4.69 9.64a2 2 0 010-2.83l2.5-2.5a2 2 0 012.83 0 .75.75 0 001.06-1.06 3.5 3.5 0 00-4.95 0l-2.5 2.5a3.5 3.5 0 004.95 4.95l1.25-1.25a.75.75 0 00-1.06-1.06l-1.25 1.25a2 2 0 01-2.83 0z"/></svg>';
            break;
        case 'github':
            icon = '<svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/></svg>';
            break;
        case 'email':
            icon = '<svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor"><path d="M1.75 2h12.5c.966 0 1.75.784 1.75 1.75v8.5A1.75 1.75 0 0114.25 14H1.75A1.75 1.75 0 010 12.25v-8.5C0 2.784.784 2 1.75 2zM1.5 3.75v.736l6.5 3.9 6.5-3.9V3.75a.25.25 0 00-.25-.25H1.75a.25.25 0 00-.25.25zm13 2.164l-6.5 3.9-6.5-3.9v6.336c0 .138.112.25.25.25h12.5a.25.25 0 00.25-.25V5.914z"/></svg>';
            break;
        default:
            icon = '<svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor"><path d="M3.75 2h3.5a.75.75 0 010 1.5h-3.5a.25.25 0 00-.25.25v8.5c0 .138.112.25.25.25h8.5a.25.25 0 00.25-.25v-3.5a.75.75 0 011.5 0v3.5A1.75 1.75 0 0112.25 14h-8.5A1.75 1.75 0 012 12.25v-8.5C2 2.784 2.784 2 3.75 2zm6.854-.22a.75.75 0 011.396.53v3.94a.75.75 0 01-1.5 0V4.56L7.28 7.78a.75.75 0 01-1.06-1.06l3.22-3.22H7.75a.75.75 0 010-1.5h2.604l.25-.22z"/></svg>';
            break;
    }

    tooltip.innerHTML = `<span class="link-tooltip-icon">${icon}</span><span class="link-tooltip-url">${escapeHtml(info.display)}</span>`;

    // Position below the link
    const rect = anchor.getBoundingClientRect();
    tooltip.style.left = `${rect.left}px`;
    tooltip.style.top = `${rect.bottom + 6}px`;
    tooltip.classList.add('visible');

    // Ensure tooltip stays in viewport
    requestAnimationFrame(() => {
        const tr = tooltip.getBoundingClientRect();
        if (tr.right > window.innerWidth - 8) {
            tooltip.style.left = `${window.innerWidth - tr.width - 8}px`;
        }
        if (tr.bottom > window.innerHeight - 8) {
            tooltip.style.top = `${rect.top - tr.height - 6}px`;
        }
    });
}

function hideLinkTooltip() {
    if (linkTooltip) {
        linkTooltip.classList.remove('visible');
    }
}

function setupLinkTooltips(container) {
    let currentAnchor = null;

    container.addEventListener('mouseover', (e) => {
        const anchor = e.target.closest('.markdown-body a:not(.heading-anchor)');
        if (!anchor || anchor === currentAnchor) return;

        currentAnchor = anchor;
        clearTimeout(linkTooltipTimeout);
        linkTooltipTimeout = setTimeout(() => {
            showLinkTooltip(anchor, e);
        }, 300);
    });

    container.addEventListener('mouseout', (e) => {
        const anchor = e.target.closest('.markdown-body a:not(.heading-anchor)');
        if (anchor === currentAnchor || (!anchor && currentAnchor)) {
            const related = e.relatedTarget;
            if (related && currentAnchor && currentAnchor.contains(related)) return;
            currentAnchor = null;
            clearTimeout(linkTooltipTimeout);
            hideLinkTooltip();
        }
    });
}
