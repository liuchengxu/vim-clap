// Markdown Preview - WebSocket Mode (vim-clap)
// This module handles WebSocket communication with the vim-clap server

// Note: core.js must be loaded before this file

(function() {
    'use strict';

    let websocket = null;

    // Switch to a different file via WebSocket
    function switchToFile(filePath) {
        if (filePath === currentFilePath) {
            return;
        }

        if (websocket && websocket.readyState === WebSocket.OPEN) {
            websocket.send(JSON.stringify({
                type: 'switch_file',
                file_path: filePath
            }));
            console.log(`Switching to: ${filePath}`);
        }
    }

    // Request manual refresh from server
    function requestRefresh() {
        if (currentFilePath && websocket && websocket.readyState === WebSocket.OPEN) {
            websocket.send(JSON.stringify({
                type: 'switch_file',
                file_path: currentFilePath
            }));
            showToast('Refreshing...');
        }
    }

    // Handle incoming WebSocket messages
    function handleMessage(message) {
        if (message.type === "update_content") {
            handleContentUpdate(message, {
                onFileClick: switchToFile
            });
        } else if (message.type === "scroll") {
            const scrollPercent = message.data;
            const windowHeight = window.innerHeight;
            const totalScrollHeight = document.documentElement.scrollHeight - windowHeight;
            const absoluteScrollPosition = totalScrollHeight * (scrollPercent / 100);
            window.scrollTo(0, absoluteScrollPosition);
        } else if (message.type === "focus_window") {
            window.focus();
        } else {
            console.log(`Invalid message: ${JSON.stringify(message)}`);
        }
    }

    // Initialize WebSocket connection
    function initWebSocket() {
        console.log('Running in WebSocket mode');
        const webSocketUrl = 'ws://' + window.location.host;
        websocket = new WebSocket(webSocketUrl);

        websocket.onmessage = function(event) {
            const message = JSON.parse(event.data);
            handleMessage(message);
        };

        websocket.onclose = function(event) {
            console.log(`WebSocket closed with code ${event.code}. Closing browser.`);
            window.open('', '_self', '');
            window.close();
        };

        websocket.onerror = function(error) {
            console.error('WebSocket error:', error);
        };
    }

    // Set up refresh keyboard shortcut
    function setupRefreshShortcut() {
        document.addEventListener('keydown', (e) => {
            if (e.key === 'F5' || ((e.ctrlKey || e.metaKey) && e.key === 'r')) {
                e.preventDefault();
                requestRefresh();
            }
        });
    }

    // Initialize on DOM ready
    document.addEventListener('DOMContentLoaded', function() {
        // Initialize core UI with file switch callback
        initCoreUI({
            onFileClick: switchToFile
        });

        // Set up WebSocket-specific features
        setupRefreshShortcut();

        // Connect to WebSocket server
        initWebSocket();
    });
})();
