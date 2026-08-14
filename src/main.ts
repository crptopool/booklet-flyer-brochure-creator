import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  bindingDiagram,
  duplexFlipDiagram,
  gutterDiagram,
  pagesPerSheetDiagram,
  resultDiagram,
} from "./diagrams";

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
