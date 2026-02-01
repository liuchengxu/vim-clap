/**
 * PDF.js based PDF Viewer
 *
 * Features:
 * - Lazy page rendering with IntersectionObserver
 * - Canvas pooling for memory efficiency
 * - Text layer for text selection
 * - Annotation layer for clickable links
 * - TOC extraction and navigation
 * - DPR-aware rendering for crisp text on retina displays
 */
(function() {
'use strict';

// PDF.js library reference (set during init, loaded via ES module)
let pdfjsLib = null;

// Tauri APIs will be accessed when needed
let convertFileSrc = null;
let resolveResource = null;

/**
 * PDF Viewer class - manages PDF document rendering and interaction
 */
class PdfViewerClass {
    constructor() {
        this.pdf = null;              // PDFDocumentProxy
        this.pages = new Map();       // pageNum -> { container, canvas, rendered }
        this.canvasPool = [];         // Reusable canvas elements
        this.scale = 1.0;
        this.baseScale = 1.0;         // Calculated to fit width
        this.currentPage = 1;
        this.onStatsUpdate = null;    // Callback for stats update
        this.initialized = false;
        this.observer = null;         // IntersectionObserver
        this.container = null;        // PDF pages container
        this.currentPath = null;      // Current PDF file path
        this.pendingRenders = new Set(); // Pages currently being rendered
        this.zoomLevels = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];
        this.zoomIndex = 2;           // Default to 1.0
        this.outline = null;          // PDF outline for TOC
        this.doubleClickZoomActive = false;  // Track if zoomed via double-click
        this.preDoubleClickZoomIndex = 2;    // Zoom level before double-click
    }

    /**
     * Initialize PDF.js worker and Tauri APIs
     */
    async init() {
        if (this.initialized) {
            return;
        }


        // Wait for PDF.js to be loaded (ES module loads asynchronously)
        pdfjsLib = await this.waitForPdfjs();
        if (!pdfjsLib) {
            throw new Error('PDF.js library not available');
        }

        // Get Tauri APIs
        if (window.__TAURI__) {
            convertFileSrc = window.__TAURI__.core.convertFileSrc;
            resolveResource = window.__TAURI__.path.resolveResource;
        }

        // Set worker path relative to the HTML file (frontend/vendor/)
        // This avoids CORS issues with resolveResource which points to wrong location
        pdfjsLib.GlobalWorkerOptions.workerSrc = './vendor/pdf.worker.min.js';

        this.initialized = true;
    }

    /**
     * Wait for PDF.js library to be available (loaded via ES module)
     */
    async waitForPdfjs(maxWaitMs = 5000) {
        const startTime = Date.now();
        while (!window.pdfjsLib && (Date.now() - startTime) < maxWaitMs) {
            await new Promise(resolve => setTimeout(resolve, 50));
        }
        if (window.pdfjsLib) {
        } else {
            console.error('[PdfViewer] pdfjsLib not loaded after', maxWaitMs, 'ms');
        }
        return window.pdfjsLib || null;
    }

    /**
     * Open a PDF file
     * @param {string} filePath - File path to the PDF (not asset URL)
     */
    async open(filePath) {
        await this.init();

        // Clean up previous PDF
        this.cleanup();
        this.currentPath = filePath;

        // Set up the container
        this.setupContainer();

        // Read the PDF file using Tauri's fs plugin and create a blob URL
        let pdfData;
        try {
            const { readFile } = window.__TAURI__.fs;
            const bytes = await readFile(filePath);
            pdfData = bytes;
        } catch (error) {
            console.error('[PdfViewer] Failed to read file:', error);
            throw new Error(`Failed to read PDF file: ${error.message || error}`);
        }

        // Build loading options with the file data
        const loadingOptions = { data: pdfData };

        // Configure cmap and font paths for Tauri (optional, improves CJK support)
        if (resolveResource && convertFileSrc) {
            try {
                const cmapPath = await resolveResource('vendor/cmaps');
                const fontPath = await resolveResource('vendor/standard_fonts');
                loadingOptions.cMapUrl = convertFileSrc(cmapPath) + '/';
                loadingOptions.cMapPacked = true;
                loadingOptions.standardFontDataUrl = convertFileSrc(fontPath) + '/';
            } catch (error) {
                console.warn('[PdfViewer] Failed to resolve cmap/font paths:', error);
                // Continue without custom fonts - system fonts will be used
            }
        }

        // Load the PDF
        try {
            const loadingTask = pdfjsLib.getDocument(loadingOptions);
            this.pdf = await loadingTask.promise;
        } catch (error) {
            console.error('[PdfViewer] Failed to load PDF:', error);
            throw error;
        }

        // Notify stats update
        this.notifyStatsUpdate();

        // Extract and render TOC
        this.outline = await this.pdf.getOutline();
        this.renderTOC(this.outline);

        // Create page placeholders
        await this.createPagePlaceholders();

        // Set up lazy loading observer
        this.setupIntersectionObserver();

        // Scroll to top
        this.container.scrollTop = 0;
    }

    /**
     * Set up the container for PDF pages
     */
    setupContainer() {
        const contentEl = document.getElementById('content');
        if (!contentEl) return;

        // Clear existing content
        contentEl.innerHTML = '';

        // Create PDF container
        this.container = document.createElement('div');
        this.container.id = 'pdf-container';
        this.container.className = 'pdf-container';
        contentEl.appendChild(this.container);

        // Add double-click to zoom handler
        this.container.addEventListener('dblclick', (e) => {
            this.handleDoubleClick(e);
        });

        // Create TOC container if not exists
        if (!document.getElementById('pdf-toc')) {
            const tocContent = document.getElementById('toc-content');
            if (tocContent) {
                tocContent.innerHTML = '<div id="pdf-toc" class="pdf-toc"></div>';
            }
        }
    }

    /**
     * Create placeholder elements for all pages
     * Uses first page dimensions as default to avoid loading every page upfront
     */
    async createPagePlaceholders() {
        if (!this.pdf || !this.container) return;

        // Get first page to calculate default dimensions
        const firstPage = await this.pdf.getPage(1);
        const firstViewport = firstPage.getViewport({ scale: 1.0 });

        // Calculate base scale to fit container width (with padding)
        const containerWidth = this.container.clientWidth - 40; // 20px padding each side
        this.baseScale = containerWidth / firstViewport.width;

        // Use first page dimensions as default for all placeholders
        const defaultWidth = firstViewport.width * this.baseScale * this.scale;
        const defaultHeight = firstViewport.height * this.baseScale * this.scale;

        // Create placeholders for all pages using default dimensions
        // Actual dimensions will be set when page is rendered
        for (let pageNum = 1; pageNum <= this.pdf.numPages; pageNum++) {
            const pageContainer = document.createElement('div');
            pageContainer.className = 'pdf-page';
            pageContainer.dataset.pageNum = pageNum;
            pageContainer.style.width = `${defaultWidth}px`;
            pageContainer.style.height = `${defaultHeight}px`;

            // Add page number label
            const pageLabel = document.createElement('div');
            pageLabel.className = 'pdf-page-label';
            pageLabel.textContent = `Page ${pageNum}`;
            pageContainer.appendChild(pageLabel);

            this.container.appendChild(pageContainer);
            this.pages.set(pageNum, {
                container: pageContainer,
                canvas: null,
                textLayer: null,
                annotationLayer: null,
                rendered: false
            });
        }
    }

    /**
     * Set up IntersectionObserver for lazy loading
     */
    setupIntersectionObserver() {
        if (this.observer) {
            this.observer.disconnect();
        }

        const options = {
            root: this.container,
            rootMargin: '200px 0px', // Load pages 200px before they're visible
            threshold: 0
        };

        this.observer = new IntersectionObserver((entries) => {
            entries.forEach(entry => {
                const pageNum = parseInt(entry.target.dataset.pageNum, 10);
                if (entry.isIntersecting) {
                    this.renderPage(pageNum);
                } else {
                    // Optional: unload pages that are far from view to save memory
                    this.maybeUnloadPage(pageNum);
                }
            });
        }, options);

        // Observe all page containers
        this.pages.forEach((pageData, pageNum) => {
            this.observer.observe(pageData.container);
        });
    }

    /**
     * Render a single page
     * @param {number} pageNum - Page number to render
     */
    async renderPage(pageNum) {
        const pageData = this.pages.get(pageNum);
        if (!pageData || pageData.rendered || this.pendingRenders.has(pageNum)) {
            return;
        }

        this.pendingRenders.add(pageNum);

        try {
            const page = await this.pdf.getPage(pageNum);
            const dpr = window.devicePixelRatio || 1;
            const effectiveScale = this.baseScale * this.scale;
            const viewport = page.getViewport({ scale: effectiveScale * dpr });
            const displayViewport = page.getViewport({ scale: effectiveScale });

            // Get or create canvas
            const canvas = this.getCanvas();
            canvas.width = viewport.width;
            canvas.height = viewport.height;
            canvas.style.width = `${displayViewport.width}px`;
            canvas.style.height = `${displayViewport.height}px`;
            canvas.className = 'pdf-canvas';

            // Clear page label and add canvas
            pageData.container.innerHTML = '';
            pageData.container.appendChild(canvas);

            // Update container size
            pageData.container.style.width = `${displayViewport.width}px`;
            pageData.container.style.height = `${displayViewport.height}px`;

            // Render page to canvas
            const context = canvas.getContext('2d');
            await page.render({
                canvasContext: context,
                viewport: viewport
            }).promise;

            // Render text layer for selection
            await this.renderTextLayer(page, displayViewport, pageData.container);

            // Render annotation layer for links
            await this.renderAnnotationLayer(page, displayViewport, pageData.container);

            pageData.canvas = canvas;
            pageData.rendered = true;
        } catch (error) {
            console.error(`Failed to render page ${pageNum}:`, error);
        } finally {
            this.pendingRenders.delete(pageNum);
        }
    }

    /**
     * Render text layer for text selection
     */
    async renderTextLayer(page, viewport, container) {
        const textContent = await page.getTextContent();

        const textLayer = document.createElement('div');
        textLayer.className = 'textLayer';
        textLayer.style.width = `${viewport.width}px`;
        textLayer.style.height = `${viewport.height}px`;

        container.appendChild(textLayer);

        // Use PDF.js TextLayer if available
        if (pdfjsLib.TextLayer) {
            const textLayerBuilder = new pdfjsLib.TextLayer({
                textContentSource: textContent,
                container: textLayer,
                viewport: viewport
            });
            await textLayerBuilder.render();
        } else {
            // Fallback: manual text span creation
            for (const item of textContent.items) {
                if (!item.str) continue;

                const span = document.createElement('span');
                span.textContent = item.str;

                const tx = pdfjsLib.Util.transform(
                    viewport.transform,
                    item.transform
                );

                const fontSize = Math.sqrt(tx[0] * tx[0] + tx[1] * tx[1]);
                const angle = Math.atan2(tx[1], tx[0]) * (180 / Math.PI);

                span.style.left = `${tx[4]}px`;
                span.style.top = `${tx[5] - fontSize}px`;
                span.style.fontSize = `${fontSize}px`;
                span.style.fontFamily = item.fontName || 'sans-serif';

                if (Math.abs(angle) > 0.1) {
                    span.style.transform = `rotate(${angle}deg)`;
                }

                textLayer.appendChild(span);
            }
        }

        return textLayer;
    }

    /**
     * Render annotation layer for clickable links
     */
    async renderAnnotationLayer(page, viewport, container) {
        const annotations = await page.getAnnotations();
        if (!annotations || annotations.length === 0) return;

        const annotationLayer = document.createElement('div');
        annotationLayer.className = 'annotationLayer';
        annotationLayer.style.width = `${viewport.width}px`;
        annotationLayer.style.height = `${viewport.height}px`;

        for (const annotation of annotations) {
            if (annotation.subtype !== 'Link') continue;
            if (!annotation.rect) continue;

            // Convert PDF coordinates to viewport coordinates
            const rect = pdfjsLib.Util.normalizeRect(
                viewport.convertToViewportRectangle(annotation.rect)
            );

            const link = document.createElement('a');
            link.className = 'pdf-link-annotation';
            link.style.left = `${rect[0]}px`;
            link.style.top = `${rect[1]}px`;
            link.style.width = `${rect[2] - rect[0]}px`;
            link.style.height = `${rect[3] - rect[1]}px`;

            if (annotation.url) {
                // External link
                link.href = annotation.url;
                link.target = '_blank';
                link.rel = 'noopener noreferrer';
            } else if (annotation.dest) {
                // Internal link (to another page/location)
                link.href = '#';
                link.dataset.dest = typeof annotation.dest === 'string'
                    ? annotation.dest
                    : JSON.stringify(annotation.dest);
                link.addEventListener('click', (e) => {
                    e.preventDefault();
                    this.navigateToDestination(annotation.dest);
                });
            } else if (annotation.action && annotation.action.dest) {
                // Action-based internal link
                link.href = '#';
                link.addEventListener('click', (e) => {
                    e.preventDefault();
                    this.navigateToDestination(annotation.action.dest);
                });
            }

            annotationLayer.appendChild(link);
        }

        container.appendChild(annotationLayer);
        return annotationLayer;
    }

    /**
     * Navigate to a PDF destination (internal link)
     */
    async navigateToDestination(dest) {
        if (!this.pdf) return;

        let pageNum;

        if (typeof dest === 'string') {
            // Named destination
            const destination = await this.pdf.getDestination(dest);
            if (destination) {
                const pageIndex = await this.pdf.getPageIndex(destination[0]);
                pageNum = pageIndex + 1;
            }
        } else if (Array.isArray(dest)) {
            // Explicit destination array
            const pageIndex = await this.pdf.getPageIndex(dest[0]);
            pageNum = pageIndex + 1;
        }

        if (pageNum && pageNum >= 1 && pageNum <= this.pdf.numPages) {
            this.scrollToPage(pageNum);
        }
    }

    /**
     * Scroll to a specific page
     */
    scrollToPage(pageNum) {
        const pageData = this.pages.get(pageNum);
        if (pageData && pageData.container) {
            pageData.container.scrollIntoView({ behavior: 'smooth', block: 'start' });
            this.currentPage = pageNum;
        }
    }

    /**
     * Render the Table of Contents from PDF outline
     */
    renderTOC(outline) {
        const tocContainer = document.getElementById('pdf-toc');
        if (!tocContainer) return;

        if (!outline || outline.length === 0) {
            tocContainer.innerHTML = '<div class="toc-empty">No table of contents</div>';
            return;
        }

        const html = this.buildTOCTree(outline);
        tocContainer.innerHTML = html;

        // Add click handlers
        tocContainer.querySelectorAll('a').forEach(link => {
            link.addEventListener('click', (e) => {
                e.preventDefault();
                const dest = link.dataset.dest;
                if (dest) {
                    try {
                        const parsed = JSON.parse(dest);
                        this.navigateToDestination(parsed);
                    } catch {
                        this.navigateToDestination(dest);
                    }
                }
            });
        });
    }

    /**
     * Refresh/re-render the TOC (called when TOC panel is toggled)
     */
    refreshTOC() {
        // Ensure the pdf-toc container exists
        const tocContent = document.getElementById('toc-content');
        if (tocContent && !document.getElementById('pdf-toc')) {
            tocContent.innerHTML = '<div id="pdf-toc" class="pdf-toc"></div>';
        }
        this.renderTOC(this.outline);
    }

    /**
     * Build TOC HTML tree recursively
     */
    buildTOCTree(items, level = 0) {
        if (!items || items.length === 0) return '';

        let html = '<ul class="pdf-toc-list">';
        for (const item of items) {
            const destData = item.dest
                ? (typeof item.dest === 'string' ? item.dest : JSON.stringify(item.dest))
                : '';
            const escapedTitle = this.escapeHtml(item.title || 'Untitled');

            html += `<li class="pdf-toc-item level-${level}">`;
            html += `<a href="#" data-dest='${this.escapeHtml(destData)}'>${escapedTitle}</a>`;

            if (item.items && item.items.length > 0) {
                html += this.buildTOCTree(item.items, level + 1);
            }

            html += '</li>';
        }
        html += '</ul>';
        return html;
    }

    /**
     * Escape HTML special characters
     */
    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    /**
     * Get a canvas from the pool or create a new one
     */
    getCanvas() {
        if (this.canvasPool.length > 0) {
            const canvas = this.canvasPool.pop();
            canvas.getContext('2d').clearRect(0, 0, canvas.width, canvas.height);
            return canvas;
        }
        return document.createElement('canvas');
    }

    /**
     * Return a canvas to the pool
     */
    returnCanvas(canvas) {
        if (this.canvasPool.length < 10) { // Keep max 10 canvases in pool
            this.canvasPool.push(canvas);
        }
    }

    /**
     * Unload a page to save memory if it's far from current view
     */
    maybeUnloadPage(pageNum) {
        // Keep rendered pages near current view
        const buffer = 3;
        if (Math.abs(pageNum - this.currentPage) <= buffer) {
            return;
        }

        const pageData = this.pages.get(pageNum);
        if (!pageData || !pageData.rendered) return;

        // Return canvas to pool
        if (pageData.canvas) {
            this.returnCanvas(pageData.canvas);
        }

        // Reset page to placeholder state
        pageData.container.innerHTML = '';
        const pageLabel = document.createElement('div');
        pageLabel.className = 'pdf-page-label';
        pageLabel.textContent = `Page ${pageNum}`;
        pageData.container.appendChild(pageLabel);

        pageData.canvas = null;
        pageData.textLayer = null;
        pageData.annotationLayer = null;
        pageData.rendered = false;
    }

    /**
     * Notify stats update callback
     */
    notifyStatsUpdate() {
        if (!this.pdf) return;

        const stats = {
            pages: this.pdf.numPages,
            reading_minutes: Math.ceil(this.pdf.numPages * 0.5) // ~30 sec per page
        };

        if (this.onStatsUpdate) {
            this.onStatsUpdate(stats);
        }
    }

    /**
     * Zoom in
     */
    zoomIn() {
        if (this.zoomIndex < this.zoomLevels.length - 1) {
            this.zoomIndex++;
            this.setZoom(this.zoomLevels[this.zoomIndex]);
        }
    }

    /**
     * Zoom out
     */
    zoomOut() {
        if (this.zoomIndex > 0) {
            this.zoomIndex--;
            this.setZoom(this.zoomLevels[this.zoomIndex]);
        }
    }

    /**
     * Reset zoom to 100%
     */
    resetZoom() {
        this.zoomIndex = 2; // 1.0 index
        this.doubleClickZoomActive = false;
        this.setZoom(1.0);
    }

    /**
     * Recalculate base scale when container width changes (e.g., content width setting)
     */
    async recalculateBaseScale() {
        if (!this.pdf || !this.container) return;

        // Get first page to calculate new base scale
        const firstPage = await this.pdf.getPage(1);
        const firstViewport = firstPage.getViewport({ scale: 1.0 });

        // Recalculate base scale based on current container width
        const containerWidth = this.container.clientWidth - 40;
        this.baseScale = containerWidth / firstViewport.width;

        // Re-apply current zoom to trigger re-render with new base scale
        await this.setZoom(this.scale);
    }

    /**
     * Handle double-click to zoom
     * - If at normal zoom: zoom to 2x centered on click point
     * - If already zoomed via double-click: zoom back to previous level
     */
    async handleDoubleClick(e) {
        if (!this.container || !this.pdf) return;

        // Get click position relative to the scrollable container
        const containerRect = this.container.getBoundingClientRect();
        const mainContent = document.getElementById('main-content');
        if (!mainContent) return;

        // Calculate click position as ratio of visible area
        const clickX = e.clientX - containerRect.left;
        const clickY = e.clientY - containerRect.top;

        // Get current scroll position and container dimensions
        const scrollLeft = mainContent.scrollLeft || 0;
        const scrollTop = mainContent.scrollTop || 0;
        const viewWidth = mainContent.clientWidth;
        const viewHeight = mainContent.clientHeight;

        // Calculate the absolute position in the document
        const docX = scrollLeft + clickX;
        const docY = scrollTop + clickY;

        // Calculate position ratios (where in the document was clicked)
        const ratioX = docX / (this.container.scrollWidth || 1);
        const ratioY = docY / (this.container.scrollHeight || 1);

        if (this.doubleClickZoomActive) {
            // Zoom back to previous level
            this.doubleClickZoomActive = false;
            this.zoomIndex = this.preDoubleClickZoomIndex;
            await this.setZoom(this.zoomLevels[this.zoomIndex]);
        } else {
            // Zoom in to 2x (or next level if already above 1x)
            this.preDoubleClickZoomIndex = this.zoomIndex;
            this.doubleClickZoomActive = true;

            // Find the 2.0 zoom level index, or go 2 steps up from current
            const targetScale = Math.min(this.scale * 2, 4.0);
            const targetIndex = this.zoomLevels.findIndex(z => z >= targetScale);
            this.zoomIndex = targetIndex >= 0 ? targetIndex : this.zoomLevels.length - 1;

            await this.setZoom(this.zoomLevels[this.zoomIndex]);

            // After zoom, scroll to keep the clicked point centered
            // Wait a frame for the DOM to update
            requestAnimationFrame(() => {
                const newDocX = ratioX * this.container.scrollWidth;
                const newDocY = ratioY * this.container.scrollHeight;

                // Scroll so the clicked point is centered in the viewport
                mainContent.scrollLeft = newDocX - (viewWidth / 2);
                mainContent.scrollTop = newDocY - (viewHeight / 2);
            });
        }
    }

    /**
     * Set zoom level - re-renders pages at new scale for crisp quality
     */
    async setZoom(scale) {
        if (!this.container || !this.pdf) return;

        this.scale = scale;

        // Update zoom display if exists
        const zoomDisplay = document.getElementById('zoom-level-display');
        if (zoomDisplay) {
            zoomDisplay.textContent = `${Math.round(scale * 100)}%`;
        }

        // Save scroll position ratio
        const scrollRatio = this.container.scrollTop / (this.container.scrollHeight || 1);

        // Get first page to calculate default dimensions (avoid loading all pages)
        const firstPage = await this.pdf.getPage(1);
        const firstViewport = firstPage.getViewport({ scale: this.baseScale * scale });
        const defaultWidth = firstViewport.width;
        const defaultHeight = firstViewport.height;

        // Update each page's dimensions and mark for re-render
        for (const [pageNum, pageData] of this.pages) {
            // Use default dimensions (actual size set on render)
            pageData.container.style.width = `${defaultWidth}px`;
            pageData.container.style.height = `${defaultHeight}px`;

            // Clear CSS transform if any
            pageData.container.style.transform = '';
            pageData.container.style.marginBottom = '';

            // Mark as not rendered so it will be re-rendered
            if (pageData.rendered) {
                pageData.rendered = false;
                // Clear the canvas content but keep the container
                if (pageData.canvas) {
                    pageData.canvas.remove();
                    pageData.canvas = null;
                }
                // Clear text and annotation layers
                const textLayer = pageData.container.querySelector('.textLayer');
                if (textLayer) textLayer.remove();
                const annotLayer = pageData.container.querySelector('.annotationLayer');
                if (annotLayer) annotLayer.remove();

                // Add loading placeholder
                if (!pageData.container.querySelector('.pdf-page-label')) {
                    const label = document.createElement('div');
                    label.className = 'pdf-page-label';
                    label.textContent = `Page ${pageNum}`;
                    pageData.container.appendChild(label);
                }
            }
        }

        // Restore scroll position
        this.container.scrollTop = scrollRatio * this.container.scrollHeight;

        // Trigger re-render of visible pages by re-observing
        this.setupIntersectionObserver();
    }

    /**
     * Navigate to next page
     */
    nextPage() {
        if (this.currentPage < this.pdf.numPages) {
            this.scrollToPage(this.currentPage + 1);
        }
    }

    /**
     * Navigate to previous page
     */
    prevPage() {
        if (this.currentPage > 1) {
            this.scrollToPage(this.currentPage - 1);
        }
    }

    /**
     * Clean up resources
     */
    cleanup() {
        // Disconnect observer
        if (this.observer) {
            this.observer.disconnect();
            this.observer = null;
        }

        // Return all canvases to pool
        this.pages.forEach(pageData => {
            if (pageData.canvas) {
                this.returnCanvas(pageData.canvas);
            }
        });

        // Clear state
        this.pages.clear();
        this.pendingRenders.clear();

        if (this.pdf) {
            this.pdf.destroy();
            this.pdf = null;
        }

        // Clear container
        if (this.container) {
            this.container.innerHTML = '';
        }

        this.currentPath = null;
    }

    /**
     * Check if PDF viewer is currently active
     */
    isActive() {
        return this.pdf !== null;
    }

    /**
     * Get current page number
     */
    getCurrentPage() {
        return this.currentPage;
    }

    /**
     * Get total page count
     */
    getPageCount() {
        return this.pdf ? this.pdf.numPages : 0;
    }
}

// Create and expose the singleton instance
window.PdfViewer = new PdfViewerClass();

// Set up keyboard shortcuts when PDF viewer is active
document.addEventListener('keydown', (e) => {
    if (!window.PdfViewer.isActive()) return;

    // Don't intercept when in input fields
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;

    switch (e.key) {
        case '+':
        case '=':
            if (e.ctrlKey || e.metaKey) {
                e.preventDefault();
                window.PdfViewer.zoomIn();
            }
            break;
        case '-':
            if (e.ctrlKey || e.metaKey) {
                e.preventDefault();
                window.PdfViewer.zoomOut();
            }
            break;
        case '0':
            if (e.ctrlKey || e.metaKey) {
                e.preventDefault();
                window.PdfViewer.resetZoom();
            }
            break;
        case 'PageDown':
            window.PdfViewer.nextPage();
            break;
        case 'PageUp':
            window.PdfViewer.prevPage();
            break;
    }
});

})();
