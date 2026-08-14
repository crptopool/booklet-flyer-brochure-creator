import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { loadDocument, renderPage, unload as unloadPdf } from "./pdfRender";
import { paintBoundSpread, paintSheet } from "./preview";
import {
  bindingDiagram,
  boundSpreadDiagram,
  duplexFlipDiagram,
  gutterDiagram,
  pagesPerSheetDiagram,
  coverDiagram,
  ebookDiagram,
  resultDiagram,
  sheetSideDiagram,
} from "./diagrams";
import type { CoverLayoutView } from "./diagrams";

// ---------------------------------------------------------------------------
// Types mirroring the Rust backend
// ---------------------------------------------------------------------------

interface PaperSize {
  name: string;
  width_mm: number;
  height_mm: number;
}

interface PageInfo {
  number: number;
  width_pt: number;
  height_pt: number;
  rotation: number;
}

interface PdfSource {
  path: string;
  page_count: number;
  pages: PageInfo[];
  encrypted: boolean;
  modification_restricted: boolean;
  metadata: [string, string][];
}

interface SheetSpread {
  sheet_number: number;
  front: [number | null, number | null];
  back: [number | null, number | null];
}

interface Finding {
  severity: "INFO" | "WARNING" | "ERROR";
  code: string;
  message: string;
  page: number | null;
}

type Operation =
  | { type: "reorder_pages"; order: number[] }
  | { type: "rotate_page"; position: number; degrees: number }
  | { type: "delete_page"; position: number }
  | { type: "duplicate_page"; position: number }
  | { type: "insert_blank"; position: number; width_pt: number | null; height_pt: number | null };

interface VirtualPage {
  source_page: number | null;
  rotation: number;
  width_pt: number | null;
  height_pt: number | null;
}

type PageCountRule = "multiple_of_four" | "multiple_of_two" | "any";

interface BindingProfile {
  binding: string;
  key: string;
  name: string;
  description: string;
  mechanism: string;
  page_count_rule: PageCountRule;
  folded: boolean;
  folds_per_sheet: number;
  creep_applies: boolean;
  has_spine: boolean;
  punched: boolean;
  recommended_binding_margin_mm: number;
  allowed_sides: string[];
  min_pages: number;
  max_pages: number;
  requires_duplex: boolean;
  separate_cover: boolean;
  guidance: string;
  typical_use: string;
}

interface DuplexPlan {
  mode: string;
  flip_axis: string;
  back_side_rotation: number;
  back_side_inverted: boolean;
  is_recommended: boolean;
  explanation: string;
  manual_steps: string[];
}

interface PlanNote {
  severity: string;
  message: string;
}

interface SheetPlacement {
  page: number | null;
  x: number;
  y: number;
  width: number;
  height: number;
  rotation: number;
}

interface SheetSide {
  sheet_number: number;
  side: string;
  width: number;
  height: number;
  placements: SheetPlacement[];
  fold_x: number[];
}

interface BookletPlan {
  profile: BindingProfile;
  duplex: DuplexPlan;
  pages_per_side: number;
  pages_per_sheet: number;
  source_pages: number;
  blanks_needed: number;
  total_pages: number;
  sheet_count: number;
  folds_per_sheet: number;
  uses_printer_spreads: boolean;
  spine_width_mm: number | null;
  caliper_mm: number;
  notes: PlanNote[];
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

document.querySelectorAll<HTMLButtonElement>(".tab").forEach((tab) => {
  tab.addEventListener("click", () => {
    document.querySelectorAll(".tab").forEach((t) => t.classList.remove("active"));
    document.querySelectorAll(".panel").forEach((p) => p.classList.remove("active"));
    tab.classList.add("active");
    document.getElementById(`panel-${tab.dataset.tab}`)?.classList.add("active");
  });
});

function el<T extends HTMLElement>(id: string): T {
  return document.getElementById(id) as T;
}

const PT_PER_MM = 72 / 25.4;
const ptToMm = (pt: number) => pt / PT_PER_MM;

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

// ---------------------------------------------------------------------------
// PDF import panel (Phase 1 — non-destructive)
// ---------------------------------------------------------------------------

let source: PdfSource | null = null;
let operations: Operation[] = [];

async function refreshPages() {
  if (!source) return;
  const pages = await invoke<VirtualPage[]>("preview_operations", {
    source,
    operations,
  });  const grid = el<HTMLDivElement>("pdf-pages");
  grid.innerHTML = "";
  pages.forEach((vp, i) => {
    const div = document.createElement("div");
    div.className = "page-thumb" + (vp.source_page === null ? " blank" : "");
    const label = vp.source_page === null ? "Blank" : `Src ${vp.source_page}`;
    div.innerHTML = `
      <div class="thumb-body" style="transform: rotate(${vp.rotation}deg)">${label}</div>
      <div class="thumb-num">Page ${i + 1}</div>
      <div class="thumb-actions">
        <button title="Rotate 90°" data-act="rotate" data-pos="${i + 1}">⟳</button>
        <button title="Duplicate" data-act="dup" data-pos="${i + 1}">⧉</button>
        <button title="Delete" data-act="del" data-pos="${i + 1}">✕</button>
      </div>`;
    grid.appendChild(div);
  });
  // Replace the placeholders with the real page artwork.
  pages.forEach((vp, i) => {
    if (vp.source_page === null) return;
    const body = grid.children[i]?.querySelector<HTMLDivElement>(".thumb-body");
    if (!body) return;
    renderPage(vp.source_page, 100)
      .then((c) => {
        if (!c) return;
        body.textContent = "";
        body.appendChild(c);
      })
      .catch(() => {
        /* keep the placeholder if the page cannot be rasterised */
      });
  });
  grid.querySelectorAll<HTMLButtonElement>("button").forEach((btn) => {
    btn.addEventListener("click", () => {
      const pos = Number(btn.dataset.pos);
      if (btn.dataset.act === "rotate") operations.push({ type: "rotate_page", position: pos, degrees: 90 });
      if (btn.dataset.act === "dup") operations.push({ type: "duplicate_page", position: pos });
      if (btn.dataset.act === "del") operations.push({ type: "delete_page", position: pos });
      refreshPages().catch(showError);
    });
  });
  await refreshPreflight(pages);
}

async function refreshPreflight(pages: VirtualPage[]) {
  if (!source) return;
  // Preflight the *working* document state: map each virtual page back to
  // its source page info so counts, sizes and rotations reflect the
  // pending operations rather than the original file.
  const fallback = source.pages[0];
  const workingPages: PageInfo[] = pages.map((vp, i) => {
    const orig = vp.source_page !== null ? source!.pages[vp.source_page - 1] : undefined;
    return {
      number: i + 1,
      width_pt: vp.width_pt ?? orig?.width_pt ?? fallback.width_pt,
      height_pt: vp.height_pt ?? orig?.height_pt ?? fallback.height_pt,
      rotation: ((orig?.rotation ?? 0) + vp.rotation) % 360,
    };
  });
  const workingSource: PdfSource = {
    ...source,
    page_count: workingPages.length,
    pages: workingPages,
  };
  const findings = await invoke<Finding[]>("run_preflight", {
    source: workingSource,
    binding: "saddle_stitch",
    bleedMm: 3.0,
    expectedTrimMm: null,
  });
  const box = el<HTMLDivElement>("pdf-preflight");
  box.classList.remove("hidden");
  const rows = findings
    .map(
      (f) =>
        `<li class="sev-${escapeHtml(f.severity.toLowerCase())}"><strong>${escapeHtml(f.severity)}</strong> ${escapeHtml(f.message)}</li>`
    )
    .join("");
  box.innerHTML = `<h3>Preflight (saddle-stitch intent, ${workingPages.length} pages)</h3>` +
    (rows ? `<ul>${rows}</ul>` : "<p>No issues found.</p>");
}

function showError(e: unknown) {
  alert(typeof e === "string" ? e : String(e));
}

el<HTMLButtonElement>("btn-open-pdf").addEventListener("click", async () => {
  try {
    const path = await open({ filters: [{ name: "PDF", extensions: ["pdf"] }], multiple: false });
    if (!path) return;
    source = await invoke<PdfSource>("inspect_pdf", { path });
    operations = [];
    try {
      await loadDocument(String(path));
    } catch {
      unloadPdf(); // previews fall back to schematics
    }
    el<HTMLSpanElement>("pdf-name").textContent = String(path);
    const info = el<HTMLDivElement>("pdf-info");
    info.classList.remove("hidden");
    if (source.modification_restricted) {
      info.innerHTML = `<p class="sev-error">This PDF is encrypted/protected — modification is restricted.</p>`;
      return;
    }
    const first = source.pages[0];
    info.innerHTML = `
      <p><strong>${source.page_count}</strong> pages ·
      first page ${ptToMm(first.width_pt).toFixed(1)} × ${ptToMm(first.height_pt).toFixed(1)} mm</p>`;
    el<HTMLDivElement>("pdf-actions").classList.remove("hidden");
    await refreshPages();
  } catch (e) {
    showError(e);
  }
});

el<HTMLButtonElement>("btn-insert-blank").addEventListener("click", async () => {
  if (!source) return;
  const pages = await invoke<VirtualPage[]>("preview_operations", { source, operations });
  operations.push({ type: "insert_blank", position: pages.length + 1, width_pt: null, height_pt: null });
  refreshPages().catch(showError);
});

el<HTMLButtonElement>("btn-export-pdf").addEventListener("click", async () => {
  if (!source) return;
  try {
    const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (!out) return;
    const count = await invoke<number>("export_pdf", {
      sourcePath: source.path,
      operations,
      outputPath: out,
    });
    alert(`Exported ${count} pages to ${out}\nThe original file was not modified.`);
  } catch (e) {
    showError(e);
  }
});

// ---------------------------------------------------------------------------
// Booklet panel
// ---------------------------------------------------------------------------

async function populatePaperSizes() {
  const sizes = await invoke<PaperSize[]>("list_paper_sizes");
  for (const id of ["bk-trim", "bk-sheet"]) {
    const select = el<HTMLSelectElement>(id);
    select.innerHTML = sizes
      .map((s) => `<option value="${s.name}">${s.name} (${s.width_mm} × ${s.height_mm} mm)</option>`)
      .join("");
  }
  el<HTMLSelectElement>("bk-trim").value = "A5";
  el<HTMLSelectElement>("bk-sheet").value = "A4";
}

// --- Binding method selection ---------------------------------------------

let bindingProfiles: BindingProfile[] = [];
let selectedBinding = "saddle_stitch";
/** Set once the user edits the margin, so we stop overwriting their value. */
let marginOverridden = false;

const SIDE_LABELS: Record<string, string> = {
  left: "Left edge",
  right: "Right edge",
  top: "Top edge",
};

function currentProfile(): BindingProfile | undefined {
  return bindingProfiles.find((p) => p.key === selectedBinding);
}

function isDuplex(): boolean {
  return el<HTMLSelectElement>("bk-sides").value === "double";
}

function isLandscape(): boolean {
  return el<HTMLSelectElement>("bk-orientation").value === "landscape";
}

function duplexMode(): string {
  return isDuplex() ? el<HTMLSelectElement>("bk-flip").value : "simplex";
}

async function renderBindingMethods() {
  bindingProfiles = await invoke<BindingProfile[]>("list_booklet_bindings");
  const grid = el<HTMLDivElement>("binding-methods");
  grid.innerHTML = bindingProfiles
    .map(
      (p) => `
      <button type="button" class="method-card${p.key === selectedBinding ? " selected" : ""}"
              data-binding="${escapeHtml(p.key)}">
        <div class="method-art">${bindingDiagram(p.key)}</div>
        <div class="method-name">${escapeHtml(p.name)}</div>
        <div class="method-mech">${escapeHtml(p.mechanism)}</div>
      </button>`
    )
    .join("");
  grid.querySelectorAll<HTMLButtonElement>(".method-card").forEach((card) => {
    card.addEventListener("click", () => {
      selectedBinding = card.dataset.binding!;
      marginOverridden = false;
      renderBindingMethods().catch(showError);
    });
  });
  await applyBindingDefaults();
}

/** Push the selected method's rules into the form and describe it. */
async function applyBindingDefaults() {
  const p = currentProfile();
  if (!p) return;

  // Binding edge options are method-specific.
  const sideSelect = el<HTMLSelectElement>("bk-side");
  const previous = sideSelect.value;
  sideSelect.innerHTML = p.allowed_sides
    .map((s) => `<option value="${s}">${SIDE_LABELS[s] ?? s}</option>`)
    .join("");
  sideSelect.value = p.allowed_sides.includes(previous as never) ? previous : p.allowed_sides[0];

  if (!marginOverridden) {
    el<HTMLInputElement>("bk-margin").value = p.recommended_binding_margin_mm.toFixed(1);
  }

  // Folded bindings must be duplex; punched ones may be single-sided.
  const sides = el<HTMLSelectElement>("bk-sides");
  if (p.requires_duplex && sides.value === "single") sides.value = "double";

  const perSide = el<HTMLSelectElement>("bk-per-side");
  if (p.folded && perSide.value === "1") perSide.value = "2";
  if (!p.folded && perSide.value !== "1") perSide.value = "1";

  const ruleText =
    p.page_count_rule === "multiple_of_four"
      ? "must be a multiple of 4"
      : p.page_count_rule === "multiple_of_two"
        ? "must be a multiple of 2"
        : "any page count works";

  el<HTMLDivElement>("binding-detail").innerHTML = `
    <h3 style="margin-top:0">${escapeHtml(p.name)}</h3>
    <p>${escapeHtml(p.description)}</p>
    <ul class="spec-list">
      <li><strong>Page count</strong> ${escapeHtml(ruleText)}</li>
      <li><strong>Practical range</strong> ${p.min_pages}–${p.max_pages === 4294967295 ? "any" : p.max_pages} pages</li>
      <li><strong>Recommended gutter</strong> ${p.recommended_binding_margin_mm.toFixed(1)} mm</li>
      <li><strong>Printing</strong> ${p.requires_duplex ? "double-sided required" : "single- or double-sided"}</li>
      <li><strong>Spine</strong> ${p.has_spine ? "yes — width must be calculated" : "none"}</li>
      <li><strong>Opens flat</strong> ${p.punched ? "yes" : "no"}</li>
    </ul>
    <p class="hint"><strong>Why this matters:</strong> ${escapeHtml(p.guidance)}</p>
    <p class="hint"><strong>Typically used for:</strong> ${escapeHtml(p.typical_use)}</p>`;

  await renderConfigDiagrams();
}

/** Live sample images for the current configuration. */
async function renderConfigDiagrams() {
  const p = currentProfile();
  if (!p) return;

  const perSide = Number(el<HTMLSelectElement>("bk-per-side").value);
  const landscape = isLandscape();
  const mode = duplexMode();
  const margin = Number(el<HTMLInputElement>("bk-margin").value) || 0;
  const side = el<HTMLSelectElement>("bk-side").value || "left";
  const pages = Number(el<HTMLInputElement>("bk-pages").value) || 1;

  // The flip selector only applies to duplex jobs.
  el<HTMLSelectElement>("bk-flip").disabled = !isDuplex();

  let flip = { back_side_rotation: 0, is_recommended: true, explanation: "", manual_steps: [] as string[], flip_axis: "" };
  try {
    flip = await invoke<DuplexPlan>("get_duplex_plan", { mode, sheetIsLandscape: landscape });
  } catch {
    /* diagrams still render with the defaults above */
  }

  const folds = p.folded && isDuplex() ? (perSide === 4 ? 2 : perSide === 2 ? 1 : 0) : 0;
  const perSheet = perSide * (isDuplex() ? 2 : 1);
  const sheets = Math.ceil(pages / Math.max(1, perSheet));

  const card = (title: string, art: string, caption: string) => `
    <figure class="diagram">
      <figcaption class="diagram-title">${escapeHtml(title)}</figcaption>
      ${art}
      <p class="diagram-caption">${escapeHtml(caption)}</p>
    </figure>`;

  el<HTMLDivElement>("config-diagrams").innerHTML =
    card(
      `${perSide} page${perSide > 1 ? "s" : ""} per sheet side`,
      pagesPerSheetDiagram(perSide, folds, landscape),
      `Each sheet carries ${perSheet} page${perSheet > 1 ? "s" : ""} in total${isDuplex() ? " across both sides" : " on one side"}.`
    ) +
    card(
      isDuplex() ? "Duplex flip result" : "Single-sided printing",
      duplexFlipDiagram(mode, landscape, flip.back_side_rotation, flip.is_recommended),
      flip.explanation || "Only the front of each sheet is printed."
    ) +
    card(
      p.punched ? "Punch-safe zone" : "Binding gutter",
      gutterDiagram(margin, p.punched, side, p.key === "wire_o" ? "rect" : "round"),
      p.punched
        ? "Content inside this strip will be cut away by the punch."
        : "Content inside this strip is swallowed by the binding when the book is closed."
    ) +
    card(
      "After printing and binding",
      resultDiagram(p.key, sheets, pages),
      `${sheets} sheet${sheets === 1 ? "" : "s"} of paper produce the finished ${p.name.toLowerCase()} document.`
    ) +
    (flip.manual_steps.length
      ? `<div class="diagram manual-steps">
           <div class="diagram-title">Manual duplex — reinsertion steps</div>
           <ol>${flip.manual_steps.map((s) => `<li>${escapeHtml(s)}</li>`).join("")}</ol>
         </div>`
      : "");
}

// Any configuration change refreshes the sample images immediately.
for (const id of ["bk-per-side", "bk-sides", "bk-flip", "bk-orientation", "bk-side", "bk-pages"]) {
  el<HTMLElement>(id).addEventListener("change", () => {
    renderConfigDiagrams().catch(showError);
  });
}
el<HTMLInputElement>("bk-margin").addEventListener("input", () => {
  marginOverridden = true;
  renderConfigDiagrams().catch(showError);
});
el<HTMLSelectElement>("bk-sides").addEventListener("change", () => {
  applyBindingDefaults().catch(showError);
});

el<HTMLButtonElement>("btn-booklet").addEventListener("click", async () => {
  try {
    const pages = Number(el<HTMLInputElement>("bk-pages").value);
    const plan = await invoke<BookletPlan>("build_booklet_plan", {
      binding: selectedBinding,
      sourcePages: pages,
      pagesPerSide: Number(el<HTMLSelectElement>("bk-per-side").value),
      duplexMode: duplexMode(),
      sheetIsLandscape: isLandscape(),
      gsm: Number(el<HTMLInputElement>("bk-gsm").value),
    });

    const result = el<HTMLDivElement>("booklet-result");
    result.classList.remove("hidden");
    result.innerHTML = `
      <h3 style="margin-top:0">${escapeHtml(plan.profile.name)} — production plan</h3>
      <ul class="spec-list">
        <li><strong>Sheets of paper</strong> ${plan.sheet_count}</li>
        <li><strong>Pages per sheet</strong> ${plan.pages_per_sheet} (${plan.pages_per_side} per side${plan.pages_per_sheet > plan.pages_per_side ? ", both sides" : ", one side"})</li>
        <li><strong>Total pages</strong> ${plan.total_pages}${plan.blanks_needed ? ` (${plan.source_pages} supplied + ${plan.blanks_needed} blank)` : ""}</li>
        <li><strong>Folds per sheet</strong> ${plan.folds_per_sheet}</li>
        <li><strong>Duplex</strong> ${escapeHtml(plan.duplex.flip_axis)}${plan.duplex.back_side_inverted ? " — back sides rotated 180°" : ""}</li>
        ${plan.spine_width_mm !== null ? `<li><strong>Spine width</strong> ${plan.spine_width_mm.toFixed(2)} mm at ${plan.caliper_mm.toFixed(3)} mm caliper</li>` : ""}
      </ul>
      <ul class="findings">${plan.notes
        .map((n) => `<li class="sev-${escapeHtml(n.severity.toLowerCase())}"><strong>${escapeHtml(n.severity)}</strong> ${escapeHtml(n.message)}</li>`)
        .join("")}</ul>`;

    await refreshSimulation(plan);

    const spreads = await invoke<SheetSpread[]>("booklet_plan_spreads", { plan });
    const grid = el<HTMLDivElement>("booklet-spreads");
    grid.innerHTML = spreads.length
      ? spreads
          .map(
            (s) => `
      <div class="spread">
        <div class="spread-title">Sheet ${s.sheet_number}</div>
        <div class="spread-side"><span>Front</span> ${fmt(s.front[0])} | ${fmt(s.front[1])}</div>
        <div class="spread-side"><span>Back</span> ${fmt(s.back[0])} | ${fmt(s.back[1])}</div>
      </div>`
          )
          .join("")
      : `<p class="hint">Printer spreads are shown for the classic single-fold saddle-stitch layout
         (2 pages per side, double-sided). ${escapeHtml(plan.profile.name)} with this configuration keeps
         pages in normal reading order instead.</p>`;

    await renderConfigDiagrams();
  } catch (e) {
    showError(e);
  }
});

const fmt = (p: number | null) => (p === null ? "—" : String(p));

// ---------------------------------------------------------------------------
// Binding simulation + imposed export
// ---------------------------------------------------------------------------

let currentPlan: BookletPlan | null = null;
let currentSheets: SheetSide[] = [];
let readingOrder: (number | null)[] = [];
let simView: "bound" | "sheets" = "bound";
/** Guards against a slow paint overwriting a newer one. */
let simToken = 0;
let simIndex = 0;
/** Set when the sheets could not be built — export is then refused too. */
let sheetError = "";

/** Trim and sheet sizes in mm, honouring the orientation selector. */
async function currentSizes(): Promise<{ trim: [number, number]; sheet: [number, number] }> {
  const sizes = await invoke<PaperSize[]>("list_paper_sizes");
  const find = (name: string) => sizes.find((s) => s.name === name) ?? sizes[1];
  const trim = find(el<HTMLSelectElement>("bk-trim").value);
  const sheet = find(el<HTMLSelectElement>("bk-sheet").value);
  const landscape = isLandscape();
  return {
    trim: [trim.width_mm, trim.height_mm],
    // Only the printer sheet is turned; the trim page keeps its own shape.
    sheet: landscape ? [sheet.height_mm, sheet.width_mm] : [sheet.width_mm, sheet.height_mm],
  };
}

async function refreshSimulation(plan: BookletPlan) {
  currentPlan = plan;
  simIndex = 0;
  sheetError = "";
  el<HTMLElement>("sim-section").classList.remove("hidden");

  const { trim, sheet } = await currentSizes();
  try {
    currentSheets = await invoke<SheetSide[]>("plan_sheets", { plan, trimMm: trim, sheetMm: sheet });
  } catch (e) {
    currentSheets = [];
    sheetError = typeof e === "string" ? e : String(e);
  }
  readingOrder = await invoke<(number | null)[]>("bound_reading_order", { plan });

  // Tell the user which document the export will actually be built from.
  const hint = el<HTMLParagraphElement>("export-source-hint");
  hint.innerHTML = source
    ? `Built from <code>${escapeHtml(source.path)}</code> (${source.page_count} pages). Your original file is never modified.`
    : "Import a PDF on the <strong>Import PDF</strong> tab first — the imposed file is built from it. Your original file is never modified.";

  renderSimulation();
}

/** Spread `i` of a bound book: cover alone, then facing pairs. */
function spreadAt(i: number): { left: number | null; right: number | null; leftPos: number | null; rightPos: number | null } {
  if (i === 0) {
    return { left: null, right: readingOrder[0] ?? null, leftPos: null, rightPos: 1 };
  }
  const leftPos = i * 2;
  const rightPos = i * 2 + 1;
  return {
    left: readingOrder[leftPos - 1] ?? null,
    right: rightPos <= readingOrder.length ? (readingOrder[rightPos - 1] ?? null) : null,
    leftPos,
    rightPos: rightPos <= readingOrder.length ? rightPos : null,
  };
}

function spreadCount(): number {
  return Math.max(1, Math.ceil((readingOrder.length + 1) / 2));
}

function renderSimulation() {
  const stage = el<HTMLDivElement>("sim-stage");
  const pos = el<HTMLSpanElement>("sim-pos");
  if (!currentPlan) return;

  if (simView === "sheets") {
    if (sheetError) {
      stage.innerHTML = `<p class="sev-warning">${escapeHtml(sheetError)}</p>`;
      pos.textContent = "";
      return;
    }
    const total = currentSheets.length;
    simIndex = Math.min(simIndex, total - 1);
    const side = currentSheets[simIndex];
    stage.innerHTML = sheetSideDiagram(side, el<HTMLInputElement>("sim-marks").checked);
    pos.textContent = `Sheet side ${simIndex + 1} of ${total}`;
    // Swap in the artwork-composited canvas once it is ready.
    const token = ++simToken;
    paintSheet(side, el<HTMLInputElement>("sim-marks").checked)
      .then((c) => {
        if (token !== simToken) return;
        stage.innerHTML = "";
        stage.appendChild(c);
      })
      .catch(() => {
        /* the schematic already rendered */
      });
    return;
  }

  const total = spreadCount();
  simIndex = Math.min(simIndex, total - 1);
  const s = spreadAt(simIndex);
  stage.innerHTML = boundSpreadDiagram(
    currentPlan.profile.key,
    s.left,
    s.right,
    s.leftPos,
    s.rightPos,
    Number(el<HTMLInputElement>("bk-margin").value) || 0,
    el<HTMLSelectElement>("bk-side").value || "left"
  );
  pos.textContent = `Spread ${simIndex + 1} of ${total} · ${readingOrder.length} bound pages`;

  const token = ++simToken;
  const first = currentSheets[0]?.placements[0];
  const aspect = first && first.width > 0 ? first.height / first.width : 210 / 148;
  paintBoundSpread(
    currentPlan.profile.key, s.left, s.right, s.leftPos, s.rightPos, aspect,
    el<HTMLSelectElement>("bk-side").value || "left"
  )
    .then((c) => {
      if (token !== simToken) return;
      stage.innerHTML = "";
      stage.appendChild(c);
    })
    .catch(() => {
      /* the schematic already rendered */
    });
}

document.querySelectorAll<HTMLButtonElement>("#sim-tabs .seg-btn").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll("#sim-tabs .seg-btn").forEach((b) => b.classList.remove("active"));
    btn.classList.add("active");
    simView = btn.dataset.view as "bound" | "sheets";
    simIndex = 0;
    renderSimulation();
  });
});

el<HTMLButtonElement>("sim-prev").addEventListener("click", () => {
  simIndex = Math.max(0, simIndex - 1);
  renderSimulation();
});
el<HTMLButtonElement>("sim-next").addEventListener("click", () => {
  const total = simView === "sheets" ? currentSheets.length : spreadCount();
  simIndex = Math.min(total - 1, simIndex + 1);
  renderSimulation();
});
el<HTMLInputElement>("sim-marks").addEventListener("change", renderSimulation);

/** Export the imposed PDF; optionally hand it to the OS for printing. */
async function exportImposed(openAfter: boolean) {
  if (!currentPlan) {
    showError("Calculate the booklet plan first.");
    return;
  }
  if (!source) {
    showError("Import a PDF on the Import PDF tab first — the imposed file is built from it.");
    return;
  }
  if (sheetError) {
    showError(sheetError);
    return;
  }
  const out = await save({
    filters: [{ name: "PDF", extensions: ["pdf"] }],
    defaultPath: `imposed-${currentPlan.profile.key}.pdf`,
  });
  if (!out) return;

  const { trim, sheet } = await currentSizes();
  const marks = el<HTMLInputElement>("sim-marks").checked;
  const count = await invoke<number>("export_imposed_pdf", {
    sourcePath: source.path,
    plan: currentPlan,
    trimMm: trim,
    sheetMm: sheet,
    outputPath: out,
    marks: { crop_marks: marks, fold_marks: marks, sheet_labels: marks },
  });

  const box = el<HTMLDivElement>("export-result");
  box.classList.remove("hidden");
  box.innerHTML = `
    <p class="sev-info">✓ Wrote <strong>${count}</strong> sheet side${count === 1 ? "" : "s"} to
      <code>${escapeHtml(out)}</code>, arranged for ${escapeHtml(currentPlan.profile.name.toLowerCase())}.</p>
    <p class="hint">The original file was not modified. Print this file at
      <strong>100% / actual size</strong> with scaling turned off, and set your printer to
      <strong>${escapeHtml(currentPlan.duplex.flip_axis)}</strong>${currentPlan.pages_per_sheet > currentPlan.pages_per_side ? " for double-sided output" : " (single-sided)"}.</p>`;

  if (openAfter) {
    try {
      await openPath(out);
    } catch {
      box.innerHTML += `<p class="hint">Could not open the file automatically — open it from ${escapeHtml(out)}.</p>`;
    }
  }
}

el<HTMLButtonElement>("btn-export-imposed").addEventListener("click", () => {
  exportImposed(false).catch(showError);
});
el<HTMLButtonElement>("btn-print-imposed").addEventListener("click", () => {
  exportImposed(true).catch(showError);
});

// ---------------------------------------------------------------------------
// N-Up panel
// ---------------------------------------------------------------------------

el<HTMLButtonElement>("btn-nup").addEventListener("click", async () => {
  try {
    const mode = el<HTMLSelectElement>("nup-mode").value;
    const pages = Number(el<HTMLInputElement>("nup-pages").value);
    const rows = Number(el<HTMLInputElement>("nup-rows").value);
    const cols = Number(el<HTMLInputElement>("nup-cols").value);
    const duplex = el<HTMLSelectElement>("nup-duplex").value === "true";
    const result = el<HTMLDivElement>("nup-result");
    const grid = el<HTMLDivElement>("nup-sheets");
    result.classList.remove("hidden");
    grid.innerHTML = "";

    if (mode === "repeat") {
      const sheets = Math.ceil(pages / (rows * cols));
      result.innerHTML = `<p><strong>${rows * cols}</strong> copies per sheet → <strong>${sheets}</strong> sheets for ${pages} copies.</p>`;
      return;
    }

    const sides =
      mode === "cutstack"
        ? await invoke<(number | null)[][]>("cut_and_stack_sequence", { pageCount: pages, rows, cols })
        : await invoke<(number | null)[][]>("nup_sequence", { pageCount: pages, rows, cols, duplex });
    const sheetCount = await invoke<number>("nup_sheet_count", {
      pageCount: pages,
      pagesPerSheet: rows * cols,
      duplex: mode === "cutstack" ? false : duplex,
    });
    result.innerHTML = `<p><strong>${sheetCount}</strong> physical sheets (${rows} × ${cols} per side${duplex && mode !== "cutstack" ? ", duplex" : ""}).</p>`;
    grid.innerHTML = sides
      .slice(0, 24)
      .map(
        (side, i) => `
      <div class="spread">
        <div class="spread-title">${mode === "cutstack" ? "Sheet" : duplex ? (i % 2 === 0 ? `Sheet ${Math.floor(i / 2) + 1} front` : `Sheet ${Math.floor(i / 2) + 1} back`) : `Sheet ${i + 1}`}</div>
        <div class="nup-grid" style="grid-template-columns: repeat(${cols}, 1fr)">
          ${side.map((p) => `<div class="nup-cell${p === null ? " blank" : ""}">${fmt(p)}</div>`).join("")}
        </div>
      </div>`
      )
      .join("") + (sides.length > 24 ? `<p class="hint">Showing first 24 of ${sides.length} sides…</p>` : "");
  } catch (e) {
    showError(e);
  }
});

// ---------------------------------------------------------------------------
// Binding panel
// ---------------------------------------------------------------------------

el<HTMLButtonElement>("btn-binding").addEventListener("click", async () => {
  try {
    const binding = el<HTMLSelectElement>("bind-type").value;
    const pages = Number(el<HTMLInputElement>("bind-pages").value);
    const gsm = Number(el<HTMLInputElement>("bind-gsm").value);

    const margin = await invoke<number>("recommended_binding_margin_mm", { binding });
    const gsmText = await invoke<string>("describe_gsm", { gsm });
    const caliper = await invoke<number>("caliper_from_gsm", { gsm, bulkFactor: null });

    let html = `
      <p><strong>Paper:</strong> ${gsm} GSM — ${gsmText}. Approx. caliper
      ${caliper.toFixed(3)} mm <em>(actual caliper varies by manufacturer and finish — override if you have printer specs)</em>.</p>
      <p><strong>Recommended binding margin:</strong> ${margin.toFixed(1)} mm (you may override).</p>`;

    if (binding === "perfect" || binding === "hardcover") {
      const spine = await invoke<number>("spine_width", { pageCount: pages, caliperMm: caliper });
      html += `<p><strong>Spine width:</strong> ${pages} pages ÷ 2 × ${caliper.toFixed(3)} mm = <strong>${spine.toFixed(2)} mm</strong>.</p>`;
    }
    if (binding === "saddle_stitch" || binding === "staple") {
      const sheets = await invoke<number>("booklet_sheet_count", { pageCount: pages });
      const creep = await invoke<{ total_creep_mm: number }>("creep", {
        sheetCount: sheets,
        caliperMm: caliper,
        foldCount: 1,
        maxCreepMm: null,
        mode: "automatic",
        customTotalMm: null,
      });
      html += `<p><strong>Creep:</strong> ${sheets} nested sheets → total creep ≈ <strong>${creep.total_creep_mm.toFixed(2)} mm</strong> at the innermost sheet. Compensation can be applied at imposition.</p>`;
    }

    const result = el<HTMLDivElement>("binding-result");
    result.classList.remove("hidden");
    result.innerHTML = html;
  } catch (e) {
    showError(e);
  }
});

populatePaperSizes().catch(() => {
  /* outside Tauri (plain browser) the invoke calls are unavailable */
});

renderBindingMethods().catch(() => {
  /* same — the panel stays empty when opened outside the desktop shell */
});

// ---------------------------------------------------------------------------
// Cover creator (Phase 7)
// ---------------------------------------------------------------------------

interface CoverInputs {
  kind: string;
  trim_width_mm: number;
  trim_height_mm: number;
  page_count: number;
  caliper_mm: number;
  bleed_mm: number;
  safe_margin_mm: number;
  board_overhang_mm: number;
  hinge_mm: number;
  turn_in_mm: number;
  flap_mm: number;
  barcode: boolean;
  pixel_width: number;
  pixel_height: number;
}

interface CoverNote {
  severity: string;
  message: string;
}

type CoverLayout = CoverLayoutView & { notes: CoverNote[] };

let coverLayout: CoverLayout | null = null;

const numVal = (id: string) => Number(el<HTMLInputElement>(id).value) || 0;

function coverKind(): string {
  return el<HTMLSelectElement>("cv-kind").value;
}

function collectCoverInputs(): CoverInputs {
  const kind = coverKind();
  return {
    kind,
    trim_width_mm: kind === "ebook" ? numVal("cv-print-w") : numVal("cv-trim-w"),
    trim_height_mm: numVal("cv-trim-h"),
    page_count: Math.max(1, numVal("cv-pages")),
    caliper_mm: numVal("cv-caliper"),
    bleed_mm: numVal("cv-bleed"),
    safe_margin_mm: numVal("cv-safe"),
    board_overhang_mm: numVal("cv-overhang"),
    hinge_mm: numVal("cv-hinge"),
    turn_in_mm: numVal("cv-turnin"),
    flap_mm: numVal("cv-flap"),
    barcode: el<HTMLInputElement>("cv-barcode").checked,
    pixel_width: Math.max(1, numVal("cv-px-w")),
    pixel_height: Math.max(1, numVal("cv-px-h")),
  };
}

async function refreshCover() {
  const kind = coverKind();
  const ebook = kind === "ebook";
  el<HTMLElement>("cv-print-fields").classList.toggle("hidden", ebook);
  el<HTMLElement>("cv-ebook-fields").classList.toggle("hidden", !ebook);
  el<HTMLElement>("cv-case-fields").classList.toggle("hidden", kind === "paperback");
  el<HTMLButtonElement>("btn-cover-png").classList.toggle("hidden", !ebook);
  el<HTMLButtonElement>("btn-cover-pdf").classList.toggle("hidden", ebook);

  try {
    coverLayout = await invoke<CoverLayout>("build_cover_layout", { input: collectCoverInputs() });
  } catch (e) {
    el<HTMLDivElement>("cover-preview").innerHTML =
      `<p class="sev-error">${escapeHtml(typeof e === "string" ? e : String(e))}</p>`;
    return;
  }

  el<HTMLDivElement>("cover-preview").innerHTML = ebook
    ? ebookDiagram(coverLayout.total_width_mm, coverLayout.total_height_mm)
    : coverDiagram(coverLayout);

  const notes = el<HTMLDivElement>("cover-notes");
  notes.classList.remove("hidden");
  notes.innerHTML = `<ul class="findings">${coverLayout.notes
    .map((n) => `<li class="sev-${escapeHtml(n.severity.toLowerCase())}"><strong>${escapeHtml(n.severity)}</strong> ${escapeHtml(n.message)}</li>`)
    .join("")}</ul>`;
}

for (const id of [
  "cv-kind", "cv-trim-w", "cv-trim-h", "cv-pages", "cv-caliper", "cv-bleed", "cv-safe",
  "cv-overhang", "cv-hinge", "cv-turnin", "cv-flap", "cv-barcode", "cv-px-w", "cv-px-h", "cv-print-w",
]) {
  el<HTMLElement>(id).addEventListener("change", () => refreshCover().catch(showError));
  el<HTMLElement>(id).addEventListener("input", () => refreshCover().catch(showError));
}

el<HTMLSelectElement>("cv-preset").addEventListener("change", () => {
  const v = el<HTMLSelectElement>("cv-preset").value;
  if (v === "custom") return;
  const [w, h] = v.split("x");
  el<HTMLInputElement>("cv-px-w").value = w;
  el<HTMLInputElement>("cv-px-h").value = h;
  refreshCover().catch(showError);
});

el<HTMLButtonElement>("btn-cover-pdf").addEventListener("click", async () => {
  if (!coverLayout) return;
  try {
    const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }], defaultPath: `cover-${coverKind()}.pdf` });
    if (!out) return;
    const [w, h] = await invoke<[number, number]>("export_cover_pdf", {
      layout: coverLayout,
      outputPath: out,
      title: "PrintPrep cover",
    });
    const box = el<HTMLDivElement>("cover-result");
    box.classList.remove("hidden");
    box.innerHTML = `<p class="sev-info">✓ Wrote a ${(w / 2.8346).toFixed(1)} × ${(h / 2.8346).toFixed(1)} mm
      cover template to <code>${escapeHtml(out)}</code>, with trim and bleed boxes set.</p>
      <p class="hint">Place your artwork behind the guides. The spine is
      ${coverLayout.spine_width_mm.toFixed(2)} mm — confirm the caliper with your printer before going to press.</p>`;
  } catch (e) {
    showError(e);
  }
});

/** eBook covers are screen artwork, so they export as a PNG. */
el<HTMLButtonElement>("btn-cover-png").addEventListener("click", async () => {
  if (!coverLayout) return;
  try {
    const out = await save({ filters: [{ name: "PNG image", extensions: ["png"] }], defaultPath: "ebook-cover.png" });
    if (!out) return;
    const w = Math.round(coverLayout.total_width_mm);
    const h = Math.round(coverLayout.total_height_mm);
    const canvas = document.createElement("canvas");
    canvas.width = w;
    canvas.height = h;
    const ctx = canvas.getContext("2d")!;
    // A neutral template with the safe area marked, ready for artwork.
    ctx.fillStyle = "#eef1fa";
    ctx.fillRect(0, 0, w, h);
    ctx.strokeStyle = "#22662c";
    ctx.setLineDash([Math.max(4, w / 120), Math.max(4, w / 120)]);
    ctx.lineWidth = Math.max(2, w / 400);
    ctx.strokeRect(w * 0.08, h * 0.06, w * 0.84, h * 0.88);
    ctx.setLineDash([]);
    ctx.fillStyle = "#7c85a3";
    ctx.font = `${Math.round(w / 28)}px Helvetica, Arial, sans-serif`;
    ctx.textAlign = "center";
    ctx.fillText(`${w} × ${h}`, w / 2, h / 2);
    ctx.font = `${Math.round(w / 44)}px Helvetica, Arial, sans-serif`;
    ctx.fillText("keep text inside the dashed safe area", w / 2, h / 2 + w / 20);

    const blob: Blob = await new Promise((res, rej) =>
      canvas.toBlob((b) => (b ? res(b) : rej(new Error("Could not encode the PNG."))), "image/png")
    );
    const bytes = Array.from(new Uint8Array(await blob.arrayBuffer()));
    await invoke<number>("write_bytes", { path: out, bytes });
    const box = el<HTMLDivElement>("cover-result");
    box.classList.remove("hidden");
    box.innerHTML = `<p class="sev-info">✓ Wrote a ${w} × ${h} px cover template to <code>${escapeHtml(out)}</code>.</p>`;
  } catch (e) {
    showError(e);
  }
});

refreshCover().catch(() => {
  /* outside Tauri the invoke calls are unavailable */
});
