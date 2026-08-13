import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";

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
  });
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
  grid.querySelectorAll<HTMLButtonElement>("button").forEach((btn) => {
    btn.addEventListener("click", () => {
      const pos = Number(btn.dataset.pos);
      if (btn.dataset.act === "rotate") operations.push({ type: "rotate_page", position: pos, degrees: 90 });
      if (btn.dataset.act === "dup") operations.push({ type: "duplicate_page", position: pos });
      if (btn.dataset.act === "del") operations.push({ type: "delete_page", position: pos });
      refreshPages().catch(showError);
    });
  });
  await refreshPreflight(pages.length);
}

async function refreshPreflight(pageCount: number) {
  if (!source) return;
  const findings = await invoke<Finding[]>("run_preflight", {
    source: { ...source },
    binding: "saddle_stitch",
    bleedMm: 3.0,
    expectedTrimMm: null,
  });
  const box = el<HTMLDivElement>("pdf-preflight");
  box.classList.remove("hidden");
  const rows = findings
    .map((f) => `<li class="sev-${f.severity.toLowerCase()}"><strong>${f.severity}</strong> ${f.message}</li>`)
    .join("");
  box.innerHTML = `<h3>Preflight (saddle-stitch intent, ${pageCount} pages)</h3>` +
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

el<HTMLButtonElement>("btn-booklet").addEventListener("click", async () => {
  try {
    const pages = Number(el<HTMLInputElement>("bk-pages").value);
    const blanks = await invoke<number>("booklet_blanks_needed", { pageCount: pages });
    const sheets = await invoke<number>("booklet_sheet_count", { pageCount: pages });
    const spreads = await invoke<SheetSpread[]>("booklet_order", { pageCount: pages });

    const result = el<HTMLDivElement>("booklet-result");
    result.classList.remove("hidden");
    result.innerHTML =
      `<p><strong>${sheets}</strong> physical sheets (duplex), 2 booklet pages per side.</p>` +
      (blanks > 0
        ? `<p class="sev-warning">⚠ ${pages} pages is not divisible by 4 — ${blanks} blank page(s) will be needed. You choose where they go; pages are never added silently.</p>`
        : `<p class="sev-info">✓ Page count is divisible by 4 — no blanks needed.</p>`);

    const grid = el<HTMLDivElement>("booklet-spreads");
    grid.innerHTML = spreads
      .map(
        (s) => `
      <div class="spread">
        <div class="spread-title">Sheet ${s.sheet_number}</div>
        <div class="spread-side"><span>Front</span> ${fmt(s.front[0])} | ${fmt(s.front[1])}</div>
        <div class="spread-side"><span>Back</span> ${fmt(s.back[0])} | ${fmt(s.back[1])}</div>
      </div>`
      )
      .join("");
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
