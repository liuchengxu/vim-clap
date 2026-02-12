// Terminal Panel - Embedded PTY terminal using xterm.js
// Provides an in-app terminal for running commands without leaving the preview

let terminalPanelOpen = false;
let terminalMinimized = false;
let terminalSavedHeight = 300;
let termInstance = null;
let termFitAddon = null;
let termResizeObserver = null;

/**
 * Check if the terminal panel is currently open.
 */
function isTerminalPanelOpen() {
    return terminalPanelOpen;
}

/**
 * Check if the terminal panel is currently focused.
 */
function isTerminalPanelFocused() {
    if (!terminalPanelOpen) return false;
    const panel = document.getElementById('terminal-panel');
    return panel && panel.contains(document.activeElement);
}

/**
 * Open the terminal panel.
 */
async function openTerminalPanel() {
    if (terminalPanelOpen) return;
    terminalPanelOpen = true;

    const { invoke } = window.__TAURI__.core;
    const { Channel } = window.__TAURI__.core;

    // Create panel DOM
    const panel = document.createElement('div');
    panel.id = 'terminal-panel';
    panel.innerHTML = `
        <div class="terminal-resize-handle"></div>
        <div class="terminal-header">
            <span class="terminal-title">Terminal</span>
            <div class="terminal-header-buttons">
                <button class="terminal-minimize-btn" title="Minimize/Maximize">
                    <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor"><path d="M3 8a.75.75 0 0 1 .75-.75h8.5a.75.75 0 0 1 0 1.5h-8.5A.75.75 0 0 1 3 8z"/></svg>
                </button>
                <button class="terminal-close-btn" title="Close (Ctrl+\` or Esc)">&times;</button>
            </div>
        </div>
        <div class="terminal-body"></div>
    `;
    document.body.appendChild(panel);

    // Initialize xterm.js
    const term = new Terminal({
        cursorBlink: true,
        fontSize: 13,
        fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', Menlo, Monaco, monospace",
        theme: {
            background: '#0d1117',
            foreground: '#f0f6fc',
            cursor: '#58a6ff',
            selectionBackground: '#264f78',
            black: '#484f58',
            red: '#ff7b72',
            green: '#3fb950',
            yellow: '#d29922',
            blue: '#58a6ff',
            magenta: '#bc8cff',
            cyan: '#39d353',
            white: '#f0f6fc',
        },
        allowProposedApi: true,
    });

    const fitAddon = new FitAddon.FitAddon();
    term.loadAddon(fitAddon);

    // Try WebGL addon for better performance
    try {
        const webglAddon = new WebglAddon.WebglAddon();
        term.loadAddon(webglAddon);
    } catch (e) {
        console.warn('WebGL addon not available, using canvas renderer:', e);
    }

    const body = panel.querySelector('.terminal-body');
    term.open(body);
    fitAddon.fit();

    termInstance = term;
    termFitAddon = fitAddon;

    // Create Tauri channel for streaming events
    const channel = new Channel();
    channel.onmessage = (message) => {
        if (message.event === 'Output') {
            term.write(new Uint8Array(message.data));
        } else if (message.event === 'Exit') {
            const code = message.data?.code ?? '?';
            term.writeln(`\r\n[Process exited with code ${code}]`);
        }
    };

    // Spawn terminal backend
    try {
        await invoke('spawn_terminal', {
            rows: term.rows,
            cols: term.cols,
            onEvent: channel,
        });
    } catch (err) {
        term.writeln(`\r\nFailed to spawn terminal: ${err}`);
    }

    // Wire input: keystrokes -> backend
    term.onData((data) => {
        invoke('write_terminal', { data }).catch(err => {
            console.error('Failed to write to terminal:', err);
        });
    });

    // Wire resize: xterm resize -> backend
    term.onResize(({ cols, rows }) => {
        invoke('resize_terminal', { rows, cols }).catch(err => {
            console.error('Failed to resize terminal:', err);
        });
    });

    // Auto-fit on body resize
    termResizeObserver = new ResizeObserver(() => {
        fitAddon.fit();
    });
    termResizeObserver.observe(body);

    // Update main content padding
    const mainContent = document.getElementById('main-content');
    if (mainContent) {
        mainContent.style.paddingBottom = panel.offsetHeight + 'px';
    }

    // Show with transition
    requestAnimationFrame(() => {
        panel.classList.add('visible');
    });

    // Close button
    panel.querySelector('.terminal-close-btn').addEventListener('click', () => {
        closeTerminalPanel();
    });

    // Minimize/maximize toggle
    panel.querySelector('.terminal-minimize-btn').addEventListener('click', () => {
        toggleTerminalMinimize();
    });

    // Double-click header to toggle minimize
    panel.querySelector('.terminal-header').addEventListener('dblclick', (e) => {
        if (e.target.closest('button')) return;
        toggleTerminalMinimize();
    });

    // Resize handle (drag to resize height)
    const handle = panel.querySelector('.terminal-resize-handle');
    handle.addEventListener('mousedown', (startEvent) => {
        startEvent.preventDefault();
        const startY = startEvent.clientY;
        const startHeight = panel.offsetHeight;

        function onMouseMove(e) {
            const delta = startY - e.clientY;
            const newHeight = Math.min(600, Math.max(150, startHeight + delta));
            panel.style.height = newHeight + 'px';
            if (mainContent) {
                mainContent.style.paddingBottom = newHeight + 'px';
            }
            fitAddon.fit();
        }

        function onMouseUp() {
            document.removeEventListener('mousemove', onMouseMove);
            document.removeEventListener('mouseup', onMouseUp);
        }

        document.addEventListener('mousemove', onMouseMove);
        document.addEventListener('mouseup', onMouseUp);
    });

    // Focus terminal
    term.focus();
}

/**
 * Toggle the terminal between minimized (header-only) and expanded states.
 */
function toggleTerminalMinimize() {
    const panel = document.getElementById('terminal-panel');
    if (!panel) return;

    const mainContent = document.getElementById('main-content');
    const btn = panel.querySelector('.terminal-minimize-btn svg');

    if (terminalMinimized) {
        // Restore
        panel.classList.remove('minimized');
        panel.style.height = terminalSavedHeight + 'px';
        if (mainContent) mainContent.style.paddingBottom = terminalSavedHeight + 'px';
        // Switch icon back to minimize (horizontal line)
        btn.innerHTML = '<path d="M3 8a.75.75 0 0 1 .75-.75h8.5a.75.75 0 0 1 0 1.5h-8.5A.75.75 0 0 1 3 8z"/>';
        terminalMinimized = false;
        if (termFitAddon) termFitAddon.fit();
        if (termInstance) termInstance.focus();
    } else {
        // Minimize — save current height, collapse to header only
        terminalSavedHeight = panel.offsetHeight;
        panel.classList.add('minimized');
        panel.style.height = '';
        if (mainContent) mainContent.style.paddingBottom = panel.offsetHeight + 'px';
        // Switch icon to maximize (expand square)
        btn.innerHTML = '<path d="M3.75 2A1.75 1.75 0 0 0 2 3.75v8.5c0 .966.784 1.75 1.75 1.75h8.5A1.75 1.75 0 0 0 14 12.25v-8.5A1.75 1.75 0 0 0 12.25 2h-8.5zm0 1.5h8.5a.25.25 0 0 1 .25.25v8.5a.25.25 0 0 1-.25.25h-8.5a.25.25 0 0 1-.25-.25v-8.5a.25.25 0 0 1 .25-.25z"/>';
        terminalMinimized = true;
    }
}

/**
 * Close the terminal panel and kill the session.
 */
async function closeTerminalPanel() {
    if (!terminalPanelOpen) return;
    terminalPanelOpen = false;
    terminalMinimized = false;

    const { invoke } = window.__TAURI__.core;

    // Kill backend process
    try {
        await invoke('kill_terminal');
    } catch (err) {
        console.error('Failed to kill terminal:', err);
    }

    // Clean up resize observer
    if (termResizeObserver) {
        termResizeObserver.disconnect();
        termResizeObserver = null;
    }

    // Dispose xterm
    if (termInstance) {
        termInstance.dispose();
        termInstance = null;
        termFitAddon = null;
    }

    // Remove panel with transition
    const panel = document.getElementById('terminal-panel');
    if (panel) {
        panel.classList.remove('visible');
        setTimeout(() => {
            panel.remove();
        }, 200);
    }

    // Restore main content padding
    const mainContent = document.getElementById('main-content');
    if (mainContent) {
        mainContent.style.paddingBottom = '';
    }
}

/**
 * Toggle the terminal panel open/closed.
 */
function toggleTerminalPanel() {
    if (terminalPanelOpen) {
        closeTerminalPanel();
    } else {
        openTerminalPanel();
    }
}

// Export terminal functions
window.toggleTerminalPanel = toggleTerminalPanel;
window.isTerminalPanelOpen = isTerminalPanelOpen;
window.isTerminalPanelFocused = isTerminalPanelFocused;
window.closeTerminalPanel = closeTerminalPanel;
