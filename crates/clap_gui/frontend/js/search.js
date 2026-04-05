let searchTimeout = null;

function debounceSearch(query) {
  if (searchTimeout) clearTimeout(searchTimeout);
  searchTimeout = setTimeout(() => startSearch(query), 50);
}

// Fire-and-forget: kicks off a search, results arrive via 'search-results' event
async function startSearch(query) {
  if (!query) {
    clearResults();
    return;
  }

  try {
    await invoke('start_search', { query, mode: currentMode });
  } catch (e) {
    console.error('Search error:', e);
  }
}

// Render results — called from the event listener in app.js
function renderResults(isFiles) {
  const pane = document.getElementById('results-pane');
  pane.innerHTML = '';

  results.forEach((result, index) => {
    const el = document.createElement('div');
    el.className = 'result-item' + (index === selectedIndex ? ' selected' : '');
    el.dataset.index = index;

    if (isFiles) {
      el.innerHTML = renderFileResult(result);
    } else {
      el.innerHTML = renderGrepResult(result);
    }

    el.addEventListener('click', () => {
      selectedIndex = index;
      renderResults(isFiles);
      loadPreview(result);
    });

    el.addEventListener('dblclick', () => {
      openResult(result);
    });

    pane.appendChild(el);
  });

  const selected = pane.querySelector('.selected');
  if (selected) selected.scrollIntoView({ block: 'nearest' });
}

function renderFileResult(result) {
  const highlighted = highlightMatches(result.display_path, result.match_indices);
  const parts = result.display_path.split('/');
  const dir = parts.length > 1 ? parts.slice(0, -1).join('/') + '/' : '';
  return `
    <span class="result-icon">${escapeHtml(result.icon)}</span>
    <span class="result-text">${highlighted}</span>
    <span class="result-path-dim">${escapeHtml(dir)}</span>
  `;
}

function renderGrepResult(result) {
  const highlighted = highlightMatches(result.line_content, result.match_indices);
  return `
    <span class="result-line-number">${result.line_number}</span>
    <span class="result-text">${highlighted}</span>
  `;
}

function highlightMatches(text, indices) {
  if (!indices || indices.length === 0) return escapeHtml(text);
  const indexSet = new Set(indices);
  let html = '';
  for (let i = 0; i < text.length; i++) {
    const ch = escapeHtml(text[i]);
    if (indexSet.has(i)) {
      html += `<span class="match">${ch}</span>`;
    } else {
      html += ch;
    }
  }
  return html;
}

function escapeHtml(str) {
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}

document.getElementById('search-input').addEventListener('input', (e) => {
  debounceSearch(e.target.value);
});
