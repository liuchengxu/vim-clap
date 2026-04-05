const { invoke } = window.__TAURI__.core;

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
    runSearch(input.value);
  } else {
    clearResults();
  }
}

function clearResults() {
  results = [];
  selectedIndex = 0;
  document.getElementById('results-pane').innerHTML = '';
  document.getElementById('preview-pane').innerHTML = '<div class="preview-empty">No file selected</div>';
  document.getElementById('status-count').textContent = '0 results';
}

document.querySelectorAll('.mode-tab').forEach(tab => {
  tab.addEventListener('click', () => setMode(tab.dataset.mode));
});
