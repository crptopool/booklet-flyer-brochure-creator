/**
 * Rasterises source PDF pages in the webview so previews can show the
 * user's real artwork rather than placeholders.
 *
 * This is preview-only. Export never goes through here — the imposition
 * renderer copies pages as vectors, so nothing that reaches a printer is
 * ever rasterised.
 */

import { invoke } from "@tauri-apps/api/core";
import * as pdfjs from "pdfjs-dist";
import type { PDFDocumentProxy } from "pdfjs-dist";
// Vite rewrites this to a bundled, same-origin worker URL, which keeps the
// strict content-security policy satisfied — nothing is fetched remotely.
import PdfWorker from "pdfjs-dist/build/pdf.worker.min.mjs?worker";

pdfjs.GlobalWorkerOptions.workerPort = new PdfWorker();

let doc: PDFDocumentProxy | null = null;
let loadedPath = "";
/** Rendered pages keyed by `page@width`, so re-navigating is instant. */
const cache = new Map<string, HTMLCanvasElement>();
const CACHE_LIMIT = 60;

/** True once a document is available to render from. */
export function isLoaded(): boolean {
  return doc !== null;
}

export function loadedPageCount(): number {
  return doc?.numPages ?? 0;
}

/** Load a PDF by path. Repeated calls for the same path are no-ops. */
export async function loadDocument(path: string): Promise<void> {
  if (loadedPath === path && doc) return;
  const bytes = await invoke<number[]>("read_pdf_bytes", { path });
  const data = new Uint8Array(bytes);
  const task = pdfjs.getDocument({ data, isEvalSupported: false });
  const next = await task.promise;
  doc?.destroy();
  doc = next;
  loadedPath = path;
  cache.clear();
}

export function unload(): void {
  doc?.destroy();
  doc = null;
  loadedPath = "";
  cache.clear();
}

/**
 * Render a 1-based page to a canvas whose width is `targetWidth` CSS
 * pixels. Returns null when no document is loaded or the page is absent,
 * so callers can fall back to a schematic placeholder.
 */
export async function renderPage(page: number, targetWidth: number): Promise<HTMLCanvasElement | null> {
  if (!doc || page < 1 || page > doc.numPages) return null;
  const width = Math.max(24, Math.round(targetWidth));
  const key = `${page}@${width}`;
  const hit = cache.get(key);
  if (hit) return hit;

  const pdfPage = await doc.getPage(page);
  // Scale 1 gives CSS-pixel size; derive the scale that hits our width.
  const base = pdfPage.getViewport({ scale: 1 });
  const scale = width / base.width;
  // Render at device resolution so the preview stays sharp.
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const viewport = pdfPage.getViewport({ scale: scale * dpr });

  const canvas = document.createElement("canvas");
  canvas.width = Math.ceil(viewport.width);
  canvas.height = Math.ceil(viewport.height);
  canvas.style.width = `${width}px`;
  canvas.style.height = `${Math.round(base.height * scale)}px`;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  await pdfPage.render({ canvasContext: ctx, viewport }).promise;

  if (cache.size >= CACHE_LIMIT) {
    const oldest = cache.keys().next().value;
    if (oldest) cache.delete(oldest);
  }
  cache.set(key, canvas);
  return canvas;
}

/**
 * Draw a page into an arbitrary rectangle of a destination context,
 * rotated by a multiple of 90 degrees and letterboxed to preserve the
 * page's aspect ratio — the same fit the exporter applies.
 */
export async function drawPageInto(
  ctx: CanvasRenderingContext2D,
  page: number,
  x: number,
  y: number,
  w: number,
  h: number,
  rotation: number
): Promise<boolean> {
  const quarter = ((rotation % 360) + 360) % 360;
  const swap = quarter === 90 || quarter === 270;
  const source = await renderPage(page, Math.round((swap ? h : w) * 2));
  if (!source) return false;

  const sw = source.width;
  const sh = source.height;
  const fitW = swap ? h : w;
  const fitH = swap ? w : h;
  const scale = Math.min(fitW / sw, fitH / sh);
  const dw = sw * scale;
  const dh = sh * scale;

  ctx.save();
  ctx.translate(x + w / 2, y + h / 2);
  ctx.rotate((quarter * Math.PI) / 180);
  ctx.drawImage(source, -dw / 2, -dh / 2, dw, dh);
  ctx.restore();
  return true;
}
