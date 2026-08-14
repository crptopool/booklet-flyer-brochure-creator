/**
 * Inline SVG illustrations for the booklet screen.
 *
 * Every diagram is generated from the same settings the calculations use,
 * so what the user sees always matches what the plan reports. Drawing them
 * as inline SVG rather than shipping bitmaps keeps them crisp at any size
 * and works under the application's strict content-security policy, which
 * blocks external image loads.
 */

const INK = "#1a1a2e";
const MUTED = "#7c85a3";
const ACCENT = "#2c5fc4";
const PAPER = "#ffffff";
const PAGE_FILL = "#eef1fa";
const PAGE_EDGE = "#ccd5ee";
const METAL = "#8894b0";
const GLUE = "#c8873f";
const BOARD = "#3d4663";
const WARN = "#c2410c";

function svg(width: number, height: number, body: string, title: string): string {
  return `<svg viewBox="0 0 ${width} ${height}" width="100%" height="auto" role="img"
      aria-label="${esc(title)}" xmlns="http://www.w3.org/2000/svg"
      style="max-width:${width}px">
      <title>${esc(title)}</title>${body}</svg>`;
}

function esc(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function label(x: number, y: number, text: string, anchor = "middle", size = 11, fill = MUTED): string {
  return `<text x="${x}" y="${y}" text-anchor="${anchor}" font-size="${size}"
    font-family="Inter, Helvetica, Arial, sans-serif" fill="${fill}">${esc(text)}</text>`;
}

function arrow(id: string, color: string): string {
  return `<defs><marker id="${id}" markerWidth="9" markerHeight="9" refX="7" refY="3"
    orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="${color}"/></marker></defs>`;
}

// ---------------------------------------------------------------------------
// 1. Binding mechanism — an edge-on cross-section of the finished book
// ---------------------------------------------------------------------------

/** Cross-section showing how each binding method physically holds pages. */
export function bindingDiagram(key: string): string {
  const W = 300;
  const H = 170;
  const cx = 150;
  const top = 34;

  switch (key) {
    case "saddle_stitch": {
      // Nested folded sheets seen end-on: concentric U shapes, stapled
      // through the common fold at the bottom.
      let body = "";
      for (let i = 0; i < 4; i++) {
        const r = 16 + i * 13;
        body += `<path d="M ${cx - r} ${top} L ${cx - r} ${top + 62} Q ${cx - r} ${top + 78} ${cx} ${top + 78}
          Q ${cx + r} ${top + 78} ${cx + r} ${top + 62} L ${cx + r} ${top}"
          fill="none" stroke="${i === 3 ? ACCENT : PAGE_EDGE}" stroke-width="${i === 3 ? 2.5 : 2}"/>`;
      }
      // Staples through the fold.
      for (const sx of [cx - 22, cx + 22]) {
        body += `<path d="M ${sx - 6} ${top + 84} L ${sx - 6} ${top + 74} L ${sx + 6} ${top + 74} L ${sx + 6} ${top + 84}"
          fill="none" stroke="${METAL}" stroke-width="3.5" stroke-linecap="round"/>`;
      }
      body +=
        label(cx, top + 104, "Staples through the centre fold", "middle", 11, INK) +
        label(cx, top + 122, "Sheets nest inside one another", "middle", 10) +
        label(cx - 62, top - 8, "outer sheet", "middle", 9) +
        `<line x1="${cx - 62}" y1="${top - 4}" x2="${cx - 55}" y2="${top + 8}" stroke="${MUTED}" stroke-width="1"/>`;
      return svg(W, H, body, "Saddle stitch: nested folded sheets stapled through the fold");
    }

    case "perfect": {
      // Flat stack of single leaves, ground and glued into a wrap cover.
      let body = "";
      const left = cx - 78;
      const right = cx + 78;
      for (let i = 0; i < 11; i++) {
        const y = top + 8 + i * 5.5;
        body += `<line x1="${left + 12}" y1="${y}" x2="${right}" y2="${y}" stroke="${PAGE_EDGE}" stroke-width="2.4"/>`;
      }
      // Glue block + wrap cover forming the square spine.
      body += `<rect x="${left + 4}" y="${top + 4}" width="9" height="66" fill="${GLUE}" rx="1"/>`;
      body += `<path d="M ${left + 4} ${top + 2} L ${left - 4} ${top + 2} L ${left - 4} ${top + 72} L ${left + 4} ${top + 72}"
        fill="none" stroke="${BOARD}" stroke-width="3" stroke-linejoin="round"/>`;
      body +=
        label(left - 6, top + 88, "square glued spine", "start", 11, INK) +
        label(cx + 20, top + 106, "Pages stay in normal reading order", "middle", 10) +
        label(cx + 20, top + 122, "Does not open flat — allow a deep gutter", "middle", 10, WARN);
      return svg(W, H, body, "Perfect binding: leaves glued into a square spine");
    }

    case "spiral": {
      // Punched leaves with a continuous coil threaded through.
      let body = "";
      const left = cx - 70;
      const right = cx + 82;
      for (let i = 0; i < 10; i++) {
        const y = top + 8 + i * 6;
        body += `<line x1="${left}" y1="${y}" x2="${right}" y2="${y}" stroke="${PAGE_EDGE}" stroke-width="2.6"/>`;
      }
      // Round punch holes.
      for (let i = 0; i < 10; i++) {
        body += `<circle cx="${left + 9}" cy="${top + 8 + i * 6}" r="1.9" fill="${PAPER}" stroke="${MUTED}" stroke-width="0.8"/>`;
      }
      // Coil: a run of slanted loops threading the holes.
      let coil = "";
      for (let i = 0; i < 10; i++) {
        const y = top + 8 + i * 6;
        coil += `M ${left + 16} ${y} C ${left - 4} ${y - 5}, ${left - 12} ${y + 5}, ${left + 4} ${y + 4} `;
      }
      body += `<path d="${coil}" fill="none" stroke="${ACCENT}" stroke-width="2.2" stroke-linecap="round"/>`;
      body +=
        label(cx + 10, top + 84, "continuous plastic coil through round holes", "middle", 11, INK) +
        label(cx + 10, top + 104, "Any page count · opens completely flat", "middle", 10) +
        label(cx + 10, top + 120, "Folds back on itself for hands-free use", "middle", 10);
      return svg(W, H, body, "Spiral binding: a plastic coil threaded through round punched holes");
    }

    case "wire_o": {
      // Punched leaves closed with twin metal loops.
      let body = "";
      const left = cx - 70;
      const right = cx + 82;
      for (let i = 0; i < 10; i++) {
        const y = top + 8 + i * 6;
        body += `<line x1="${left}" y1="${y}" x2="${right}" y2="${y}" stroke="${PAGE_EDGE}" stroke-width="2.6"/>`;
      }
      // Rectangular punch holes.
      for (let i = 0; i < 5; i++) {
        body += `<rect x="${left + 6}" y="${top + 9 + i * 12}" width="6" height="4" rx="1"
          fill="${PAPER}" stroke="${MUTED}" stroke-width="0.8"/>`;
      }
      // Twin loops: pairs of closed rings around the spine edge.
      for (let i = 0; i < 5; i++) {
        const y = top + 11 + i * 12;
        body += `<path d="M ${left + 8} ${y} C ${left - 12} ${y - 8}, ${left - 12} ${y + 8}, ${left + 8} ${y}"
          fill="none" stroke="${METAL}" stroke-width="2.4"/>`;
      }
      body +=
        label(cx + 10, top + 84, "twin metal loops through rectangular holes", "middle", 11, INK) +
        label(cx + 10, top + 104, "Any page count · opens completely flat", "middle", 10) +
        label(cx + 10, top + 120, "More formal finish than plastic coil", "middle", 10);
      return svg(W, H, body, "Wire-O binding: twin metal loops through rectangular punched holes");
    }

    case "hardcover": {
      // Sewn text block cased into rigid boards with a hinge groove.
      let body = "";
      const left = cx - 74;
      const right = cx + 74;
      for (let i = 0; i < 9; i++) {
        const y = top + 14 + i * 5;
        body += `<line x1="${left + 16}" y1="${y}" x2="${right - 8}" y2="${y}" stroke="${PAGE_EDGE}" stroke-width="2.4"/>`;
      }
      // Rigid boards, top and bottom, overhanging the text block.
      body += `<rect x="${left + 6}" y="${top + 2}" width="${right - left - 4}" height="7" rx="1.5" fill="${BOARD}"/>`;
      body += `<rect x="${left + 6}" y="${top + 58}" width="${right - left - 4}" height="7" rx="1.5" fill="${BOARD}"/>`;
      // Rounded spine joining the two boards.
      body += `<path d="M ${left + 6} ${top + 2} C ${left - 12} ${top + 2}, ${left - 12} ${top + 65}, ${left + 6} ${top + 65}"
        fill="none" stroke="${BOARD}" stroke-width="6" stroke-linecap="round"/>`;
      // Hinge grooves.
      for (const y of [top + 2, top + 58]) {
        body += `<line x1="${left + 17}" y1="${y}" x2="${left + 17}" y2="${y + 7}" stroke="${PAPER}" stroke-width="1.6"/>`;
      }
      body +=
        label(left + 2, top + 84, "rigid boards", "start", 10, INK) +
        label(right, top + 84, "board overhang", "end", 10, INK) +
        label(cx, top + 104, "Signatures sewn into a text block, then cased in", "middle", 10) +
        label(cx, top + 120, "Allow for spine, hinge groove and turn-in", "middle", 10);
      return svg(W, H, body, "Case binding: a sewn text block cased into rigid boards");
    }

    default:
      return svg(W, H, label(cx, H / 2, "No binding selected", "middle", 12), "No binding");
  }
}

// ---------------------------------------------------------------------------
// 2. Pages per sheet — what lands on one side of the paper
// ---------------------------------------------------------------------------

/** Grid layout of one sheet side, with fold lines where the sheet folds. */
export function pagesPerSheetDiagram(perSide: number, folds: number, landscape: boolean): string {
  const W = 300;
  const H = 172;
  // Sheet proportions follow the chosen orientation.
  const sheetW = landscape ? 210 : 132;
  const sheetH = landscape ? 100 : 148;
  const x0 = (W - sheetW) / 2;
  const y0 = 16;

  // Split the sheet into the requested number of cells.
  const cols = perSide === 1 ? 1 : perSide === 2 ? 2 : perSide === 4 ? 2 : 4;
  const rows = Math.max(1, Math.round(perSide / cols));
  const cw = sheetW / cols;
  const ch = sheetH / rows;

  let body = `<rect x="${x0}" y="${y0}" width="${sheetW}" height="${sheetH}" rx="3"
    fill="${PAPER}" stroke="${MUTED}" stroke-width="1.5"/>`;

  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      const n = r * cols + c + 1;
      body += `<rect x="${x0 + c * cw + 3}" y="${y0 + r * ch + 3}" width="${cw - 6}" height="${ch - 6}"
        rx="2" fill="${PAGE_FILL}" stroke="${PAGE_EDGE}" stroke-width="1"/>`;
      body += label(x0 + c * cw + cw / 2, y0 + r * ch + ch / 2 + 5, String(n), "middle", 14, ACCENT);
    }
  }

  // Fold lines sit on the internal grid divisions.
  if (folds > 0) {
    for (let c = 1; c < cols; c++) {
      body += `<line x1="${x0 + c * cw}" y1="${y0 - 5}" x2="${x0 + c * cw}" y2="${y0 + sheetH + 5}"
        stroke="${WARN}" stroke-width="1.5" stroke-dasharray="5 4"/>`;
    }
    if (folds > 1) {
      for (let r = 1; r < rows; r++) {
        body += `<line x1="${x0 - 5}" y1="${y0 + r * ch}" x2="${x0 + sheetW + 5}" y2="${y0 + r * ch}"
          stroke="${WARN}" stroke-width="1.5" stroke-dasharray="5 4"/>`;
      }
    }
    body += label(x0 + sheetW + 4, y0 + 6, "fold", "start", 9, WARN);
  }

  const foldText = folds === 0 ? "no folds — flat sheet" : folds === 1 ? "folded once" : `folded ${folds} times`;
  body +=
    label(W / 2, y0 + sheetH + 20, `${perSide} page${perSide > 1 ? "s" : ""} on this side · ${foldText}`, "middle", 11, INK) +
    label(W / 2, y0 + sheetH + 36, `Sheet fed ${landscape ? "landscape" : "portrait"}`, "middle", 10);

  return svg(W, H, body, `${perSide} pages per sheet side`);
}

// ---------------------------------------------------------------------------
// 3. Duplex flip — how the reverse side comes out
// ---------------------------------------------------------------------------

/**
 * Front side, the flip that happens inside the printer, and the resulting
 * back side. The digit orientation shows exactly what the user will see.
 */
export function duplexFlipDiagram(
  mode: string,
  landscape: boolean,
  backRotation: number,
  recommended: boolean
): string {
  const W = 320;
  const H = 190;
  const sheetW = landscape ? 96 : 66;
  const sheetH = landscape ? 66 : 92;
  const y0 = 30;
  const leftX = 16;
  const rightX = W - sheetW - 16;

  const simplex = mode === "simplex";
  const inverted = backRotation !== 0;

  const sheet = (x: number, digit: string, rotate: boolean, caption: string) => {
    const cxp = x + sheetW / 2;
    const cyp = y0 + sheetH / 2;
    return (
      `<rect x="${x}" y="${y0}" width="${sheetW}" height="${sheetH}" rx="3"
        fill="${PAPER}" stroke="${MUTED}" stroke-width="1.5"/>` +
      // A corner mark makes the rotation unmistakable.
      `<path d="M ${x + 6} ${y0 + 6} L ${x + 20} ${y0 + 6} L ${x + 6} ${y0 + 20} z"
        fill="${PAGE_EDGE}" transform="rotate(${rotate ? 180 : 0} ${cxp} ${cyp})"/>` +
      `<text x="${cxp}" y="${cyp + 11}" text-anchor="middle" font-size="30" font-weight="600"
        font-family="Inter, Helvetica, Arial, sans-serif" fill="${ACCENT}"
        transform="rotate(${rotate ? 180 : 0} ${cxp} ${cyp})">${esc(digit)}</text>` +
      label(cxp, y0 + sheetH + 17, caption, "middle", 10, INK)
    );
  };

  let body = arrow("flip", ACCENT);
  body += sheet(leftX, "1", false, "Front side");

  if (simplex) {
    body +=
      `<line x1="${leftX + sheetW + 20}" y1="${y0 + sheetH / 2}" x2="${rightX - 20}" y2="${y0 + sheetH / 2}"
        stroke="${MUTED}" stroke-width="1.5" stroke-dasharray="4 4"/>` +
      `<rect x="${rightX}" y="${y0}" width="${sheetW}" height="${sheetH}" rx="3" fill="${PAPER}"
        stroke="${PAGE_EDGE}" stroke-width="1.5" stroke-dasharray="5 4"/>` +
      label(rightX + sheetW / 2, y0 + sheetH / 2 + 4, "blank", "middle", 12) +
      label(rightX + sheetW / 2, y0 + sheetH + 17, "Back side", "middle", 10, INK) +
      label(W / 2, 16, "Single-sided — the reverse is never printed", "middle", 11, INK);
    return svg(W, H, body, "Single-sided printing");
  }

  // The flip axis: vertical when the sheet turns like a book page.
  const axisVertical = !inverted;
  const midX = (leftX + sheetW + rightX) / 2;
  const axisIcon = axisVertical
    ? `<line x1="${midX}" y1="${y0 - 4}" x2="${midX}" y2="${y0 + sheetH + 4}"
         stroke="${ACCENT}" stroke-width="1.5" stroke-dasharray="4 3"/>`
    : `<line x1="${midX - 34}" y1="${y0 + sheetH / 2}" x2="${midX + 34}" y2="${y0 + sheetH / 2}"
         stroke="${ACCENT}" stroke-width="1.5" stroke-dasharray="4 3"/>`;

  body +=
    axisIcon +
    `<path d="M ${leftX + sheetW + 14} ${y0 + sheetH / 2 - 16} Q ${midX} ${y0 - 6} ${rightX - 14} ${y0 + sheetH / 2 - 16}"
      fill="none" stroke="${ACCENT}" stroke-width="2" marker-end="url(#flip)"/>` +
    label(midX, y0 + sheetH / 2 + 26, axisVertical ? "turns about a" : "turns about a", "middle", 9) +
    label(midX, y0 + sheetH / 2 + 38, axisVertical ? "vertical axis" : "horizontal axis", "middle", 9, ACCENT);

  body += sheet(rightX, "2", inverted, "Back side, as printed");

  const heading = inverted
    ? "Back side lands upside down — pages are pre-rotated 180° to correct it"
    : "Back side lands upright — no correction needed";
  body += label(W / 2, 16, heading, "middle", 11, inverted ? WARN : INK);

  const footer = recommended
    ? "✓ Correct flip setting for this sheet orientation"
    : "⚠ The other flip setting avoids the correction entirely";
  body += label(W / 2, H - 8, footer, "middle", 10, recommended ? "#22662c" : WARN);

  return svg(W, H, body, `Duplex ${mode} flip on a ${landscape ? "landscape" : "portrait"} sheet`);
}

// ---------------------------------------------------------------------------
// 4. Gutter and punch zone — where content must not go
// ---------------------------------------------------------------------------

/** A spread showing the binding margin, and the punch zone when relevant. */
export function gutterDiagram(marginMm: number, punched: boolean, side: string, holeShape: string): string {
  const W = 300;
  const H = 168;
  const pageW = 106;
  const pageH = 118;
  const gap = 6;
  const x0 = (W - pageW * 2 - gap) / 2;
  const y0 = 14;

  // The gutter band scales with the setting so the user sees it grow.
  const band = Math.max(6, Math.min(34, marginMm * 1.7));
  const bindTop = side === "top";

  let body = "";
  for (const [i, px] of [x0, x0 + pageW + gap].entries()) {
    body += `<rect x="${px}" y="${y0}" width="${pageW}" height="${pageH}" rx="2"
      fill="${PAPER}" stroke="${MUTED}" stroke-width="1.3"/>`;
    // Body text suggestion, kept clear of the binding edge.
    const inner = bindTop ? y0 + band + 8 : y0 + 10;
    const innerH = bindTop ? pageH - band - 20 : pageH - 20;
    const lx = i === 0 ? px + 8 : px + band + 8;
    const lw = pageW - band - 16;
    for (let l = 0; l < 6; l++) {
      const ly = inner + l * (innerH / 6);
      if (ly > y0 + pageH - 12) break;
      body += `<line x1="${bindTop ? px + 8 : lx}" y1="${ly}" x2="${bindTop ? px + pageW - 8 : lx + lw}" y2="${ly}"
        stroke="${PAGE_EDGE}" stroke-width="3"/>`;
    }
  }

  // Highlight the binding band on the inner edges.
  const bandFill = punched ? "rgba(194,65,12,0.14)" : "rgba(44,95,196,0.12)";
  const bandStroke = punched ? WARN : ACCENT;
  if (bindTop) {
    body += `<rect x="${x0}" y="${y0}" width="${pageW * 2 + gap}" height="${band}"
      fill="${bandFill}" stroke="${bandStroke}" stroke-width="1" stroke-dasharray="4 3"/>`;
  } else {
    body += `<rect x="${x0 + pageW - band}" y="${y0}" width="${band}" height="${pageH}"
      fill="${bandFill}" stroke="${bandStroke}" stroke-width="1" stroke-dasharray="4 3"/>`;
    body += `<rect x="${x0 + pageW + gap}" y="${y0}" width="${band}" height="${pageH}"
      fill="${bandFill}" stroke="${bandStroke}" stroke-width="1" stroke-dasharray="4 3"/>`;
  }

  // Punch holes sit inside the band.
  if (punched) {
    const holes = 6;
    for (let i = 0; i < holes; i++) {
      if (bindTop) {
        const hx = x0 + 18 + i * ((pageW * 2 + gap - 36) / (holes - 1));
        body +=
          holeShape === "round"
            ? `<circle cx="${hx}" cy="${y0 + band / 2}" r="2.6" fill="${PAPER}" stroke="${WARN}" stroke-width="1"/>`
            : `<rect x="${hx - 3}" y="${y0 + band / 2 - 2.2}" width="6" height="4.4" rx="1" fill="${PAPER}" stroke="${WARN}" stroke-width="1"/>`;
      } else {
        const hy = y0 + 14 + i * ((pageH - 28) / (holes - 1));
        for (const hx of [x0 + pageW - band / 2, x0 + pageW + gap + band / 2]) {
          body +=
            holeShape === "round"
              ? `<circle cx="${hx}" cy="${hy}" r="2.6" fill="${PAPER}" stroke="${WARN}" stroke-width="1"/>`
              : `<rect x="${hx - 3}" y="${hy - 2.2}" width="6" height="4.4" rx="1" fill="${PAPER}" stroke="${WARN}" stroke-width="1"/>`;
        }
      }
    }
  }

  const caption = punched
    ? `${marginMm.toFixed(0)} mm punch-safe zone — the holes destroy this strip`
    : `${marginMm.toFixed(0)} mm gutter — content here disappears into the binding`;
  body +=
    label(W / 2, y0 + pageH + 20, caption, "middle", 11, punched ? WARN : INK) +
    label(W / 2, y0 + pageH + 36, `Bound on the ${esc(side)} edge`, "middle", 10);

  return svg(W, H, body, `Binding margin of ${marginMm} mm on the ${side} edge`);
}

// ---------------------------------------------------------------------------
// 5b. Simulation of the finished, bound document
// ---------------------------------------------------------------------------

/** Draws the binding hardware down the spine of an open book. */
function spineHardware(key: string, cx: number, top: number, height: number): string {
  let out = "";
  switch (key) {
    case "saddle_stitch":
      out += `<line x1="${cx}" y1="${top}" x2="${cx}" y2="${top + height}" stroke="${PAGE_EDGE}" stroke-width="1"/>`;
      for (const f of [0.3, 0.7]) {
        out += `<rect x="${cx - 2}" y="${top + height * f - 5}" width="4" height="10" rx="1" fill="${METAL}"/>`;
      }
      break;
    case "perfect":
      out += `<rect x="${cx - 4}" y="${top}" width="8" height="${height}" fill="${GLUE}" opacity="0.75"/>`;
      break;
    case "hardcover":
      out += `<rect x="${cx - 5}" y="${top - 4}" width="10" height="${height + 8}" rx="2" fill="${BOARD}"/>`;
      break;
    case "spiral":
      for (let i = 0; i < 9; i++) {
        const y = top + 6 + i * ((height - 12) / 8);
        out += `<path d="M ${cx - 8} ${y} C ${cx - 2} ${y - 7}, ${cx + 3} ${y + 4}, ${cx + 8} ${y - 3}"
          fill="none" stroke="${ACCENT}" stroke-width="2" stroke-linecap="round"/>`;
      }
      break;
    case "wire_o":
      for (let i = 0; i < 7; i++) {
        const y = top + 8 + i * ((height - 16) / 6);
        out += `<path d="M ${cx - 6} ${y} C ${cx - 6} ${y - 6}, ${cx + 6} ${y - 6}, ${cx + 6} ${y}"
          fill="none" stroke="${METAL}" stroke-width="2"/>`;
      }
      break;
  }
  return out;
}

/**
 * One spread of the finished document, as it reads once bound.
 *
 * `left`/`right` are source page numbers, `null` for an inserted blank.
 * `position` is the reading position of the right-hand page.
 */
export function boundSpreadDiagram(
  key: string,
  left: number | null,
  right: number | null,
  leftPos: number | null,
  rightPos: number | null,
  gutterMm: number,
  bindSide: string
): string {
  const W = 420;
  const H = 260;
  const pw = 150;
  const ph = 200;
  const top = 22;
  const cx = W / 2;
  const gutter = Math.max(4, Math.min(26, gutterMm * 1.6));

  const face = (x: number, page: number | null, pos: number | null, innerOnRight: boolean) => {
    if (pos === null) {
      // Outside the book — the cover's outer face.
      return `<rect x="${x}" y="${top}" width="${pw}" height="${ph}" rx="2" fill="#f4f5fa"
        stroke="${PAGE_EDGE}" stroke-width="1" stroke-dasharray="4 4"/>` +
        label(x + pw / 2, top + ph / 2, "outside cover", "middle", 11);
    }
    const blank = page === null;
    let out = `<rect x="${x}" y="${top}" width="${pw}" height="${ph}" rx="2"
      fill="${blank ? "#fbfbfe" : PAPER}" stroke="${MUTED}" stroke-width="1.2"/>`;
    if (blank) {
      out += label(x + pw / 2, top + ph / 2 - 4, "blank", "middle", 12);
      out += label(x + pw / 2, top + ph / 2 + 12, "inserted for binding", "middle", 9);
    } else {
      // Text block, pushed clear of the gutter.
      const padL = innerOnRight ? 12 : gutter + 8;
      const padR = innerOnRight ? gutter + 8 : 12;
      for (let i = 0; i < 9; i++) {
        const ly = top + 24 + i * 17;
        const w = i === 8 ? (pw - padL - padR) * 0.55 : pw - padL - padR;
        out += `<line x1="${x + padL}" y1="${ly}" x2="${x + padL + w}" y2="${ly}"
          stroke="${PAGE_EDGE}" stroke-width="4" stroke-linecap="round"/>`;
      }
      out += label(x + pw / 2, top + ph - 10, `source page ${page}`, "middle", 9, ACCENT);
    }
    out += label(x + pw / 2, top + ph + 18, pos === null ? "" : `reading page ${pos}`, "middle", 10, INK);
    return out;
  };

  const topEdge = bindSide === "top";
  let body = "";
  body += face(cx - pw - (topEdge ? 4 : 0), left, leftPos, false);
  body += face(cx + (topEdge ? 4 : 0), right, rightPos, true);
  if (!topEdge) body += spineHardware(key, cx, top, ph);

  body += label(W / 2, 13, "How the pages read once bound", "middle", 11, INK);
  return svg(W, H, body, "Simulated spread of the bound document");
}

/**
 * One physical sheet side exactly as it will be printed, including the
 * 180 degree rotation applied to back sides when the flip demands it.
 */
export function sheetSideDiagram(
  side: { sheet_number: number; side: string; width: number; height: number; fold_x: number[]; placements: Array<{ page: number | null; x: number; y: number; width: number; height: number; rotation: number }> },
  showMarks: boolean
): string {
  const W = 360;
  const H = 250;
  const pad = 26;
  const s = Math.min((W - pad * 2) / side.width, (H - pad * 2 - 24) / side.height);
  const sw = side.width * s;
  const sh = side.height * s;
  const ox = (W - sw) / 2;
  const oy = 30;

  // PDF space has its origin bottom-left; SVG is top-left.
  const toY = (y: number, h: number) => oy + sh - (y + h) * s;

  let body = `<rect x="${ox}" y="${oy}" width="${sw}" height="${sh}" rx="2"
    fill="${PAPER}" stroke="${MUTED}" stroke-width="1.4"/>`;

  for (const p of side.placements) {
    const px = ox + p.x * s;
    const py = toY(p.y, p.height);
    const cw = p.width * s;
    const ch = p.height * s;
    const blank = p.page === null;
    body += `<rect x="${px}" y="${py}" width="${cw}" height="${ch}"
      fill="${blank ? "#fafafd" : PAGE_FILL}" stroke="${PAGE_EDGE}" stroke-width="1"
      ${blank ? 'stroke-dasharray="4 3"' : ""}/>`;
    const mx = px + cw / 2;
    const my = py + ch / 2;
    if (blank) {
      body += label(mx, my + 4, "blank", "middle", 10);
    } else {
      body += `<text x="${mx}" y="${my + 9}" text-anchor="middle" font-size="26" font-weight="600"
        font-family="Inter, Helvetica, Arial, sans-serif" fill="${ACCENT}"
        transform="rotate(${p.rotation} ${mx} ${my})">${p.page}</text>`;
      // A corner tick makes the applied rotation visible.
      body += `<path d="M ${px + 4} ${py + 4} L ${px + 16} ${py + 4} L ${px + 4} ${py + 16} z"
        fill="${PAGE_EDGE}" transform="rotate(${p.rotation} ${mx} ${my})"/>`;
    }
  }

  for (const fx of side.fold_x) {
    const x = ox + fx * s;
    body += `<line x1="${x}" y1="${oy - 6}" x2="${x}" y2="${oy + sh + 6}"
      stroke="${WARN}" stroke-width="1.3" stroke-dasharray="5 4"/>`;
    body += label(x, oy - 10, "fold", "middle", 9, WARN);
  }

  if (showMarks) {
    for (const p of side.placements) {
      const l = ox + p.x * s;
      const r = l + p.width * s;
      const t = toY(p.y, p.height);
      const b = t + p.height * s;
      for (const [cxm, cym, dx, dy] of [
        [l, b, -1, 1], [r, b, 1, 1], [l, t, -1, -1], [r, t, 1, -1],
      ] as Array<[number, number, number, number]>) {
        body += `<line x1="${cxm + dx * 3}" y1="${cym}" x2="${cxm + dx * 10}" y2="${cym}" stroke="${INK}" stroke-width="0.7"/>`;
        body += `<line x1="${cxm}" y1="${cym + dy * 3}" x2="${cxm}" y2="${cym + dy * 10}" stroke="${INK}" stroke-width="0.7"/>`;
      }
    }
  }

  const rotated = side.placements.some((p) => p.rotation !== 0);
  body +=
    label(W / 2, 15, `Sheet ${side.sheet_number} — ${side.side}${rotated ? " (rotated 180° for the flip)" : ""}`, "middle", 11, INK) +
    label(W / 2, oy + sh + 20, `${(side.width / 2.8346).toFixed(0)} × ${(side.height / 2.8346).toFixed(0)} mm sheet`, "middle", 9);

  return svg(W, H, body, `Sheet ${side.sheet_number} ${side.side} as printed`);
}

// ---------------------------------------------------------------------------
// 5. After printing — what the finished, folded job looks like
// ---------------------------------------------------------------------------

/** The finished result: how sheets become a bound document. */
export function resultDiagram(key: string, sheets: number, totalPages: number): string {
  const W = 300;
  const H = 168;
  const cx = 150;

  if (key === "saddle_stitch") {
    // A folded booklet seen from the front, opened slightly.
    let body = "";
    const y0 = 26;
    const h = 86;
    const w = 62;
    body += `<path d="M ${cx} ${y0} L ${cx - w} ${y0 + 8} L ${cx - w} ${y0 + h + 8} L ${cx} ${y0 + h} z"
      fill="${PAGE_FILL}" stroke="${MUTED}" stroke-width="1.4"/>`;
    body += `<path d="M ${cx} ${y0} L ${cx + w} ${y0 + 8} L ${cx + w} ${y0 + h + 8} L ${cx} ${y0 + h} z"
      fill="${PAPER}" stroke="${MUTED}" stroke-width="1.4"/>`;
    body += `<line x1="${cx}" y1="${y0}" x2="${cx}" y2="${y0 + h}" stroke="${ACCENT}" stroke-width="2"/>`;
    for (const sy of [y0 + 26, y0 + 60]) {
      body += `<rect x="${cx - 2}" y="${sy}" width="4" height="9" rx="1" fill="${METAL}"/>`;
    }
    body +=
      label(cx - 34, y0 + h + 30, "page 2", "middle", 10) +
      label(cx + 34, y0 + h + 30, "page 1", "middle", 10) +
      label(cx, 16, `${sheets} folded sheet${sheets === 1 ? "" : "s"} → ${totalPages}-page booklet`, "middle", 11, INK) +
      label(cx, H - 8, "Fold in the centre, staple through the spine, trim the fore-edge", "middle", 10);
    return svg(W, H, body, "Finished saddle-stitched booklet");
  }

  if (key === "perfect" || key === "hardcover") {
    const hard = key === "hardcover";
    let body = "";
    const y0 = 30;
    const bh = 84;
    const bw = 76;
    const spine = 16;
    // Front cover and spine in a slight perspective.
    body += `<rect x="${cx - bw / 2}" y="${y0}" width="${bw}" height="${bh}" rx="2"
      fill="${hard ? BOARD : PAGE_FILL}" stroke="${MUTED}" stroke-width="1.4"/>`;
    body += `<path d="M ${cx - bw / 2} ${y0} L ${cx - bw / 2 - spine} ${y0 + 10}
      L ${cx - bw / 2 - spine} ${y0 + bh + 10} L ${cx - bw / 2} ${y0 + bh} z"
      fill="${hard ? "#2f3752" : GLUE}" stroke="${MUTED}" stroke-width="1.4"/>`;
    // Page block edge.
    body += `<path d="M ${cx + bw / 2} ${y0} L ${cx + bw / 2} ${y0 + bh}" stroke="${PAGE_EDGE}" stroke-width="3"/>`;
    body +=
      label(cx - bw / 2 - spine / 2, y0 + bh + 28, "spine", "middle", 10, INK) +
      label(cx, 16, `${totalPages} pages on ${sheets} leaves — square spine`, "middle", 11, INK) +
      label(cx, H - 8,
        hard
          ? "Text block cased into boards; spine width sets the cover layout"
          : "Spine width must be calculated before the cover can be laid out",
        "middle", 10);
    return svg(W, H, body, "Finished bound book with a square spine");
  }

  // Punched bindings: shown opened flat, which is their defining property.
  const wire = key === "wire_o";
  let body = "";
  const y0 = 34;
  const ph = 76;
  const pw = 82;
  body += `<rect x="${cx - pw - 8}" y="${y0}" width="${pw}" height="${ph}" rx="2"
    fill="${PAGE_FILL}" stroke="${MUTED}" stroke-width="1.4"/>`;
  body += `<rect x="${cx + 8}" y="${y0}" width="${pw}" height="${ph}" rx="2"
    fill="${PAPER}" stroke="${MUTED}" stroke-width="1.4"/>`;
  for (let i = 0; i < 6; i++) {
    const hy = y0 + 9 + i * ((ph - 18) / 5);
    if (wire) {
      body += `<path d="M ${cx - 7} ${hy} C ${cx - 7} ${hy - 6}, ${cx + 7} ${hy - 6}, ${cx + 7} ${hy}"
        fill="none" stroke="${METAL}" stroke-width="2.2"/>`;
    } else {
      body += `<path d="M ${cx - 9} ${hy} C ${cx - 2} ${hy - 8}, ${cx + 4} ${hy + 4}, ${cx + 9} ${hy - 3}"
        fill="none" stroke="${ACCENT}" stroke-width="2.2" stroke-linecap="round"/>`;
    }
  }
  body +=
    label(cx, 18, `${totalPages} pages on ${sheets} punched leaves`, "middle", 11, INK) +
    label(cx, y0 + ph + 24, "Opens completely flat — both pages stay readable", "middle", 10) +
    label(cx, H - 8, wire ? "Twin loops close through the punched holes" : "The coil threads through every hole", "middle", 10);
  return svg(W, H, body, "Finished punch-bound document lying flat");
}

// ---------------------------------------------------------------------------
// 6. Cover layout — panels, folds and guides drawn to scale
// ---------------------------------------------------------------------------

interface RectMm { x: number; y: number; width: number; height: number }

export interface CoverLayoutView {
  kind: string;
  total_width_mm: number;
  total_height_mm: number;
  spine_width_mm: number;
  trim_rect: RectMm;
  back_panel: RectMm | null;
  spine_panel: RectMm | null;
  front_panel: RectMm;
  back_flap: RectMm | null;
  front_flap: RectMm | null;
  safe_areas: RectMm[];
  barcode_rect: RectMm | null;
  fold_x_mm: number[];
  hinge_x_mm: number[];
  effective_dpi: number | null;
}

/** Scale drawing of the full cover artboard with every production guide. */
export function coverDiagram(l: CoverLayoutView): string {
  const MAXW = 460;
  const MAXH = 300;
  const pad = 26;
  const s = Math.min((MAXW - pad * 2) / l.total_width_mm, (MAXH - pad * 2) / l.total_height_mm);
  const W = l.total_width_mm * s + pad * 2;
  const H = l.total_height_mm * s + pad * 2 + 14;
  const ox = pad;
  const oy = pad;
  // Millimetre space is bottom-up; SVG is top-down.
  const X = (mm: number) => ox + mm * s;
  const Y = (mm: number, h = 0) => oy + (l.total_height_mm - mm - h) * s;

  const rect = (r: RectMm, fill: string, stroke: string, dash = "", width = 1) =>
    `<rect x="${X(r.x)}" y="${Y(r.y, r.height)}" width="${r.width * s}" height="${r.height * s}"
      fill="${fill}" stroke="${stroke}" stroke-width="${width}" ${dash ? `stroke-dasharray="${dash}"` : ""}/>`;

  let body = "";
  // Bleed artboard.
  body += `<rect x="${ox}" y="${oy}" width="${l.total_width_mm * s}" height="${l.total_height_mm * s}"
    fill="#fff8f4" stroke="${WARN}" stroke-width="1" stroke-dasharray="3 3"/>`;

  // Panels.
  if (l.back_flap) body += rect(l.back_flap, "#f6f7fc", MUTED);
  if (l.back_panel) body += rect(l.back_panel, PAGE_FILL, MUTED);
  if (l.spine_panel) body += rect(l.spine_panel, "#dfe6f8", MUTED);
  body += rect(l.front_panel, PAPER, MUTED);
  if (l.front_flap) body += rect(l.front_flap, "#f6f7fc", MUTED);

  // Trim line.
  body += rect(l.trim_rect, "none", INK, "", 1.2);

  // Safe areas.
  for (const a of l.safe_areas) body += rect(a, "none", "#22662c", "3 3", 0.8);

  // Hinge grooves.
  for (const hx of l.hinge_x_mm) {
    body += `<line x1="${X(hx)}" y1="${oy}" x2="${X(hx)}" y2="${oy + l.total_height_mm * s}"
      stroke="#7333a0" stroke-width="1" stroke-dasharray="1 2"/>`;
  }
  // Spine folds.
  for (const fx of l.fold_x_mm) {
    body += `<line x1="${X(fx)}" y1="${oy}" x2="${X(fx)}" y2="${oy + l.total_height_mm * s}"
      stroke="${ACCENT}" stroke-width="1.2" stroke-dasharray="4 3"/>`;
  }
  // Barcode reservation.
  if (l.barcode_rect) {
    body += rect(l.barcode_rect, "rgba(194,65,12,0.12)", WARN, "", 0.9);
    body += label(X(l.barcode_rect.x + l.barcode_rect.width / 2), Y(l.barcode_rect.y + l.barcode_rect.height / 2) + 3, "barcode", "middle", 7, WARN);
  }

  // Panel names.
  const name = (r: RectMm | null, text: string) =>
    r && r.width * s > 26 ? label(X(r.x + r.width / 2), Y(r.y + r.height) + 13, text, "middle", 8, INK) : "";
  body += name(l.back_flap, "FLAP");
  body += name(l.back_panel, "BACK");
  body += name(l.front_panel, "FRONT");
  body += name(l.front_flap, "FLAP");
  if (l.spine_panel && l.spine_panel.width * s > 16) {
    body += label(X(l.spine_panel.x + l.spine_panel.width / 2), Y(l.spine_panel.y + l.spine_panel.height) + 13, "SPINE", "middle", 7, ACCENT);
  }

  body += label(W / 2, H - 16,
    `Artboard ${l.total_width_mm.toFixed(1)} × ${l.total_height_mm.toFixed(1)} mm · trims to ${l.trim_rect.width.toFixed(1)} × ${l.trim_rect.height.toFixed(1)} mm`,
    "middle", 9, INK);
  body += label(W / 2, H - 4,
    l.spine_width_mm > 0 ? `Spine ${l.spine_width_mm.toFixed(2)} mm` : "Front panel only", "middle", 9);

  return svg(W, H, body, "Cover layout with trim, bleed, spine and safe-area guides");
}

/** eBook cover proportions with the safe title area indicated. */
export function ebookDiagram(pxWidth: number, pxHeight: number): string {
  const W = 300;
  const H = 250;
  const maxH = 190;
  const ratio = pxHeight / pxWidth;
  const h = Math.min(maxH, 150 * ratio);
  const w = h / ratio;
  const x = (W - w) / 2;
  const y = 22;

  let body = `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="2" fill="${PAGE_FILL}" stroke="${MUTED}" stroke-width="1.4"/>`;
  body += `<rect x="${x + w * 0.08}" y="${y + h * 0.06}" width="${w * 0.84}" height="${h * 0.88}"
    fill="none" stroke="#22662c" stroke-width="0.8" stroke-dasharray="3 3"/>`;
  // Suggested title and author blocks.
  body += `<rect x="${x + w * 0.15}" y="${y + h * 0.14}" width="${w * 0.7}" height="${h * 0.1}" rx="2" fill="${ACCENT}" opacity="0.28"/>`;
  body += `<rect x="${x + w * 0.25}" y="${y + h * 0.78}" width="${w * 0.5}" height="${h * 0.06}" rx="2" fill="${MUTED}" opacity="0.3"/>`;
  body += label(x + w / 2, y + h * 0.22, "title", "middle", 9, ACCENT);
  body += label(x + w / 2, y + h * 0.845, "author", "middle", 8);

  body += label(W / 2, 14, `${pxWidth} × ${pxHeight} px · 1:${ratio.toFixed(2)}`, "middle", 11, INK);
  body += label(W / 2, y + h + 18, "Keep text inside the dashed area — stores crop the edges", "middle", 9);
  body += label(W / 2, y + h + 32, "No bleed, no spine, no trim allowance", "middle", 9);
  return svg(W, H, body, "eBook cover proportions");
}
