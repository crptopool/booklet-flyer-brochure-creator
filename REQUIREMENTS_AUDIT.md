# PrintPrep — Requirements Audit

**Audited against:** `AI_Agent_Print_Imposition_Booklet_Cover_Requirements.md`
**Date:** 2026-08-14
**Codebase:** ~3,130 lines (Rust backend + TypeScript frontend), 89 passing unit tests
**Branches audited:** `main`, `copilot/create-print-layout-application`, `copilot/ai-agent-requirements-specification`

---

## 1. Headline finding

The **deterministic calculation core is genuinely strong** — it is the part of the
spec that was built properly, and it is well covered by tests. Every worked example
in §34 and every acceptance scenario in §38 (A–E) has a corresponding passing test.

The gap is that **almost none of those calculations reach a PDF or a screen.**

The application can *calculate* a booklet imposition but cannot *produce* one.
`export_pdf` writes a reordered / rotated / blank-padded page list and nothing more —
there is no code path anywhere that places source pages onto a printer sheet.
Of the nine export modes required by §22, only mode 1 ("Reading PDF") exists.

That single gap invalidates several MUST HAVE line items at once: booklet
imposition, print-ready PDF, 1-up/2-up/4-up, and duplex logic all currently
terminate in a number displayed in the UI rather than in a file a user can print.

The second gap is that **§20 Preview does not exist in any form.** There is no
reading preview, sheet preview, duplex preview, fold preview, page thumbnail, zoom
or guide toggle. The UI presents numeric tables. §37.9 requires previews to be
rendered from the same geometry used for export; today neither side of that
sentence is implemented.

---

## 2. Priority scorecard (§36)

Legend: ✅ complete · ◐ partial (calculation exists, not delivered to user) · ✗ absent

### MUST HAVE — ~55%

| # | Requirement | Status | Notes |
|---|---|---|---|
| 1 | PDF import | ✅ | `pdf_ops/document.rs` — count, dims, orientation, mixed sizes, metadata, encryption |
| 2 | Page management | ✅ | reorder / rotate / delete / duplicate / insert blank, all non-destructive |
| 3 | Trim & sheet sizes | ✅ | 11 presets incl. SRA3, 12×18, 13×19 |
| 4 | Bleed | ◐ | `bleed_box()` computes it; **never written to the PDF** as `/TrimBox` `/BleedBox` |
| 5 | Margins | ✅ | independent + binding-aware, `geometry.rs` |
| 6 | 1-up / 2-up / 4-up | ◐ | sequences and cell geometry computed; no PDF output |
| 7 | Booklet imposition | ◐ | sequencing correct and tested; no imposed PDF |
| 8 | Duplex logic | ◐ | sheet counts + even-side padding only; no flip-edge logic |
| 9 | Preview | ✗ | **nothing** — no thumbnails, sheet, duplex or fold preview |
| 10 | Blank-page handling | ✅ | never silent; two placement strategies |
| 11 | Print-ready PDF | ✗ | no trim/bleed boxes, no marks, no imposition |
| 12 | DPI validation | ◐ | engine correct, but no image is ever extracted from a PDF to check |
| 13 | Saddle-stitch support | ◐ | sequencing ✅, output ✗ |
| 14 | Binding guidance | ✅ | per-binding margins, GSM→caliper guidance |
| 15 | Deterministic sequencing | ✅ | pure Rust, 89 tests, zero model inference — §33 honoured |

### SHOULD HAVE — ~40%

| Requirement | Status | Notes |
|---|---|---|
| Signatures | ◐ | division + balancing ✅; no per-signature PDF, no labels |
| Creep | ✅ | None / Automatic / Custom, per-sheet offsets, limit flag |
| Perfect binding | ◐ | spine calc ✅; cover generation ✗ |
| Spiral / Wire-O / comb | ◐ | margins + binding side ✅; punch-zone preview ✗ |
| Cover creator | ✗ | Phase 7 entirely absent |
| Spine calculation | ✅ | both spec formulas + custom pages-per-mm |
| Step-and-repeat | ◐ | optimum-grid fitting ✅ (Scenario C passes); no PDF |
| Cut-and-stack | ◐ | sequence ✅; no PDF |
| Printer profiles | ✗ | §25 absent |
| Automated preflight | ◐ | 6 of ~16 checks implemented |

### COULD HAVE — 0%

PDF/X, ICC profiles, CMYK tooling, barcode generation, nesting optimisation,
artwork bleed extension, AI cover design, cloud print integration — none present.

---

## 3. Roadmap phase completion (§35)

| Phase | Title | Status |
|---|---|---|
| 1 | PDF Foundation | ~85% — missing thumbnails and image import |
| 2 | Basic Print Preparation | ~40% — calcs only; no crop marks, sheet preview or true print-ready PDF |
| 3 | N-Up Imposition | ~50% — sequences ✅, no PDF output, no spacing/rotation/scaling controls |
| 4 | Booklet Printing | ~50% — sequencing ✅, no PDF, no preview, no fold/staple marks, no flip logic |
| 5 | Binding Intelligence | ~70% — strongest phase; hardcover (§11.4) only has a margin value |
| 6 | Signature Engine | ~50% — division ✅, export and labels ✗ |
| 7 | Cover Designer | 0% |
| 8 | AI Assistant | 0% |
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

**§8 Bleed/trim** — media/crop/trim/bleed distinctions are computed but not written
to output, and none of the required visual overlays exist. The three §8 warnings
(background not extending into bleed, text near trim, objects in binding danger
area) are all unimplemented — they require content analysis that does not exist.

**§11.4 Hardcover** — spine, hinge, wrap, board dimensions and turn-in allowance
are all missing; only a binding-margin constant exists.

**§12–13** — crop marks, registration marks, page labels and sheet labels are
listed as per-mode controls but none are implemented. Custom imposition (manual
assignment of pages to sheet positions) is absent.

**§14 Duplex** — flip-on-long-edge / flip-on-short-edge, printer-dependent
orientation, manual-duplex instructions and the printer test sheet are all absent.
Only sheet counting is done.

**§19 Cover Creator** — absent in full, both eBook and print-book covers.

**§21 Print Marks** — no mark rendering of any kind.

**§22 PDF Export Modes** — 1 of 9 implemented.

**§23 PDF Quality** — vectors, text and fonts *are* preserved (pages are copied by
object reference, never rasterised — §23 and §37.3 honoured). But there are no
quality presets, no compression control and no PDF/X.

**§24 Color** — absent. No RGB/CMYK detection or ICC handling.

**§25 Printer profiles** — absent.

**§26 AI Assistant** — absent. Worth stating plainly: the specification is titled
"AI Agent Requirements" and Phase 8 is an AI assistant, but the application
contains no AI functionality whatsoever. Note this is *not* a correctness problem —
§33 explicitly requires that page order and measurements come from deterministic
code rather than model inference, and the codebase honours that rule rigorously.
What is missing is the advisory layer on top.

**§27 Preflight** — implemented: encrypted, empty document, page-count vs binding,
mixed page sizes, stored rotations, missing bleed, wrong page size (7 checks).
Missing: low-DPI images, text near trim, text near binding, content outside
printable region, blank-page detection, excessive scaling, missing fonts,
transparency risks, thin margins, cover/spine mismatch (9 checks). Severity levels
INFO/WARNING/ERROR are correctly modelled.

**§28 Guidance panel** — only two static hint sentences.

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
