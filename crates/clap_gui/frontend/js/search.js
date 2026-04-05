let searchTimeout = null;

function debounceSearch(query) {
  if (searchTimeout) clearTimeout(searchTimeout);
  searchTimeout = setTimeout(() => runSearch(query), 50);
}

async function runSearch(query) {
  if (!query) {
    clearResults();
    return;
  }

  try {
    if (currentMode === 'files') {
      results = await invoke('search_files', { query });
    } else {
      results = await invoke('search_grep', { query });
    }
    selectedIndex = 0;
    renderResults();
    if (results.length > 0) {
      loadPreview(results[0]);
    }
  } catch (e) {
    console.error('Search error:', e);
  }
}

function renderResults() {
  const pane = document.getElementById('results-pane');
  pane.innerHTML = '';

  results.forEach((result, index) => {
    const el = document.createElement('div');
    el.className = 'result-item' + (index === selectedIndex ? ' selected' : '');
    el.dataset.index = index;

    if (currentMode === 'files') {
      el.innerHTML = renderFileResult(result);
    } else {
      el.innerHTML = renderGrepResult(result);
    }

    el.addEventListener('click', () => {
      selectedIndex = index;
      renderResults();
      loadPreview(result);
    });

    el.addEventListener('dblclick', () => {
      openResult(result);
    });

    pane.appendChild(el);
  });

  document.getElementById('status-count').textContent =
    `${results.length} result${results.length !== 1 ? 's' : ''}`;

  const selected = pane.querySelector('.selected');
  if (selected) selected.scrollIntoView({ block: 'nearest' });
}

function renderFileResult(result) {
  const highlighted = highlightMatches(result.display_path, result.match_indices);
  const parts = result.display_path.split('/');
  const dir = parts.length > 1 ? parts.slice(0, -1).join('/') + '/' : '';
  return `
    <span class="result-icon">${result.icon}</span>
    <span class="result-text">${highlighted}</span>
    <span class="result-path-dim">${escapeHtml(dir)}</span>
  `;
}

function renderGrepResult(result) {
  return `
    <span class="result-line-number">${result.line_number}</span>
    <span class="result-text">${escapeHtml(result.line_content)}</span>
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
