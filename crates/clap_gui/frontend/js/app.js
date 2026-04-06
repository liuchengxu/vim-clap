const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let currentMode = 'files';
let results = [];
let selectedIndex = 0;

function setMode(mode) {
  currentMode = mode;
  document.querySelectorAll('.mode-tab').forEach(tab => {
    tab.classList.toggle('active', tab.dataset.mode === mode);
  });
  const input = document.getElementById('search-input');
  input.placeholder = mode === 'files' ? 'Search files...' : 'Search content...';
  if (input.value) {
    startSearch(input.value);
  } else {
    clearResults();
  }
}

function clearResults() {
  results = [];
  selectedIndex = 0;
  document.getElementById('results-pane').innerHTML = '';
  document.getElementById('preview-pane').innerHTML = '<div class="preview-empty">No file selected</div>';
  updateStatusCount(0, 0, true);
}

function updateStatusCount(matched, processed, finished) {
  const status = document.getElementById('status-count');
  if (finished) {
    status.textContent = `${matched} result${matched !== 1 ? 's' : ''}`;
  } else {
    status.textContent = `${matched} results (searching... ${processed} files)`;
  }
}

// Listen for progressive search results from backend
listen('search-results', (event) => {
  const payload = event.payload;
  const isFiles = payload.type === 'files';

  results = payload.results;
  if (selectedIndex >= results.length) {
    selectedIndex = Math.max(0, results.length - 1);
  }

  renderResults(isFiles);
  updateStatusCount(payload.total_matched, payload.total_processed, payload.finished);

  // Load preview for the first result when search completes or on first batch
  if (results.length > 0 && (payload.finished || selectedIndex === 0)) {
    loadPreview(results[selectedIndex]);
  }
});

document.querySelectorAll('.mode-tab').forEach(tab => {
  tab.addEventListener('click', () => setMode(tab.dataset.mode));
});

// Show cwd in titlebar on load
(async () => {
  try {
    const cwd = await invoke('get_cwd');
    // Replace home dir with ~ for brevity
    const home = cwd.match(/^(\/Users\/[^/]+|\/home\/[^/]+)/);
    const display = home ? cwd.replace(home[0], '~') : cwd;
    document.getElementById('titlebar-title').textContent = display;
    document.getElementById('titlebar-title').title = cwd;
  } catch (e) {
    console.error('Failed to get cwd:', e);
  }
})();
