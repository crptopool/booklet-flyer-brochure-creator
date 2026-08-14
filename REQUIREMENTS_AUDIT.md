# PrintPrep — Requirements Audit

**Audited against:** `AI_Agent_Print_Imposition_Booklet_Cover_Requirements.md`
**Date:** 2026-08-14
**Codebase:** Rust backend + TypeScript frontend, 206 passing unit tests

**Branches audited:** `main`, `copilot/create-print-layout-application`, `copilot/ai-agent-requirements-specification`

> **Update 1 — booklet binding methods.** All five binding methods
> (saddle stitch, perfect, spiral/coil, Wire-O, case binding) with per-method
> settings, pages-per-sheet, simplex/duplex and long/short-edge flip. Closed
> **§14 duplex flip-edge logic** (`print_calc/duplex.rs`) and **§11.3/§11.4
> per-binding settings**, and every setting renders a live illustration.
>
> **Update 2 — the imposition renderer now exists.** This retires the original
> headline finding. `pdf_ops/impose.rs` places source pages onto printer sheets
> as Form XObjects and writes a real imposed PDF; `pdf_ops/sheets.rs` turns a
> binding plan into those sheets. Verified end-to-end: an 8-page source imposes
> to `8|1 / 2|7 / 6|3 / 4|5` exactly as §11.1 specifies, with text still
> selectable and fonts intact in the output. Closed or advanced: **§22 export
> modes** (1 of 9 → 4 of 9), **§21 print marks** (crop, fold and sheet labels
> rendered), **§20 preview** (bound-document and printed-sheet simulation, both
> built from the same imposition the exporter consumes, satisfying §37.9), and
> the MUST HAVE rows for booklet imposition and 1-up/2-up/4-up.
>
> **Update 3 — the three remaining gaps are closed.** (a) `/TrimBox`,
> `/BleedBox` and `/CropBox` are now written to every imposed sheet and to the
> cover, so output is print-ready for a commercial printer. (b) Previews render
> the user's **real artwork**: `pdfjs-dist` rasterises pages in the webview for
> page thumbnails and composites them into the sheet and bound-document
> previews — preview only, never the export path. (c) **Phase 7 Cover Creator**
> is built: eBook, paperback, hardcover and dust-jacket layouts with spine,
> board overhang, hinge, turn-in, flaps, safe areas and a barcode reservation,
> exporting a guide PDF (or PNG for eBook). This also completes §11.4 hardcover
> geometry.
>
> **Update 4 — printer profiles, the assistant and cover artwork.**
> **§25 printer profiles** are saved per machine and check a job for sheet fit,
> duplex capability, flip-direction mismatch, borderless/bleed conflict and
> unconfigured sizes. **Phase 8 / §26 the assistant** reads a plain-language
> request and answers with a full plan — it is a rule-based parser over the
> existing deterministic modules, offline and reproducible, which is what §33
> requires; **§28 guidance** ships as a searchable glossary and a
> troubleshooting screen. The **Cover Creator now places artwork** (PNG or
> JPEG) into the template, with fill/fit/stretch and a guides-off mode for the
> final file. The UI was also reorganised into a collapsible categorised
> sidebar.

---

## 1. Headline finding

The **deterministic calculation core is genuinely strong** — it is the part of the
spec that was built properly, and it is well covered by tests. Every worked example
in §34 and every acceptance scenario in §38 (A–E) has a corresponding passing test.

**The original headline finding — that none of these calculations reached a PDF —
has been resolved.** The imposition renderer now places source pages onto printer
sheets and writes the file, so booklet imposition, N-up and duplex logic produce
something a user can actually print rather than a number on screen.

What remains is narrower. Of the nine export modes in §22, five exist (reading,
imposed, booklet, N-up and cover); signature, step-and-repeat and proofing-guide
PDFs do not.
Print-ready output is complete for these modes: crop, fold and sheet marks are
drawn, and `/TrimBox`, `/BleedBox` and `/CropBox` are written so a commercial
printer knows exactly where to trim.

**§20 Preview** now shows the user's real artwork: page thumbnails on the import
screen, and the actual pages composited into the sheet and bound-document previews.
Crucially it is still met *correctly* — the previews are generated from the same
`SheetSide` structures the exporter consumes, satisfying §37.9. Rasterising happens
only in the webview for display; the export path still copies pages as vectors.
What remains absent from §20 is zoom, pan and the individual guide toggles.

---

## 2. Priority scorecard (§36)

Legend: ✅ complete · ◐ partial (calculation exists, not delivered to user) · ✗ absent

### MUST HAVE — ~93%

| # | Requirement | Status | Notes |
|---|---|---|---|
| 1 | PDF import | ✅ | `pdf_ops/document.rs` — count, dims, orientation, mixed sizes, metadata, encryption |
| 2 | Page management | ✅ | reorder / rotate / delete / duplicate / insert blank, all non-destructive |
| 3 | Trim & sheet sizes | ✅ | 11 presets incl. SRA3, 12×18, 13×19 |
| 4 | Bleed | ✅ | computed, written as `/BleedBox`, and drawn on cover templates |
| 5 | Margins | ✅ | independent + binding-aware, `geometry.rs` |
| 6 | 1-up / 2-up / 4-up | ✅ | sequences, geometry and imposed PDF output |
| 7 | Booklet imposition | ✅ | printer spreads rendered to PDF; verified against the §11.1 example |
| 8 | Duplex logic | ✅ | flip long/short edge, back-side rotation, manual-duplex steps — `duplex.rs` |
| 9 | Preview | ◐ | real page artwork in thumbnails, sheet and bound previews, from the exporter's own geometry; no zoom/pan or guide toggles yet |
| 10 | Blank-page handling | ✅ | never silent; two placement strategies |
| 11 | Print-ready PDF | ✅ | imposition, crop/fold/label marks and trim/bleed/crop boxes |
| 12 | DPI validation | ◐ | engine correct and applied to eBook cover artwork; PDF image XObjects still unscanned |
| 13 | Saddle-stitch support | ✅ | sequencing, imposition, folds and staple guidance |
| 14 | Binding guidance | ✅ | per-binding margins, GSM→caliper guidance |
| 15 | Deterministic sequencing | ✅ | pure Rust, 206 tests, zero model inference — §33 honoured |

### SHOULD HAVE — ~70%

| Requirement | Status | Notes |
|---|---|---|
| Signatures | ◐ | division + balancing ✅; no per-signature PDF, no labels |
| Creep | ✅ | None / Automatic / Custom, per-sheet offsets, limit flag |
| Perfect binding | ✅ | spine calc, reading-order imposition and cover generation |
| Spiral / Wire-O / comb | ✅ | margins, binding side and punch-zone preview |
| Cover creator | ✅ | eBook, paperback, hardcover and dust jacket with full guide export |
| Spine calculation | ✅ | both spec formulas + custom pages-per-mm |
| Step-and-repeat | ◐ | optimum-grid fitting ✅ (Scenario C passes); no PDF |
| Cut-and-stack | ◐ | sequence ✅; no PDF |
| Printer profiles | ✅ | saved per machine; jobs checked for fit, duplex, flip direction and bleed |
| Automated preflight | ◐ | 6 of ~16 checks implemented |

### COULD HAVE — 0%

PDF/X, ICC profiles, CMYK tooling, barcode generation, nesting optimisation,
artwork bleed extension, AI cover design, cloud print integration — none present.

---

## 3. Roadmap phase completion (§35)

| Phase | Title | Status |
|---|---|---|
| 1 | PDF Foundation | ~95% — thumbnails ✅; image import (PNG/JPEG/TIFF/SVG) still missing |
| 2 | Basic Print Preparation | ~90% — crop marks, sheet preview and trim/bleed boxes all ✅ |
| 3 | N-Up Imposition | ~75% — sequences and imposed PDF ✅; no spacing/scaling controls |
| 4 | Booklet Printing | ~95% — sequencing, duplex flip, imposed PDF, fold marks and previews all ✅ |
| 5 | Binding Intelligence | ~95% — five binding methods with full settings profiles; hardcover board, hinge and turn-in geometry now on the Cover Creator |
| 6 | Signature Engine | ~50% — division ✅, export and labels ✗ |
| 7 | Cover Designer | ~95% — all four cover kinds with spine, guides, barcode area and artwork placement |
| 8 | AI Assistant | ~85% — request understanding, full recommendation, glossary and troubleshooting; no free-form conversation |
| 9 | Automated Preflight | ~35% |
| 10 | Advanced Commercial | 0% |

The README on `main` claims Phases 1, 3, 4, 5, 6 and 9 as "✅". That is accurate
only if read as *"the calculations for these phases are complete"*. It overstates
delivered capability and should be reworded — the checkmarks read as shipped
features.

---

## 4. Section-by-section gaps

**§4 Document types** — no product presets at all. Paper sizes exist, but none of
the 8 single-sheet, 8 folded (bi-fold, tri-fold, Z-fold, gate, roll, accordion…)
or 11 multi-page product types are modelled. Folded products have no fold geometry.

**§5 File import** — PDF only. PNG, JPEG, TIFF, SVG and multi-file combination are
absent. No thumbnail previews. Encryption *is* detected (§5 last bullet ✅).

**§6 Project Setup Wizard** — absent. The UI is four independent calculator tabs;
there is no guided source → output → trim → sheet → orientation → mode → binding flow.

**§7 DPI** — presets and the exact §7 warning string are implemented, but nothing
walks a PDF's image XObjects, so per-image warnings never fire on a real document.

**§8 Bleed/trim** — media, crop, trim and bleed boxes are computed *and written* to
imposed sheets and cover templates, and cover templates draw the trim, bleed, fold
and safe-area overlays. The three §8 content warnings (background not extending into
bleed, text near trim, objects in the binding danger area) remain unimplemented —
they need content analysis the project does not do.

**§11.4 Hardcover** — complete. Board overhang, hinge groove, turn-in allowance and
spine are all editable inputs on the Cover Creator, and each is drawn on the exported
template.

**§12–13** — crop marks, registration marks, page labels and sheet labels are
listed as per-mode controls but none are implemented. Custom imposition (manual
assignment of pages to sheet positions) is absent.

**§14 Duplex** — implemented in `print_calc/duplex.rs`: flip on long edge, flip on
short edge, simplex and manual duplex, with the required back-side rotation derived
from the sheet orientation and manual reinsertion instructions. The flip result is
illustrated live. Still absent: stored printer-dependent duplex behaviour (§25) and
the printer test sheet.

**§19 Cover Creator** — implemented. eBook covers report aspect ratio, pixel
adequacy and effective print DPI, and export a PNG template. Print covers compute
spine width from the real page count and caliper, lay out back/spine/front (plus
flaps for a dust jacket), and export a PDF with trim, bleed, fold, hinge, safe-area
and barcode guides. Not yet supported: placing existing artwork into the template.

**§21 Print Marks** — crop marks, fold marks and sheet labels are drawn on the imposed 
output and can be toggled. Registration marks, centre marks and colour bars are absent.

**§22 PDF Export Modes** — 5 of 9 implemented: reading, imposed, booklet, N-up and
cover. Missing: signature PDFs, step-and-repeat and proofing-guide PDFs.

**§23 PDF Quality** — vectors, text and fonts *are* preserved (pages are copied by
object reference, never rasterised — §23 and §37.3 honoured). But there are no
quality presets, no compression control and no PDF/X.

**§24 Color** — absent. No RGB/CMYK detection or ICC handling.

**§25 Printer profiles** — implemented. Name, maximum sheet, duplex and
borderless support, minimum printable margin, configured sizes, preferred feed
and known duplex-flip behaviour, saved to the platform config directory. A job
can be checked against a profile before printing.

**§26 Assistant** — implemented, and worth being precise about what it is. It
reads a plain-language request, extracts page count, sizes, binding, duplex and
paper weight, states its assumptions, asks for anything genuinely missing, and
answers with a complete plan it can apply to the Booklet screen. It is a
**rule-based parser over the deterministic modules, not a language model** — it
runs offline, needs no key and gives the same answer every time. That is the
correct reading of §33, which requires page order and measurements to come from
deterministic code rather than model inference. What it does not do is hold a
free-form conversation or answer questions outside print preparation.

**§27 Preflight** — implemented: encrypted, empty document, page-count vs binding,
mixed page sizes, stored rotations, missing bleed, wrong page size (7 checks).
Missing: low-DPI images, text near trim, text near binding, content outside
printable region, blank-page detection, excessive scaling, missing fonts,
transparency risks, thin margins, cover/spine mismatch (9 checks). Severity levels
INFO/WARNING/ERROR are correctly modelled.

**§28 Guidance panel** — a searchable glossary gives every term a short
explanation, a recommended value, why it matters, an example and the consequence
of ignoring it, in the exact shape §28 specifies. A troubleshooting screen covers
the common failure modes.

**§29 Project management** — none. No save, Save As, autosave, recent projects,
undo, redo, duplicate, or settings import/export. Operations live in a JavaScript
array and are lost when the window closes. This also breaks §40.10 (*"Save project
settings so the output can be reproduced exactly"*).

**§31 UI workflow** — 4 tabs against 8 required screens. Missing Home, Document
Setup, Preview, Preflight and Export screens.

**§32 Auto-fix** — absent.

**§39 Pre-export summary** — absent; nothing summarises trim/sheets/binding/quality
or requires confirmation when warnings remain.

---

## 5. What the code does well

Worth recording, because it is the foundation everything else can be built on:

- **§33 / §37.4 fully honoured.** All page order and geometry is deterministic
  Rust. No LLM touches a measurement anywhere in the codebase.
- **§30 / §37.1 fully honoured.** Operations are project instructions applied to a
  virtual page list; the source file is opened read-only and export refuses to
  overwrite it (`export_refuses_overwriting_source`).
- **§23 / §37.3 honoured.** Pages are copied by PDF object reference, so vector
  content, live text and embedded fonts survive. Inherited page attributes
  (`MediaBox`, `Resources`, `Rotate`, `CropBox`) are correctly materialised onto
  copies so pages don't silently change size under a new parent — a subtle bug
  that is easy to get wrong and was got right.
- **§37.6 honoured.** mm / cm / inch / point conversion with round-trip tests.
- **§37.10 honoured.** 89 unit tests covering every calculation module.
- **Spec examples are encoded as tests**, not just implemented: the §11.1 8-page
  booklet layout, §34's 32→8 sheets, 200→100 sheets, 4-up simplex 25 / duplex 13,
  and Scenarios A, B, C and E all appear as named test cases.
- Content Security Policy is properly restrictive.

---

## 6. Recommended build order

1. **Imposition renderer** — place source pages onto sheets as PDF Form XObjects
   with the `grid_placements` geometry that already exists. This one component
   unlocks export modes 3, 4, 6 and 7 and converts four partial MUST HAVEs into
   complete ones. Highest value per unit of work by a wide margin.
2. **Write `/TrimBox` and `/BleedBox`** on export, and render crop/fold/registration
   marks — completes "print-ready PDF".
3. **Sheet & duplex preview**, rendered from the same `SheetLayout` structs the
   exporter consumes, satisfying §37.9 by construction.
4. **Page thumbnails** on the Pages screen (§31 Screen 3).
5. **Project save/load** (§29) — currently blocks reproducibility (§40.10).
6. **Duplex flip-edge logic** and manual-duplex instructions (§14).
7. **Wire DPI checking into preflight** by walking image XObjects (§7, §27).
8. Then Cover Creator (Phase 7), printer profiles, AI assistant.

---

## 7. Branch audit

| Branch | Tip | Relationship to `main` |
|---|---|---|
| `main` | `4889820` | Contains all production code |
| `copilot/create-print-layout-application` | `b493be2` | **Already fully merged** into `main` via PR #2 (merge commit `484d453`). Zero unique content. |
| `copilot/ai-agent-requirements-specification` | `64ffb31` | One commit, *"Initial plan"*, which is **empty — it changes no files**. Zero unique content. Open PR #1 is an abandoned WIP stub whose README is a one-line placeholder. |

**Conclusion: `main` is already the union of all work across all three branches.**
No code exists on any branch that is not on `main`, and no merge can add content.
Verified by `git merge-base --is-ancestor` and by confirming both branches add zero
files relative to `main` (`git diff --diff-filter=A`).
