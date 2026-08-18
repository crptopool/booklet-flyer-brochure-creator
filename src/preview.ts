/**
 * Canvas previews that composite the user's real page artwork.
 *
 * These use the same sheet geometry the exporter consumes, so a preview
 * cannot drift from the file that gets written. When no document is
 * loaded the caller falls back to the schematic SVG diagrams.
 */

import { drawPageInto, isLoaded } from "./pdfRender";

const INK = "#1a1a2e";
const MUTED = "#5b6480";
const ACCENT = "#2c5fc4";
const EDGE = "#ccd5ee";
const WARN = "#c2410c";
const METAL = "#8894b0";
const GLUE = "#c8873f";
const BOARD = "#3d4663";

interface Placement {
  page: number | null;
  x: number;
  y: number;
  width: number;
  height: number;
  rotation: number;
}

export interface SheetSide {
  sheet_number: number;
  side: string;
  width: number;
  height: number;
  placements: Placement[];
  fold_x: number[];
  fold_y?: number[];
  cut_x?: number[];
  cut_y?: number[];
  stock: string;
}

function newCanvas(cssWidth: number, cssHeight: number): { canvas: HTMLCanvasElement; ctx: CanvasRenderingContext2D } {
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const canvas = document.createElement("canvas");
  canvas.width = Math.ceil(cssWidth * dpr);
  canvas.height = Math.ceil(cssHeight * dpr);
  canvas.style.width = `${cssWidth}px`;
  canvas.style.height = `${cssHeight}px`;
  const ctx = canvas.getContext("2d")!;
  ctx.scale(dpr, dpr);
  return { canvas, ctx };
}

function label(ctx: CanvasRenderingContext2D, text: string, x: number, y: number, size = 11, fill = MUTED, align: CanvasTextAlign = "center") {
  ctx.fillStyle = fill;
  ctx.font = `${size}px Inter, Helvetica, Arial, sans-serif`;
  ctx.textAlign = align;
  ctx.fillText(text, x, y);
}

function dashedRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, colour: string, dash: number[]) {
  ctx.save();
  ctx.setLineDash(dash);
  ctx.strokeStyle = colour;
  ctx.lineWidth = 1;
  ctx.strokeRect(x, y, w, h);
  ctx.restore();
}

/** One physical sheet side with real page artwork placed in each cell. */
export async function paintSheet(side: SheetSide, showMarks: boolean, maxWidth = 440): Promise<HTMLCanvasElement> {
  const pad = 28;
  const headroom = 46;
  const scale = Math.min((maxWidth - pad * 2) / side.width, 300 / side.height);
  const sw = side.width * scale;
  const sh = side.height * scale;
  const { canvas, ctx } = newCanvas(sw + pad * 2, sh + pad + headroom);
  const ox = pad;
  const oy = 26;

  // PDF space runs bottom-up; canvas runs top-down.
  const toY = (y: number, h: number) => oy + sh - (y + h) * scale;

  ctx.fillStyle = "#ffffff";
  ctx.fillRect(ox, oy, sw, sh);
  ctx.strokeStyle = MUTED;
  ctx.lineWidth = 1.2;
  ctx.strokeRect(ox, oy, sw, sh);

  for (const p of side.placements) {
    const px = ox + p.x * scale;
    const py = toY(p.y, p.height);
    const cw = p.width * scale;
    const ch = p.height * scale;

    if (p.page === null) {
      dashedRect(ctx, px, py, cw, ch, EDGE, [4, 3]);
      label(ctx, "blank", px + cw / 2, py + ch / 2 + 4, 10);
      continue;
    }

    ctx.save();
    ctx.beginPath();
    ctx.rect(px, py, cw, ch);
    ctx.clip();
    const drawn = await drawPageInto(ctx, p.page, px, py, cw, ch, p.rotation);
    ctx.restore();

    if (!drawn) {
      ctx.fillStyle = "#eef1fa";
      ctx.fillRect(px, py, cw, ch);
      ctx.save();
      ctx.translate(px + cw / 2, py + ch / 2);
      ctx.rotate((p.rotation * Math.PI) / 180);
      label(ctx, String(p.page), 0, 9, 26, ACCENT);
      ctx.restore();
    }
    ctx.strokeStyle = EDGE;
    ctx.lineWidth = 1;
    ctx.strokeRect(px, py, cw, ch);

    // Page number badge, so the sequence stays readable over artwork.
    ctx.fillStyle = "rgba(44,95,196,0.92)";
    ctx.fillRect(px + 3, py + 3, 22, 15);
    label(ctx, String(p.page), px + 14, py + 14, 10, "#ffffff");
  }

  const crease = (from: [number, number], to: [number, number], at: [number, number]) => {
    ctx.save();
    ctx.setLineDash([5, 4]);
    ctx.strokeStyle = WARN;
    ctx.lineWidth = 1.2;
    ctx.beginPath();
    ctx.moveTo(from[0], from[1]);
    ctx.lineTo(to[0], to[1]);
    ctx.stroke();
    ctx.restore();
    label(ctx, "fold", at[0], at[1], 9, WARN);
  };

  for (const fx of side.fold_x) {
    const x = ox + fx * scale;
    crease([x, oy - 6], [x, oy + sh + 6], [x, oy - 9]);
  }
  // A sheet folded more than once creases both ways.
  for (const fy of side.fold_y ?? []) {
    // PDF space measures up from the bottom; the canvas measures down.
    const y = oy + sh - fy * scale;
    crease([ox - 6, y], [ox + sw + 6, y], [ox - 14, y - 3]);
  }

  // Cuts are solid where folds are dashed: the waste past a cut line is
  // removed, and the scissors glyph says so at a glance.
  const cut = (from: [number, number], to: [number, number], at: [number, number]) => {
    ctx.save();
    ctx.strokeStyle = WARN;
    ctx.lineWidth = 1.6;
    ctx.beginPath();
    ctx.moveTo(from[0], from[1]);
    ctx.lineTo(to[0], to[1]);
    ctx.stroke();
    ctx.restore();
    label(ctx, "✂ cut", at[0], at[1], 9, WARN);
  };
  for (const cx of side.cut_x ?? []) {
    const x = ox + cx * scale;
    cut([x, oy - 6], [x, oy + sh + 6], [x + 16, oy - 9]);
  }
  for (const cy of side.cut_y ?? []) {
    const y = oy + sh - cy * scale;
    cut([ox - 6, y], [ox + sw + 6, y], [ox + sw - 16, y - 5]);
  }

  if (showMarks) {
    ctx.strokeStyle = INK;
    ctx.lineWidth = 0.7;
    for (const p of side.placements) {
      const l = ox + p.x * scale;
      const r = l + p.width * scale;
      const t = toY(p.y, p.height);
      const b = t + p.height * scale;
      for (const [cx, cy, dx, dy] of [
        [l, b, -1, 1], [r, b, 1, 1], [l, t, -1, -1], [r, t, 1, -1],
      ] as Array<[number, number, number, number]>) {
        ctx.beginPath();
        ctx.moveTo(cx + dx * 3, cy);
        ctx.lineTo(cx + dx * 10, cy);
        ctx.moveTo(cx, cy + dy * 3);
        ctx.lineTo(cx, cy + dy * 10);
        ctx.stroke();
      }
    }
  }

  const rotated = side.placements.some((p) => p.rotation !== 0);
  const stock = side.stock === "cover" ? "Cover sheet" : "Text sheet";
  label(ctx, `${stock} ${side.sheet_number} — ${side.side}${rotated ? " (rotated 180° for the flip)" : ""}`, (sw + pad * 2) / 2, 16, 11, INK);
  label(ctx, `${(side.width / 2.8346).toFixed(0)} × ${(side.height / 2.8346).toFixed(0)} mm sheet${isLoaded() ? "" : " · import a PDF to see real artwork"}`, (sw + pad * 2) / 2, oy + sh + 20, 9);
  return canvas;
}

/** Draws the binding hardware down the spine of an open book. */
function paintSpine(ctx: CanvasRenderingContext2D, key: string, cx: number, top: number, height: number) {
  switch (key) {
    case "saddle_stitch":
      ctx.strokeStyle = EDGE;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(cx, top);
      ctx.lineTo(cx, top + height);
      ctx.stroke();
      ctx.fillStyle = METAL;
      for (const f of [0.3, 0.7]) ctx.fillRect(cx - 2, top + height * f - 5, 4, 10);
      break;
    case "perfect":
      ctx.fillStyle = GLUE;
      ctx.globalAlpha = 0.75;
      ctx.fillRect(cx - 4, top, 8, height);
      ctx.globalAlpha = 1;
      break;
    case "hardcover":
      ctx.fillStyle = BOARD;
      ctx.fillRect(cx - 5, top - 4, 10, height + 8);
      break;
    case "spiral":
      ctx.strokeStyle = ACCENT;
      ctx.lineWidth = 2;
      ctx.lineCap = "round";
      for (let i = 0; i < 9; i++) {
        const y = top + 6 + i * ((height - 12) / 8);
        ctx.beginPath();
        ctx.moveTo(cx - 8, y);
        ctx.bezierCurveTo(cx - 2, y - 7, cx + 3, y + 4, cx + 8, y - 3);
        ctx.stroke();
      }
      break;
    case "wire_o":
      ctx.strokeStyle = METAL;
      ctx.lineWidth = 2;
      for (let i = 0; i < 7; i++) {
        const y = top + 8 + i * ((height - 16) / 6);
        ctx.beginPath();
        ctx.moveTo(cx - 6, y);
        ctx.bezierCurveTo(cx - 6, y - 6, cx + 6, y - 6, cx + 6, y);
        ctx.stroke();
      }
      break;
  }
}

/** One spread of the bound document, showing the real page artwork. */
export async function paintBoundSpread(
  key: string,
  left: number | null,
  right: number | null,
  leftPos: number | null,
  rightPos: number | null,
  aspect: number,
  bindSide: string
): Promise<HTMLCanvasElement> {
  const ph = 240;
  const pw = Math.max(90, Math.round(ph / Math.max(0.5, aspect)));
  const top = 26;
  const { canvas, ctx } = newCanvas(pw * 2 + 60, ph + 74);
  const cx = (pw * 2 + 60) / 2;

  const paintFace = async (x: number, page: number | null, pos: number | null) => {
    if (pos === null) {
      dashedRect(ctx, x, top, pw, ph, EDGE, [4, 4]);
      label(ctx, "outside cover", x + pw / 2, top + ph / 2, 11);
      return;
    }
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(x, top, pw, ph);
    if (page === null) {
      label(ctx, "blank", x + pw / 2, top + ph / 2 - 4, 12);
      label(ctx, "inserted for binding", x + pw / 2, top + ph / 2 + 12, 9);
    } else {
      ctx.save();
      ctx.beginPath();
      ctx.rect(x, top, pw, ph);
      ctx.clip();
      const drawn = await drawPageInto(ctx, page, x, top, pw, ph, 0);
      ctx.restore();
      if (!drawn) {
        ctx.fillStyle = "#eef1fa";
        ctx.fillRect(x, top, pw, ph);
        label(ctx, `page ${page}`, x + pw / 2, top + ph / 2, 12, ACCENT);
      }
    }
    ctx.strokeStyle = MUTED;
    ctx.lineWidth = 1.2;
    ctx.strokeRect(x, top, pw, ph);
    label(ctx, `reading page ${pos}`, x + pw / 2, top + ph + 18, 10, INK);
  };

  await paintFace(cx - pw, left, leftPos);
  await paintFace(cx, right, rightPos);
  if (bindSide !== "top") paintSpine(ctx, key, cx, top, ph);

  label(ctx, "How the pages read once bound", cx, 15, 11, INK);
  return canvas;
}
