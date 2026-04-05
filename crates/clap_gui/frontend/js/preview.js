// Map file extensions to Prism.js language identifiers
const LANG_MAP = {
  rs: 'rust', py: 'python', js: 'javascript', ts: 'typescript',
  jsx: 'javascript', tsx: 'typescript',
  go: 'go', c: 'c', cpp: 'cpp', cc: 'cpp', h: 'c', hpp: 'cpp',
  java: 'java', sh: 'bash', bash: 'bash', zsh: 'bash',
  toml: 'toml', json: 'json', yaml: 'yaml', yml: 'yaml',
  css: 'css', html: 'markup', xml: 'markup', svg: 'markup',
  md: 'markdown',
};

function highlightLine(lineText, lang) {
  if (window.Prism && lang && Prism.languages[lang]) {
    return Prism.highlight(lineText, Prism.languages[lang], lang);
  }
  return escapeHtml(lineText);
}

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

    const ext = preview.language.toLowerCase();
    const lang = LANG_MAP[ext] || ext;
    const headerPath = path.split('/').slice(-3).join('/');

    let html = `<div class="preview-header">Preview — ${escapeHtml(headerPath)}</div>`;

    preview.lines.forEach((lineText, i) => {
      const lineNum = preview.start_line + i;
      const isHighlighted = line && lineNum === Number(line);
      const cls = isHighlighted ? 'preview-line highlighted' : 'preview-line';
      const highlighted = highlightLine(lineText, lang);

      html += `<div class="${cls}"><span class="preview-line-number">${lineNum}</span>${highlighted}</div>`;
    });

    pane.innerHTML = html;

    const highlightedEl = pane.querySelector('.highlighted');
    if (highlightedEl) highlightedEl.scrollIntoView({ block: 'center' });
  } catch (e) {
    pane.innerHTML = `<div class="preview-empty">Cannot preview: ${escapeHtml(String(e))}</div>`;
  }
}
