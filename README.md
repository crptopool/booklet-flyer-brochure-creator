# PrintPrep — Booklet, Flyer & Brochure Creator

A cross-platform (Windows / macOS / Linux) print-preparation application built
with **Tauri 2**, a **Rust** backend and a lightweight TypeScript frontend.

PrintPrep helps home users, designers, print shops and publishers prepare
booklets, flyers, pamphlets, brochures and imported PDFs for professional or
desktop printing and binding — page imposition, booklet sequencing, duplex
logic, bleed/margins, spine width, creep compensation and print-ready PDF
export.

## Architecture

| Layer | Location | Role |
| --- | --- | --- |
| Deterministic calculations | `src-tauri/src/print_calc/` | Page imposition, booklet sequencing, sheet counts, signatures, spine width, creep, effective DPI, scaling, printable region, binding margins, unit conversions |
| PDF foundation | `src-tauri/src/pdf_ops/` | Non-destructive PDF inspection, page operations (reorder / rotate / delete / duplicate / blank insertion) and export via [`lopdf`](https://crates.io/crates/lopdf) |
| Preflight | `src-tauri/src/preflight.rs` | INFO / WARNING / ERROR findings before export |
| Tauri commands | `src-tauri/src/lib.rs` | Bridge exposing all functionality to the UI |
| Frontend | `src/`, `index.html` | Import-PDF, Booklet, N-Up and Binding screens |

### Design principles

1. **The original source file is never modified.** All edits are project
   instructions applied to a virtual page list and only materialised into a
   *new* PDF at export time.
2. **Deterministic geometry.** Page order and measurements are computed by
   tested Rust code — never by language-model inference.
3. **Vector preservation.** Pages are copied by PDF object reference; content
   is never rasterised.
4. **Nothing silent.** Blank pages, scaling and rotations are always surfaced
   to the user before export.

## Development

Prerequisites: [Rust](https://rustup.rs), Node.js ≥ 20 and the
[Tauri 2 platform prerequisites](https://tauri.app/start/prerequisites/)
(on Linux: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `librsvg2-dev`).

```bash
npm install          # frontend dependencies
npm run tauri dev    # run the desktop app in development
npm run tauri build  # produce platform installers
```

### Tests

All print calculations are covered by Rust unit tests:

```bash
cd src-tauri
cargo test --lib
cargo clippy --lib   # lint
```

The frontend type-checks and builds with:

```bash
npm run build
```

## Feature status (per the phased roadmap)

- **Phase 1 — PDF foundation:** ✅ import/inspect (page count, sizes,
  orientation, mixed sizes, encryption detection, metadata), non-destructive
  reorder / rotate / delete / duplicate / blank insertion, standard PDF export
- **Phase 3 — N-Up imposition (core calculations):** ✅ sequential N-up,
  step-and-repeat with optimum-grid fitting, cut-and-stack, duplex sheet
  counts, centered grid geometry
- **Phase 4 — Booklet:** ✅ saddle-stitch printer-spread sequencing,
  blank-page requirements & placement strategies, sheet counts
- **Phase 5 — Binding intelligence:** ✅ binding-margin recommendations,
  GSM→caliper guidance, spine-width formulas, creep compensation
- **Phase 6 — Signatures (calculations):** ✅ signature division & balancing
- **Phase 9 — Preflight:** ✅ page-count/binding, mixed sizes, missing bleed,
  wrong trim size, rotation and encryption checks with severity levels
- Upcoming: imposed/booklet PDF rendering, visual sheet previews, cover
  designer, printer profiles, AI assistant
