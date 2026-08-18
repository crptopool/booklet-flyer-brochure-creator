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
  finishingDiagram,
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
  fold_y?: number[];
  stock: string;
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
  separate_cover: boolean;
  cover_pages: number;
  text_pages: number;
  text_sheet_count: number;
  cover_sheet_count: number;
  cover_gsm: number | null;
  spine_width_mm: number | null;
  caliper_mm: number;
  notes: PlanNote[];
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

/** Show one screen and mark its menu entry active. */
function showPanel(name: string) {
  document.querySelectorAll(".nav-item").forEach((t) => t.classList.remove("active"));
  document.querySelectorAll(".panel").forEach((p) => p.classList.remove("active"));
  document.querySelector(`.nav-item[data-tab="${name}"]`)?.classList.add("active");
  document.getElementById(`panel-${name}`)?.classList.add("active");
  try {
    localStorage.setItem("printprep.panel", name);
  } catch {
    /* storage is optional */
  }
  window.scrollTo({ top: 0 });
}

document.querySelectorAll<HTMLButtonElement>(".nav-item").forEach((item) => {
  item.addEventListener("click", () => showPanel(item.dataset.tab!));
});

// Menu groups fold away individually.
document.querySelectorAll<HTMLButtonElement>(".group-head").forEach((head) => {
  head.addEventListener("click", () => {
    const group = head.parentElement!;
    const collapsed = group.classList.toggle("collapsed");
    head.setAttribute("aria-expanded", String(!collapsed));
    try {
      localStorage.setItem(`printprep.group.${group.getAttribute("data-group")}`, String(collapsed));
    } catch {
      /* storage is optional */
    }
  });
});

/** Collapse the whole sidebar to an icon rail. */
function setSidebarCollapsed(collapsed: boolean) {
  document.body.classList.toggle("collapsed-sidebar", collapsed);
  const btn = el<HTMLButtonElement>("sidebar-toggle");
  btn.setAttribute("aria-expanded", String(!collapsed));
  btn.title = collapsed ? "Expand menu (Ctrl+B)" : "Collapse menu (Ctrl+B)";
  try {
    localStorage.setItem("printprep.sidebar", collapsed ? "collapsed" : "open");
  } catch {
    /* storage is optional */
  }
}

el<HTMLButtonElement>("sidebar-toggle").addEventListener("click", () => {
  setSidebarCollapsed(!document.body.classList.contains("collapsed-sidebar"));
});

document.addEventListener("keydown", (e) => {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "b") {
    e.preventDefault();
    setSidebarCollapsed(!document.body.classList.contains("collapsed-sidebar"));
  }
});

// Restore the previous session's menu state.
try {
  setSidebarCollapsed(localStorage.getItem("printprep.sidebar") === "collapsed");
  document.querySelectorAll<HTMLElement>(".menu-group").forEach((group) => {
    if (localStorage.getItem(`printprep.group.${group.getAttribute("data-group")}`) === "true") {
      group.classList.add("collapsed");
      group.querySelector(".group-head")?.setAttribute("aria-expanded", "false");
    }
  });
  const last = localStorage.getItem("printprep.panel");
  if (last && document.getElementById(`panel-${last}`)) showPanel(last);
} catch {
  /* storage is optional */
}

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

/** Fields that answer "how many pages does the document have?". */
const PAGE_COUNT_FIELDS = ["bk-pages", "nup-pages", "cv-pages"] as const;
/** Fields the user has typed into themselves, which the document must not stamp over. */
const pageCountTyped = new Set<string>();
/** Page count of the loaded document *after* the pending page operations. */
let documentPageCount: number | null = null;

/**
 * Push the loaded document's page count into the planning screens.
 *
 * The page count is a fact about the file, not a preference. Without this the
 * screens keep their starting values, so opening a 48-page document still
 * plans, simulates and exports a 20-page booklet — silently describing a
 * document that does not exist. A value the user typed themselves is left
 * alone, and the mismatch is called out instead.
 */
function syncPageCountFromDocument(count: number) {
  documentPageCount = count;
  for (const id of PAGE_COUNT_FIELDS) {
    if (pageCountTyped.has(id)) continue;
    const field = el<HTMLInputElement>(id);
    if (Number(field.value) === count) continue;
    field.value = String(count);
    // Programmatic "change" — the typed-override flag hangs off "input".
    field.dispatchEvent(new Event("change"));
  }
  showPageCountNotice();
  // A plan already on screen was built for the old count, so it now
  // contradicts the fields above it. Rebuild rather than leave it stale.
  if (currentPlan) buildAndShowBookletPlan().catch(showError);
}

/** Say which page count the plan is using, and flag it when it isn't the document's. */
function showPageCountNotice() {
  const box = el<HTMLParagraphElement>("bk-pages-sync");
  if (documentPageCount === null) {
    box.classList.add("hidden");
    return;
  }
  const typed = Number(el<HTMLInputElement>("bk-pages").value) || 0;
  box.classList.remove("hidden");
  box.className =
    typed === documentPageCount ? "doc-sync" : "doc-sync doc-sync-mismatch";
  box.innerHTML =
    typed === documentPageCount
      ? `Planning the loaded document: <strong>${documentPageCount}</strong> pages.`
      : `The loaded document has <strong>${documentPageCount}</strong> pages, but this plan is
         being built for <strong>${typed}</strong>.
         <button type="button" id="bk-pages-use-doc">Use ${documentPageCount}</button>`;
}

document.addEventListener("click", (e) => {
  if ((e.target as HTMLElement)?.id !== "bk-pages-use-doc") return;
  if (documentPageCount === null) return;
  pageCountTyped.clear();
  syncPageCountFromDocument(documentPageCount);
});

for (const id of PAGE_COUNT_FIELDS) {
  el<HTMLInputElement>(id).addEventListener("input", () => {
    pageCountTyped.add(id);
    if (id === "bk-pages") showPageCountNotice();
  });
}

async function refreshPages() {
  if (!source) return;
  const pages = await invoke<VirtualPage[]>("preview_operations", {
    source,
    operations,
  });
  // Insertions and deletions change the document, so the planning screens
  // follow the working page count, not the count the file arrived with.
  syncPageCountFromDocument(pages.length);
  const grid = el<HTMLDivElement>("pdf-pages");
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
    // A new file supersedes counts typed for the previous one.
    pageCountTyped.clear();
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
  for (const id of ["bk-trim", "bk-sheet", "bk-cover-sheet"]) {
    const select = el<HTMLSelectElement>(id);
    select.innerHTML = sizes
      .map((s) => `<option value="${s.name}">${s.name} (${s.width_mm} × ${s.height_mm} mm)</option>`)
      .join("");
  }
  el<HTMLSelectElement>("bk-trim").value = "A5";
  el<HTMLSelectElement>("bk-sheet").value = "A4";
  el<HTMLSelectElement>("bk-cover-sheet").value = "A5";
}

/**
 * Whether the cover is a separate wrap, and on what paper.
 *
 * `null` means "no separate cover" — the outermost sheet of the text stock
 * carries the cover pages, which is how a self-cover booklet has always
 * worked here. This is the one place that decision is made, so the plan,
 * the simulation and the export all read it from here and cannot disagree.
 */
async function coverStock(): Promise<{ gsm: number; sheetMm: [number, number] } | null> {
  if (!el<HTMLInputElement>("bk-cover-sep").checked) return null;
  const sizes = await invoke<PaperSize[]>("list_paper_sizes");
  const chosen = sizes.find((s) => s.name === el<HTMLSelectElement>("bk-cover-sheet").value) ?? sizes[1];
  // The cover sheet follows the same orientation choice as the text sheet —
  // a landscape job wraps in a landscape cover.
  const landscape = isLandscape();
  return {
    gsm: Number(el<HTMLInputElement>("bk-cover-gsm").value) || 200,
    sheetMm: landscape ? [chosen.height_mm, chosen.width_mm] : [chosen.width_mm, chosen.height_mm],
  };
}

el<HTMLInputElement>("bk-cover-sep").addEventListener("change", () => {
  const on = el<HTMLInputElement>("bk-cover-sep").checked;
  el<HTMLSelectElement>("bk-cover-sheet").disabled = !on;
  el<HTMLInputElement>("bk-cover-gsm").disabled = !on;
  replanIfShowing();
});
for (const id of ["bk-cover-sheet", "bk-cover-gsm"]) {
  el<HTMLElement>(id).addEventListener("change", replanIfShowing);
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
      replanIfShowing();
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
  await refreshSheetCapacity();

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

interface SheetCapacity {
  fit_rows: number;
  fit_cols: number;
  options: number[];
  rows: number | null;
  cols: number | null;
  turned: boolean;
  folds: string[];
}

/** What the currently chosen papers can hold, or null while they cannot. */
let capacity: SheetCapacity | null = null;

/**
 * Offer only the pages-per-side counts the chosen paper can actually hold.
 *
 * The imposition is generated from the sheet and trim sizes, so the menu has
 * to come from the same place — otherwise the user can pick 8 pages a side on
 * A4 and only find out it was impossible at the export step.
 */
async function refreshSheetCapacity() {
  const perSide = el<HTMLSelectElement>("bk-per-side");
  const wanted = Number(perSide.value) || 1;
  const note = el<HTMLParagraphElement>("bk-capacity");
  let sizes: { trim: [number, number]; sheet: [number, number] };
  try {
    sizes = await currentSizes();
  } catch {
    return;
  }

  try {
    capacity = await invoke<SheetCapacity>("sheet_capacity", {
      trimMm: sizes.trim,
      sheetMm: sizes.sheet,
      pagesPerSide: wanted,
    });
  } catch (e) {
    capacity = null;
    note.className = "doc-sync doc-sync-mismatch";
    note.classList.remove("hidden");
    note.textContent = typeof e === "string" ? e : String(e);
    return;
  }

  const folded = currentProfile()?.folded ?? false;
  // Unfolded work is stacked and cut, so it is not limited to powers of two.
  const options = folded ? capacity.options : [1, 2, 4].filter((n) => n <= capacity!.fit_rows * capacity!.fit_cols);
  const keep = options.includes(wanted) ? wanted : options[options.length - 1] ?? 1;
  perSide.innerHTML = options
    .map((n) => `<option value="${n}"${n === keep ? " selected" : ""}>${n} page${n > 1 ? "s" : ""} per side</option>`)
    .join("");
  perSide.value = String(keep);

  const { fit_rows: fr, fit_cols: fc } = capacity;
  note.className = "doc-sync";
  note.classList.remove("hidden");
  const grid =
    capacity.rows && capacity.cols
      ? ` · imposed ${capacity.rows} × ${capacity.cols}${capacity.turned ? ", pages turned 90° to fit" : ""}`
      : "";
  const folds = capacity.folds.length
    ? ` · fold ${capacity.folds.join(", then ")} (the last fold makes the spine)`
    : " · no folds";
  // The upright fit is the headline number, but a turned grid can hold more.
  const most = capacity.options[capacity.options.length - 1] ?? fr * fc;
  const upright = `${fr} × ${fc} = ${fr * fc} page${fr * fc === 1 ? "" : "s"} a side upright`;
  const extra = most > fr * fc ? `, up to ${most} with the pages turned` : "";
  note.textContent = `This sheet holds ${upright}${extra}${grid}` + (folded ? folds : "");
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

  // The fold count comes from the layout engine, not a guess here, so the
  // diagram cannot disagree with the imposition.
  const folds = p.folded && isDuplex() ? (capacity?.folds.length ?? 0) : 0;
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
    card(
      "Cutting and stapling",
      finishingDiagram(p.key, folds, side),
      p.folded
        ? folds >= 2
          ? "Staples go through the spine fold. The head is closed by the second fold and has to be cut off, along with the fore-edge and foot."
          : "Staples go through the spine fold. Only the fore-edge is trimmed — the head and foot are already cut edges of the sheet."
        : p.punched
          ? "Trim the stack to its final size before punching, or the holes will sit at different distances from the edge."
          : "The spine is glued and never cut. Trim the other three edges through the whole stack in one pass."
    ) +
    (flip.manual_steps.length
      ? `<div class="diagram manual-steps">
           <div class="diagram-title">Manual duplex — reinsertion steps</div>
           <ol>${flip.manual_steps.map((s) => `<li>${escapeHtml(s)}</li>`).join("")}</ol>
         </div>`
      : "");
}

/**
 * Rebuild a plan that is already on screen.
 *
 * The plan is a snapshot of the settings at the moment it was calculated.
 * Left alone it goes stale the instant anything above it changes, and then
 * the screen contradicts itself: the diagrams show 4 pages per side while
 * the plan below still reports the 2-per-side sheet count. Debounced so
 * typing in a number field does not fire a rebuild per keystroke.
 */
let replanTimer: number | undefined;
function replanIfShowing() {
  if (!currentPlan) return;
  window.clearTimeout(replanTimer);
  replanTimer = window.setTimeout(() => {
    buildAndShowBookletPlan().catch(showError);
  }, 150);
}

// Any configuration change refreshes the sample images immediately.
for (const id of ["bk-per-side", "bk-sides", "bk-flip", "bk-orientation", "bk-side", "bk-pages"]) {
  el<HTMLElement>(id).addEventListener("change", () => {
    renderConfigDiagrams().catch(showError);
    replanIfShowing();
  });
}
// These do not feed the diagrams, but they do feed the plan and the sheets.
for (const id of ["bk-trim", "bk-sheet", "bk-gsm"]) {
  el<HTMLElement>(id).addEventListener("change", replanIfShowing);
}
// The paper decides what the sheet can hold, and the chosen count decides
// how it is folded, so all four rebuild the note and the diagrams.
for (const id of ["bk-trim", "bk-sheet", "bk-orientation", "bk-per-side"]) {
  el<HTMLElement>(id).addEventListener("change", () => {
    refreshSheetCapacity()
      .then(() => renderConfigDiagrams())
      .catch(showError);
  });
}
el<HTMLInputElement>("bk-margin").addEventListener("input", () => {
  marginOverridden = true;
  renderConfigDiagrams().catch(showError);
});
el<HTMLSelectElement>("bk-sides").addEventListener("change", () => {
  applyBindingDefaults().catch(showError);
});

el<HTMLButtonElement>("btn-booklet").addEventListener("click", () => {
  buildAndShowBookletPlan().catch(showError);
});

/** Build the production plan from the form and paint every panel that depends on it. */
async function buildAndShowBookletPlan() {
  {
    const pages = Number(el<HTMLInputElement>("bk-pages").value);
    const cover = await coverStock();
    const plan = await invoke<BookletPlan>("build_booklet_plan", {
      binding: selectedBinding,
      sourcePages: pages,
      pagesPerSide: Number(el<HTMLSelectElement>("bk-per-side").value),
      duplexMode: duplexMode(),
      sheetIsLandscape: isLandscape(),
      gsm: Number(el<HTMLInputElement>("bk-gsm").value),
      coverGsm: cover?.gsm ?? null,
    });

    const result = el<HTMLDivElement>("booklet-result");
    result.classList.remove("hidden");
    result.innerHTML = `
      <h3 style="margin-top:0">${escapeHtml(plan.profile.name)} — production plan</h3>
      <ul class="spec-list">
        <li><strong>Sheets of paper</strong> ${plan.sheet_count}${plan.separate_cover ? ` (${plan.cover_sheet_count} cover + ${plan.text_sheet_count} text)` : ""}</li>
        <li><strong>Pages per sheet</strong> ${plan.pages_per_sheet} (${plan.pages_per_side} per side${plan.pages_per_sheet > plan.pages_per_side ? ", both sides" : ", one side"})</li>
        <li><strong>Total pages</strong> ${plan.total_pages}${plan.blanks_needed ? ` (${plan.source_pages} supplied + ${plan.blanks_needed} blank)` : ""}</li>
        ${plan.separate_cover ? `<li><strong>Cover stock</strong> ${plan.cover_gsm?.toFixed(0)} GSM, printed separately from the text</li>` : ""}
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
          .map((s) => {
            // The nesting order is identical with or without a separate
            // cover — peeling off the outer four pages of any nested
            // saddle-stitch booklet leaves exactly the same inner order —
            // so this table needs no different numbers, only a label
            // saying which physical stock each sheet is printed on.
            const onCover = plan.separate_cover && s.sheet_number === 1;
            return `
      <div class="spread">
        <div class="spread-title">Sheet ${s.sheet_number}${onCover ? " — cover stock" : plan.separate_cover ? " — text stock" : ""}</div>
        <div class="spread-side"><span>Front</span> ${fmt(s.front[0])} | ${fmt(s.front[1])}</div>
        <div class="spread-side"><span>Back</span> ${fmt(s.back[0])} | ${fmt(s.back[1])}</div>
      </div>`;
          })
          .join("")
      : `<p class="hint">Printer spreads are shown for the classic single-fold saddle-stitch layout
         (2 pages per side, double-sided). ${escapeHtml(plan.profile.name)} with this configuration keeps
         pages in normal reading order instead.</p>`;

    await renderConfigDiagrams();
  }
}

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
let simZoom = 1;
let simPanX = 0;
let simPanY = 0;

/** Wrap whatever is on the stage so it can be zoomed and panned. */
function applySimTransform() {
  const layer = el<HTMLDivElement>("sim-stage").querySelector<HTMLElement>(".zoom-layer");
  if (layer) {
    layer.style.transform = `translate(${simPanX}px, ${simPanY}px) scale(${simZoom})`;
  }
  el<HTMLSpanElement>("sim-zoom-label").textContent = `${Math.round(simZoom * 100)}%`;
}

function setSimZoom(next: number) {
  simZoom = Math.min(6, Math.max(0.25, next));
  applySimTransform();
}

function resetSimView() {
  simZoom = 1;
  simPanX = 0;
  simPanY = 0;
  applySimTransform();
}

/** Put the stage's content inside a transformable layer. */
function wrapStage(content: Node | string) {
  const stage = el<HTMLDivElement>("sim-stage");
  stage.innerHTML = "";
  const layer = document.createElement("div");
  layer.className = "zoom-layer";
  if (typeof content === "string") layer.innerHTML = content;
  else layer.appendChild(content);
  stage.appendChild(layer);
  applySimTransform();
}
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
  const cover = await coverStock();
  try {
    currentSheets = await invoke<SheetSide[]>("plan_sheets", {
      plan,
      trimMm: trim,
      sheetMm: sheet,
      coverSheetMm: cover?.sheetMm ?? null,
    });
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
  const pos = el<HTMLSpanElement>("sim-pos");
  if (!currentPlan) return;

  if (simView === "sheets") {
    if (sheetError) {
      wrapStage(`<p class="sev-warning">${escapeHtml(sheetError)}</p>`);
      pos.textContent = "";
      return;
    }
    const total = currentSheets.length;
    simIndex = Math.min(simIndex, total - 1);
    const side = currentSheets[simIndex];
    wrapStage(sheetSideDiagram(side, el<HTMLInputElement>("sim-marks").checked));
    const kind = side.stock === "cover" ? "cover" : "text";
    pos.textContent = `Sheet side ${simIndex + 1} of ${total} (${kind} stock)`;
    // Swap in the artwork-composited canvas once it is ready.
    const token = ++simToken;
    paintSheet(side, el<HTMLInputElement>("sim-marks").checked)
      .then((c) => {
        if (token !== simToken) return;
        wrapStage(c);
      })
      .catch(() => {
        /* the schematic already rendered */
      });
    return;
  }

  const total = spreadCount();
  simIndex = Math.min(simIndex, total - 1);
  const s = spreadAt(simIndex);
  wrapStage(boundSpreadDiagram(
    currentPlan.profile.key,
    s.left,
    s.right,
    s.leftPos,
    s.rightPos,
    Number(el<HTMLInputElement>("bk-margin").value) || 0,
    el<HTMLSelectElement>("bk-side").value || "left"
  ));
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
      wrapStage(c);
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
  resetSimView();
  renderSimulation();
});

el<HTMLButtonElement>("sim-zoom-in").addEventListener("click", () => setSimZoom(simZoom * 1.25));
el<HTMLButtonElement>("sim-zoom-out").addEventListener("click", () => setSimZoom(simZoom / 1.25));
el<HTMLButtonElement>("sim-zoom-reset").addEventListener("click", resetSimView);

// Ctrl (or Cmd) with the wheel zooms. A plain wheel must keep scrolling
// the page, or the preview swallows the scroll and traps the reader.
el<HTMLDivElement>("sim-stage").addEventListener("wheel", (e) => {
  if (!e.ctrlKey && !e.metaKey) return;
  e.preventDefault();
  setSimZoom(simZoom * (e.deltaY < 0 ? 1.12 : 1 / 1.12));
}, { passive: false });

let panning = false;
let panStartX = 0;
let panStartY = 0;
const stageEl = el<HTMLDivElement>("sim-stage");
stageEl.addEventListener("pointerdown", (e) => {
  panning = true;
  panStartX = e.clientX - simPanX;
  panStartY = e.clientY - simPanY;
  stageEl.classList.add("panning");
  stageEl.setPointerCapture(e.pointerId);
});
stageEl.addEventListener("pointermove", (e) => {
  if (!panning) return;
  simPanX = e.clientX - panStartX;
  simPanY = e.clientY - panStartY;
  applySimTransform();
});
for (const ev of ["pointerup", "pointercancel"]) {
  stageEl.addEventListener(ev, () => {
    panning = false;
    stageEl.classList.remove("panning");
  });
}
el<HTMLButtonElement>("sim-next").addEventListener("click", () => {
  const total = simView === "sheets" ? currentSheets.length : spreadCount();
  simIndex = Math.min(total - 1, simIndex + 1);
  resetSimView();
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
  const cover = await coverStock();
  // A separate cover is a different print run on different paper, so it
  // goes to its own file — named next to the text file rather than asking
  // for a second save location.
  const coverOut = currentPlan.separate_cover
    ? /\.pdf$/i.test(out)
      ? out.replace(/\.pdf$/i, "-cover.pdf")
      : `${out}-cover.pdf`
    : null;
  const count = await invoke<number>("export_imposed_pdf", {
    sourcePath: source.path,
    plan: currentPlan,
    trimMm: trim,
    sheetMm: sheet,
    outputPath: out,
    marks: markOptions(),
    coverSheetMm: cover?.sheetMm ?? null,
    coverOutputPath: coverOut,
  });

  const box = el<HTMLDivElement>("export-result");
  box.classList.remove("hidden");
  box.innerHTML = `
    <p class="sev-info">✓ Wrote <strong>${count}</strong> sheet side${count === 1 ? "" : "s"} to
      <code>${escapeHtml(out)}</code>, arranged for ${escapeHtml(currentPlan.profile.name.toLowerCase())}.</p>
    ${coverOut ? `<p class="sev-info">✓ Cover wrap written separately to <code>${escapeHtml(coverOut)}</code> — print it on the ${escapeHtml(el<HTMLSelectElement>("bk-cover-sheet").value)} cover stock, not with the text run.</p>` : ""}
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
let coverArtPath: string | null = null;

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

el<HTMLButtonElement>("btn-cover-art").addEventListener("click", async () => {
  try {
    const picked = await open({
      filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg"] }],
      multiple: false,
    });
    if (!picked) return;
    coverArtPath = String(picked);
    el<HTMLSpanElement>("cover-art-name").textContent = coverArtPath;
    el<HTMLButtonElement>("btn-cover-art-clear").classList.remove("hidden");
  } catch (e) {
    showError(e);
  }
});

el<HTMLButtonElement>("btn-cover-art-clear").addEventListener("click", () => {
  coverArtPath = null;
  el<HTMLSpanElement>("cover-art-name").textContent = "No artwork chosen";
  el<HTMLButtonElement>("btn-cover-art-clear").classList.add("hidden");
});

el<HTMLButtonElement>("btn-cover-pdf").addEventListener("click", async () => {
  if (!coverLayout) return;
  try {
    const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }], defaultPath: `cover-${coverKind()}.pdf` });
    if (!out) return;
    const artwork = coverArtPath
      ? {
          path: coverArtPath,
          target: el<HTMLSelectElement>("cv-art-target").value,
          fit: el<HTMLSelectElement>("cv-art-fit").value,
          show_guides: el<HTMLSelectElement>("cv-art-guides").value === "true",
        }
      : null;
    const [w, h] = await invoke<[number, number]>("export_cover_pdf", {
      layout: coverLayout,
      outputPath: out,
      title: "PrintPrep cover",
      artwork,
    });
    const box = el<HTMLDivElement>("cover-result");
    box.classList.remove("hidden");
    box.innerHTML = `<p class="sev-info">✓ Wrote a ${(w / 2.8346).toFixed(1)} × ${(h / 2.8346).toFixed(1)} mm
      cover template to <code>${escapeHtml(out)}</code>, with trim and bleed boxes set${coverArtPath ? ", artwork placed" : ""}.</p>
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

// ---------------------------------------------------------------------------
// Printer profiles (§25)
// ---------------------------------------------------------------------------

interface PrinterProfile {
  name: string;
  max_width_mm: number;
  max_height_mm: number;
  duplex_supported: boolean;
  borderless_supported: boolean;
  min_margin_mm: number;
  supported_sizes: string[];
  preferred_orientation: string;
  duplex_behaviour: string;
  notes: string;
}

interface ProfileFinding {
  severity: string;
  message: string;
}

let profiles: PrinterProfile[] = [];

function readProfileForm(): PrinterProfile {
  return {
    name: el<HTMLInputElement>("pp-name").value.trim(),
    max_width_mm: numVal("pp-maxw"),
    max_height_mm: numVal("pp-maxh"),
    duplex_supported: el<HTMLSelectElement>("pp-duplex").value === "true",
    borderless_supported: el<HTMLSelectElement>("pp-borderless").value === "true",
    min_margin_mm: numVal("pp-margin"),
    supported_sizes: el<HTMLInputElement>("pp-sizes").value.split(",").map((s) => s.trim()).filter(Boolean),
    preferred_orientation: el<HTMLSelectElement>("pp-orientation").value,
    duplex_behaviour: el<HTMLSelectElement>("pp-flip").value,
    notes: "",
  };
}

function writeProfileForm(p: PrinterProfile) {
  el<HTMLInputElement>("pp-name").value = p.name;
  el<HTMLInputElement>("pp-maxw").value = String(p.max_width_mm);
  el<HTMLInputElement>("pp-maxh").value = String(p.max_height_mm);
  el<HTMLSelectElement>("pp-duplex").value = String(p.duplex_supported);
  el<HTMLSelectElement>("pp-borderless").value = String(p.borderless_supported);
  el<HTMLInputElement>("pp-margin").value = String(p.min_margin_mm);
  el<HTMLInputElement>("pp-sizes").value = p.supported_sizes.join(", ");
  el<HTMLSelectElement>("pp-orientation").value = p.preferred_orientation;
  el<HTMLSelectElement>("pp-flip").value = p.duplex_behaviour;
}

function renderProfileList() {
  const sel = el<HTMLSelectElement>("pp-list");
  sel.innerHTML = profiles.length
    ? profiles.map((p) => `<option value="${escapeHtml(p.name)}">${escapeHtml(p.name)}</option>`).join("")
    : `<option value="">No profiles saved yet</option>`;
}

async function loadProfiles() {
  profiles = await invoke<PrinterProfile[]>("list_printer_profiles");
  renderProfileList();
  if (profiles.length) writeProfileForm(profiles[0]);
}

el<HTMLSelectElement>("pp-list").addEventListener("change", () => {
  const found = profiles.find((p) => p.name === el<HTMLSelectElement>("pp-list").value);
  if (found) writeProfileForm(found);
});

el<HTMLButtonElement>("btn-pp-new").addEventListener("click", async () => {
  writeProfileForm(await invoke<PrinterProfile>("default_printer_profile"));
});

el<HTMLButtonElement>("btn-pp-save").addEventListener("click", async () => {
  try {
    profiles = await invoke<PrinterProfile[]>("save_printer_profile", { profile: readProfileForm() });
    renderProfileList();
    el<HTMLSelectElement>("pp-list").value = readProfileForm().name;
  } catch (e) {
    showError(e);
  }
});

el<HTMLButtonElement>("btn-pp-delete").addEventListener("click", async () => {
  try {
    const name = el<HTMLSelectElement>("pp-list").value;
    if (!name) return;
    profiles = await invoke<PrinterProfile[]>("delete_printer_profile", { name });
    renderProfileList();
    if (profiles.length) writeProfileForm(profiles[0]);
  } catch (e) {
    showError(e);
  }
});

el<HTMLButtonElement>("btn-pp-check").addEventListener("click", async () => {
  try {
    const sizes = await invoke<PaperSize[]>("list_paper_sizes");
    const chosen = sizes.find((s) => s.name === el<HTMLSelectElement>("pp-check-sheet").value) ?? sizes[1];
    const landscape = el<HTMLSelectElement>("pp-check-orient").value === "landscape";
    const findings = await invoke<ProfileFinding[]>("check_job_against_printer", {
      profile: readProfileForm(),
      sheetMm: landscape ? [chosen.height_mm, chosen.width_mm] : [chosen.width_mm, chosen.height_mm],
      sheetName: chosen.name,
      bleedMm: numVal("pp-check-bleed"),
      duplex: el<HTMLSelectElement>("pp-check-duplex").value,
    });
    const box = el<HTMLDivElement>("pp-findings");
    box.classList.remove("hidden");
    box.innerHTML = `<ul class="findings">${findings
      .map((f) => `<li class="sev-${escapeHtml(f.severity.toLowerCase())}"><strong>${escapeHtml(f.severity)}</strong> ${escapeHtml(f.message)}</li>`)
      .join("")}</ul>`;
  } catch (e) {
    showError(e);
  }
});

// ---------------------------------------------------------------------------
// Print assistant (§26) and guidance panels (§28)
// ---------------------------------------------------------------------------

interface Understanding {
  page_count: number | null;
  trim_size: string | null;
  sheet_size: string | null;
  binding: string | null;
  duplex: boolean | null;
  gsm: number | null;
  assumptions: string[];
  unresolved: string[];
}

interface Advice {
  understanding: Understanding;
  plan: BookletPlan | null;
  explanation: string[];
  warnings: string[];
  suggested_trim: string | null;
  suggested_sheet: string | null;
  suggested_pages_per_side: number;
  suggested_duplex: string;
  sheet_is_landscape: boolean;
}

interface GlossaryEntry {
  term: string;
  short: string;
  recommended: string;
  why: string;
  example: string;
  consequence: string;
}

let lastAdvice: Advice | null = null;

async function askAssistant(question: string) {
  if (!question.trim()) return;
  const advice = await invoke<Advice>("assistant_advise", { request: question });
  lastAdvice = advice;
  const box = el<HTMLDivElement>("as-answer");
  box.classList.remove("hidden");

  const list = (items: string[]) => items.map((x) => `<li>${escapeHtml(x)}</li>`).join("");
  const u = advice.understanding;

  box.innerHTML = `
    <div class="answer-block">
      ${u.assumptions.length ? `<h3>Before I answer</h3><ul class="assumption">${list(u.assumptions)}</ul>` : ""}
      ${u.unresolved.length ? `<h3>I still need to know</h3><ul>${list(u.unresolved)}</ul>` : ""}
      <h3>What I would do</h3>
      <ul>${list(advice.explanation)}</ul>
      ${advice.warnings.length ? `<h3>Watch out for</h3><ul class="findings">${advice.warnings.map((w) => `<li class="sev-warning">${escapeHtml(w)}</li>`).join("")}</ul>` : ""}
      <p class="hint">Every figure above is calculated, not estimated — the same code produces the imposition and the exported file.</p>
    </div>`;

  el<HTMLElement>("as-apply-row").classList.toggle("hidden", !advice.plan);
}

el<HTMLButtonElement>("btn-assistant").addEventListener("click", () => {
  askAssistant(el<HTMLInputElement>("as-input").value).catch(showError);
});
el<HTMLInputElement>("as-input").addEventListener("keydown", (e) => {
  if (e.key === "Enter") askAssistant(el<HTMLInputElement>("as-input").value).catch(showError);
});
document.querySelectorAll<HTMLButtonElement>("#as-examples .chip").forEach((chip) => {
  chip.addEventListener("click", () => {
    el<HTMLInputElement>("as-input").value = chip.textContent!.trim();
    askAssistant(chip.textContent!.trim()).catch(showError);
  });
});

/** Push the assistant's recommendation into the booklet form. */
el<HTMLButtonElement>("btn-assistant-apply").addEventListener("click", () => {
  const a = lastAdvice;
  if (!a?.plan) return;
  selectedBinding = a.plan.profile.key;
  marginOverridden = false;
  // The count the user described to the assistant wins over the loaded file.
  pageCountTyped.add("bk-pages");
  el<HTMLInputElement>("bk-pages").value = String(a.plan.source_pages);
  showPageCountNotice();
  if (a.suggested_trim) el<HTMLSelectElement>("bk-trim").value = a.suggested_trim;
  if (a.suggested_sheet) el<HTMLSelectElement>("bk-sheet").value = a.suggested_sheet;
  el<HTMLSelectElement>("bk-orientation").value = a.sheet_is_landscape ? "landscape" : "portrait";
  el<HTMLSelectElement>("bk-per-side").value = String(a.suggested_pages_per_side);
  el<HTMLSelectElement>("bk-sides").value = a.suggested_duplex === "simplex" ? "single" : "double";
  if (a.suggested_duplex !== "simplex") el<HTMLSelectElement>("bk-flip").value = a.suggested_duplex;
  renderBindingMethods()
    .then(() => showPanel("booklet"))
    .catch(showError);
});

function termCard(e: GlossaryEntry): string {
  return `
    <article class="term">
      <h3>${escapeHtml(e.term)}</h3>
      <p>${escapeHtml(e.short)}</p>
      <dl>
        <dt>Recommended</dt><dd>${escapeHtml(e.recommended)}</dd>
        <dt>Why it matters</dt><dd>${escapeHtml(e.why)}</dd>
        <dt>Example</dt><dd>${escapeHtml(e.example)}</dd>
        <dt>If ignored</dt><dd>${escapeHtml(e.consequence)}</dd>
      </dl>
    </article>`;
}

async function renderGlossary(term = "") {
  const entries = await invoke<GlossaryEntry[]>("assistant_explain", { term });
  el<HTMLDivElement>("gl-list").innerHTML = entries.length
    ? entries.map(termCard).join("")
    : `<p class="hint">Nothing matched “${escapeHtml(term)}”.</p>`;
}

el<HTMLInputElement>("gl-search").addEventListener("input", () => {
  renderGlossary(el<HTMLInputElement>("gl-search").value).catch(showError);
});

async function renderTroubleshooting() {
  const entries = await invoke<GlossaryEntry[]>("assistant_troubleshooting");
  el<HTMLDivElement>("tr-list").innerHTML = entries.map(termCard).join("");
}

async function initExtras() {
  const sizes = await invoke<PaperSize[]>("list_paper_sizes");
  el<HTMLSelectElement>("pp-check-sheet").innerHTML = sizes
    .map((s) => `<option value="${s.name}">${s.name}</option>`)
    .join("");
  el<HTMLSelectElement>("pp-check-sheet").value = "A4";
  await loadProfiles();
  await renderGlossary();
  await renderTroubleshooting();
}

initExtras().catch(() => {
  /* outside Tauri the invoke calls are unavailable */
});

// ---------------------------------------------------------------------------
// Projects (§29) — settings are saved so an output can be reproduced exactly
// ---------------------------------------------------------------------------

interface BookletSettings {
  binding: string;
  page_count: number;
  pages_per_side: number;
  duplex: string;
  sheet_is_landscape: boolean;
  gsm: number;
  trim_size: string;
  sheet_size: string;
  binding_side: string;
  binding_margin_mm: number;
}

interface Project {
  version: number;
  name: string;
  source_path: string | null;
  operations: Operation[];
  booklet: BookletSettings;
  marks: { crop_marks: boolean; fold_marks: boolean; sheet_labels: boolean; bleed_mm: number };
  cover: CoverInputs | null;
  printer_profile: PrinterProfile | null;
  notes: string;
}

let projectPath: string | null = null;

function markOptions() {
  const on = el<HTMLInputElement>("sim-marks")?.checked ?? true;
  const bleed = el<HTMLInputElement>("bk-bleed");
  return {
    crop_marks: on,
    fold_marks: on,
    sheet_labels: on,
    bleed_mm: bleed ? Number(bleed.value) || 0 : 3,
  };
}

/** Gather the whole application state into a project. */
function collectProject(name: string): Project {
  return {
    version: 1,
    name,
    source_path: source?.path ?? null,
    operations,
    booklet: {
      binding: selectedBinding,
      page_count: Number(el<HTMLInputElement>("bk-pages").value) || 1,
      pages_per_side: Number(el<HTMLSelectElement>("bk-per-side").value) || 2,
      duplex: duplexMode(),
      sheet_is_landscape: isLandscape(),
      gsm: numVal("bk-gsm") || 80,
      trim_size: el<HTMLSelectElement>("bk-trim").value,
      sheet_size: el<HTMLSelectElement>("bk-sheet").value,
      binding_side: el<HTMLSelectElement>("bk-side").value || "left",
      binding_margin_mm: numVal("bk-margin"),
    },
    marks: markOptions(),
    cover: collectCoverInputs(),
    printer_profile: profiles.length ? readProfileForm() : null,
    notes: "",
  };
}

/** Push a loaded project back into every screen. */
async function applyProject(p: Project) {
  const b = p.booklet;
  selectedBinding = b.binding;
  marginOverridden = true; // the saved margin wins over the method default
  // The saved counts are deliberate, so re-opening the source must not
  // overwrite them; a disagreement is surfaced by the notice instead.
  pageCountTyped.add("bk-pages");
  if (p.cover) pageCountTyped.add("cv-pages");
  el<HTMLInputElement>("bk-pages").value = String(b.page_count);
  el<HTMLSelectElement>("bk-per-side").value = String(b.pages_per_side);
  el<HTMLSelectElement>("bk-sides").value = b.duplex === "simplex" ? "single" : "double";
  if (b.duplex !== "simplex") el<HTMLSelectElement>("bk-flip").value = b.duplex;
  el<HTMLSelectElement>("bk-orientation").value = b.sheet_is_landscape ? "landscape" : "portrait";
  el<HTMLInputElement>("bk-gsm").value = String(b.gsm);
  el<HTMLSelectElement>("bk-trim").value = b.trim_size;
  el<HTMLSelectElement>("bk-sheet").value = b.sheet_size;
  el<HTMLInputElement>("bk-margin").value = String(b.binding_margin_mm);

  operations = p.operations ?? [];

  if (p.cover) {
    el<HTMLSelectElement>("cv-kind").value = p.cover.kind;
    el<HTMLInputElement>("cv-trim-w").value = String(p.cover.trim_width_mm);
    el<HTMLInputElement>("cv-trim-h").value = String(p.cover.trim_height_mm);
    el<HTMLInputElement>("cv-pages").value = String(p.cover.page_count);
    el<HTMLInputElement>("cv-caliper").value = String(p.cover.caliper_mm);
    el<HTMLInputElement>("cv-bleed").value = String(p.cover.bleed_mm);
    el<HTMLInputElement>("cv-safe").value = String(p.cover.safe_margin_mm);
  }

  await renderBindingMethods();
  el<HTMLSelectElement>("bk-side").value = b.binding_side;
  await refreshCover();
  showPageCountNotice();
  el<HTMLDivElement>("project-name").textContent = p.name;
}

el<HTMLButtonElement>("btn-proj-new").addEventListener("click", async () => {
  try {
    const fresh = await invoke<Project>("new_project");
    projectPath = null;
    source = null;
    documentPageCount = null; // nothing loaded, so nothing to compare against
    unloadPdf();
    el<HTMLSpanElement>("pdf-name").textContent = "";
    el<HTMLDivElement>("pdf-info").classList.add("hidden");
    el<HTMLDivElement>("pdf-pages").innerHTML = "";
    el<HTMLElement>("sim-section").classList.add("hidden");
    await applyProject(fresh);
  } catch (e) {
    showError(e);
  }
});

el<HTMLButtonElement>("btn-proj-open").addEventListener("click", async () => {
  try {
    const picked = await open({ filters: [{ name: "PrintPrep project", extensions: ["ppproj", "json"] }], multiple: false });
    if (!picked) return;
    const [p, sourcePresent] = await invoke<[Project, boolean]>("load_project", { path: String(picked) });
    projectPath = String(picked);
    await applyProject(p);

    // Re-open the source so page operations and previews work again.
    if (p.source_path && sourcePresent) {
      source = await invoke<PdfSource>("inspect_pdf", { path: p.source_path });
      el<HTMLSpanElement>("pdf-name").textContent = p.source_path;
      el<HTMLDivElement>("pdf-info").classList.remove("hidden");
      el<HTMLDivElement>("pdf-info").innerHTML = `<p><strong>${source.page_count}</strong> pages restored from the project.</p>`;
      el<HTMLDivElement>("pdf-actions").classList.remove("hidden");
      try {
        await loadDocument(p.source_path);
      } catch {
        unloadPdf();
      }
      await refreshPages();
    } else if (p.source_path) {
      alert(`The project opened, but its source PDF is missing:\n${p.source_path}\n\nSettings were restored — re-import the file to export again.`);
    }
  } catch (e) {
    showError(e);
  }
});

el<HTMLButtonElement>("btn-proj-save").addEventListener("click", async () => {
  try {
    const target = projectPath ?? (await save({
      filters: [{ name: "PrintPrep project", extensions: ["ppproj"] }],
      defaultPath: "project.ppproj",
    }));
    if (!target) return;
    projectPath = String(target);
    const name = projectPath.split(/[/\\]/).pop()!.replace(/\.(ppproj|json)$/, "");
    await invoke("save_project", { project: collectProject(name), path: projectPath });
    el<HTMLDivElement>("project-name").textContent = name;
  } catch (e) {
    showError(e);
  }
});

// ---------------------------------------------------------------------------
// Image resolution and colour checks (§7, §24, §27)
// ---------------------------------------------------------------------------

interface ColorUsage {
  device_rgb: boolean;
  device_cmyk: boolean;
  device_gray: boolean;
  icc_based: boolean;
  separation: boolean;
  spot_names: string[];
}

function showQuality(title: string, findings: Finding[], extra = "") {
  const box = el<HTMLDivElement>("pdf-quality");
  box.classList.remove("hidden");
  box.innerHTML =
    `<h3 style="margin-top:0">${escapeHtml(title)}</h3>${extra}` +
    `<ul class="findings">${findings
      .map((f) => `<li class="sev-${escapeHtml(f.severity.toLowerCase())}"><strong>${escapeHtml(f.severity)}</strong> ${escapeHtml(f.message)}</li>`)
      .join("")}</ul>`;
}

el<HTMLButtonElement>("btn-check-dpi").addEventListener("click", async () => {
  if (!source) return showError("Import a PDF first.");
  try {
    const findings = await invoke<Finding[]>("preflight_images", { path: source.path, minimumDpi: null });
    const images = await invoke<
      { page: number; pixel_width: number; pixel_height: number; effective_dpi: number }[]
    >("scan_image_resolution", { path: source.path });
    const table = images.length
      ? `<p class="hint">${images.length} image(s) scanned. Lowest effective resolution:
         <strong>${Math.min(...images.map((i) => i.effective_dpi)).toFixed(0)} DPI</strong>.</p>`
      : "";
    showQuality("Image resolution", findings, table);
  } catch (e) {
    showError(e);
  }
});

el<HTMLButtonElement>("btn-check-colour").addEventListener("click", async () => {
  if (!source) return showError("Import a PDF first.");
  try {
    const [usage, findings] = await invoke<[ColorUsage, Finding[]]>("scan_color_usage", {
      path: source.path,
      commercialPrint: el<HTMLInputElement>("colour-commercial").checked,
    });
    const spots = usage.spot_names.length
      ? `<p class="hint">Spot colourants: ${escapeHtml(usage.spot_names.join(", "))}</p>`
      : "";
    showQuality("Colour spaces", findings, spots);
  } catch (e) {
    showError(e);
  }
});

// ---------------------------------------------------------------------------
// Signature and step-and-repeat export (§22 modes 5 and 7)
// ---------------------------------------------------------------------------

interface Signature {
  number: number;
  first_page: number;
  last_page: number;
  blank_pages: number;
}

async function nupSizes(): Promise<{ trim: [number, number]; sheet: [number, number] }> {
  const sizes = await invoke<PaperSize[]>("list_paper_sizes");
  const find = (n: string) => sizes.find((s) => s.name === n) ?? sizes[1];
  const trim = find(el<HTMLSelectElement>("nup-trim").value);
  const sheet = find(el<HTMLSelectElement>("nup-sheet").value);
  const landscape = el<HTMLSelectElement>("nup-orient").value === "landscape";
  return {
    trim: [trim.width_mm, trim.height_mm],
    sheet: landscape ? [sheet.height_mm, sheet.width_mm] : [sheet.width_mm, sheet.height_mm],
  };
}

function showExportResult(html: string) {
  const box = el<HTMLDivElement>("nup-export-result");
  box.classList.remove("hidden");
  box.innerHTML = html;
}

el<HTMLButtonElement>("btn-nup-export").addEventListener("click", async () => {
  if (!source) return showError("Import a PDF on the Import & Pages screen first.");
  try {
    const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }], defaultPath: "step-and-repeat.pdf" });
    if (!out) return;
    const { trim, sheet } = await nupSizes();
    const sheets = await invoke<number>("export_step_and_repeat_pdf", {
      sourcePath: source.path,
      page: Number(el<HTMLInputElement>("nup-repeat-page").value) || 1,
      copies: Number(el<HTMLInputElement>("nup-pages").value) || 1,
      rows: Number(el<HTMLInputElement>("nup-rows").value) || 1,
      cols: Number(el<HTMLInputElement>("nup-cols").value) || 1,
      trimMm: trim,
      sheetMm: sheet,
      spacingMm: numVal("nup-spacing"),
      outputPath: out,
      marks: markOptions(),
    });
    showExportResult(`<p class="sev-info">✓ Wrote <strong>${sheets}</strong> sheet(s) to <code>${escapeHtml(out)}</code>.</p>`);
  } catch (e) {
    showError(e);
  }
});

el<HTMLButtonElement>("btn-sig-plan").addEventListener("click", async () => {
  try {
    const pages = Number(el<HTMLInputElement>("nup-pages").value) || 1;
    const sigs = await invoke<Signature[]>("divide_signatures", {
      pageCount: pages,
      signatureSize: Number(el<HTMLSelectElement>("sig-size").value),
      balanced: el<HTMLSelectElement>("sig-balanced").value === "true",
    });
    showExportResult(
      `<p><strong>${sigs.length}</strong> signature(s) for ${pages} pages.</p>` +
        `<ul class="findings">${sigs
          .map((s) => `<li>Signature ${s.number}: pages ${s.first_page}–${s.last_page}` +
            (s.blank_pages ? ` <span class="sev-warning">(${s.blank_pages} blank)</span>` : "") + "</li>")
          .join("")}</ul>`
    );
  } catch (e) {
    showError(e);
  }
});

el<HTMLButtonElement>("btn-sig-export").addEventListener("click", async () => {
  if (!source) return showError("Import a PDF on the Import & Pages screen first.");
  try {
    const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }], defaultPath: "signatures.pdf" });
    if (!out) return;
    const { trim, sheet } = await nupSizes();
    // Back sides need the rotation the chosen flip demands.
    const flip = await invoke<DuplexPlan>("get_duplex_plan", {
      mode: "short_edge",
      sheetIsLandscape: el<HTMLSelectElement>("nup-orient").value === "landscape",
    });
    const written = await invoke<string[]>("export_signature_pdfs", {
      sourcePath: source.path,
      pageCount: Number(el<HTMLInputElement>("nup-pages").value) || 1,
      signatureSize: Number(el<HTMLSelectElement>("sig-size").value),
      balanced: el<HTMLSelectElement>("sig-balanced").value === "true",
      trimMm: trim,
      sheetMm: sheet,
      backRotation: flip.back_side_rotation,
      outputPath: out,
      combined: el<HTMLSelectElement>("sig-combined").value === "true",
      marks: markOptions(),
    });
    showExportResult(
      `<p class="sev-info">✓ Wrote ${written.length} file(s):</p><ul class="findings">${written
        .map((w) => `<li><code>${escapeHtml(w)}</code></li>`)
        .join("")}</ul>`
    );
  } catch (e) {
    showError(e);
  }
});

async function initNupSizes() {
  const sizes = await invoke<PaperSize[]>("list_paper_sizes");
  const opts = sizes.map((s) => `<option value="${s.name}">${s.name}</option>`).join("");
  el<HTMLSelectElement>("nup-trim").innerHTML = opts;
  el<HTMLSelectElement>("nup-sheet").innerHTML = opts;
  el<HTMLSelectElement>("nup-trim").value = "A6";
  el<HTMLSelectElement>("nup-sheet").value = "A3";
}

initNupSizes().catch(() => {
  /* outside Tauri the invoke calls are unavailable */
});
