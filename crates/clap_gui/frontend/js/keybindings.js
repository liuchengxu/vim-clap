document.addEventListener('keydown', (e) => {
  const input = document.getElementById('search-input');
  const isMeta = e.metaKey || e.ctrlKey;

  if (e.key === 'Tab') {
    e.preventDefault();
    setMode(currentMode === 'files' ? 'grep' : 'files');
    return;
  }

  if (e.key === 'Escape') {
    if (input.value) {
      input.value = '';
      clearResults();
    } else {
      // Input already empty — close the window
      window.__TAURI__.window.getCurrentWindow().close();
    }
    return;
  }

  if (e.key === 'ArrowUp' || (e.ctrlKey && e.key === 'k')) {
    e.preventDefault();
    if (results.length > 0) {
      selectedIndex = Math.max(0, selectedIndex - 1);
      renderResults(currentMode === 'files');
      loadPreview(results[selectedIndex]);
    }
    return;
  }

  if (e.key === 'ArrowDown' || (e.ctrlKey && e.key === 'j')) {
    e.preventDefault();
    if (results.length > 0) {
      selectedIndex = Math.min(results.length - 1, selectedIndex + 1);
      renderResults(currentMode === 'files');
      loadPreview(results[selectedIndex]);
    }
    return;
  }

  if (e.key === 'Enter' && !isMeta && results.length > 0) {
    e.preventDefault();
    openResult(results[selectedIndex]);
    return;
  }

  if (e.key === 'Enter' && isMeta && results.length > 0) {
    e.preventDefault();
    copyResult(results[selectedIndex]);
    return;
  }

  if (e.key === 'p' && isMeta) {
    e.preventDefault();
    togglePin();
    return;
  }

  if (document.activeElement !== input) {
    input.focus();
  }
});

async function openResult(result) {
  try {
    await invoke('open_in_editor', {
      path: result.path,
      line: result.line_number ? Number(result.line_number) : null,
    });
  } catch (e) {
    console.error('Failed to open:', e);
  }
}

async function copyResult(result) {
  try {
    await navigator.clipboard.writeText(result.path);
    const status = document.getElementById('status-count');
    const prev = status.textContent;
    status.textContent = 'Copied to clipboard!';
    setTimeout(() => { status.textContent = prev; }, 1000);
  } catch (e) {
    console.error('Failed to copy:', e);
  }
}

let isPinned = false;

async function togglePin() {
  isPinned = !isPinned;
  const btn = document.getElementById('pin-btn');
  btn.classList.toggle('active', isPinned);

  try {
    const currentWindow = window.__TAURI__.window.getCurrentWindow();
    await currentWindow.setDecorations(isPinned);
    await currentWindow.setResizable(isPinned);
  } catch (e) {
    console.error('Pin toggle error:', e);
  }
}
