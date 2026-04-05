async function loadPreview(result) {
  const pane = document.getElementById('preview-pane');
  const path = result.path;
  const line = result.line_number || null;

  try {
    const preview = await invoke('preview_file', {
      path,
      line: line ? Number(line) : null,
      maxLines: 50,
    });

    const headerPath = path.split('/').slice(-3).join('/');

    let html = `<div class="preview-header">Preview — ${escapeHtml(headerPath)}</div>`;

    preview.lines.forEach((lineText, i) => {
      const lineNum = preview.start_line + i;
      const isHighlighted = line && lineNum === Number(line);
      const cls = isHighlighted ? 'preview-line highlighted' : 'preview-line';

      html += `<div class="${cls}"><span class="preview-line-number">${lineNum}</span>${escapeHtml(lineText)}</div>`;
    });

    pane.innerHTML = html;

    const highlightedEl = pane.querySelector('.highlighted');
    if (highlightedEl) highlightedEl.scrollIntoView({ block: 'center' });
  } catch (e) {
    pane.innerHTML = `<div class="preview-empty">Cannot preview: ${escapeHtml(String(e))}</div>`;
  }
}
