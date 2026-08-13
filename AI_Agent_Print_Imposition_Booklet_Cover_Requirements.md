# AI Agent Requirements Specification
## Print Layout, Imposition, Booklet, Flyer, Pamphlet, eBook Cover & PDF Preparation Software

**Document Purpose:**  
This specification defines the business, functional, workflow, print-preparation, imposition, preview, validation, and export requirements for an AI-assisted software application that helps users prepare booklets, flyers, pamphlets, brochures, eBook/print-book covers, and imported PDFs for professional or desktop printing and binding.

The software shall guide non-expert and professional users through page setup, DPI/resolution, bleed, trim size, paper thickness, binding allowance, imposition, page sequencing, duplex printing, PDF generation, and print preview.

---

# 1. Product Vision

Build an intelligent print-preparation application that can:

- Create new print layouts from scratch.
- Import existing PDF files and prepare them for different print/binding outcomes.
- Create and prepare:
  - Booklets
  - Flyers
  - Pamphlets
  - Brochures
  - Leaflets
  - Posters
  - Cards
  - eBook covers
  - Paperback/hardcover print covers
  - Multi-page books/manuals/catalogues
- Automatically determine the correct page order for printing and binding.
- Support 1-up, 2-up, 3-up, 4-up, and configurable N-up layouts.
- Prepare sheets for simplex or duplex printing.
- Guide the user on trim size, margins, bleed, gutter, paper thickness, creep, spine width, and binding margins.
- Provide visual previews before exporting.
- Export print-ready PDF files and alternative PDF layouts.
- Preserve the original PDF wherever possible.
- Warn users about print-quality or layout problems before export.

---

# 2. Primary User Groups

1. Home / Office Users
   - Users printing small booklets, flyers, brochures, cards, or manuals.

2. Graphic Designers
   - Users preparing files for professional printing.

3. Print Shops
   - Users performing imposition and binding preparation for customer PDFs.

4. Authors / Publishers
   - Users preparing eBooks, paperback covers, manuals, and books.

5. Corporate Users
   - Users producing reports, catalogues, training material, pamphlets, and marketing collateral.

---

# 3. Core Business Objectives

The system shall:

- Reduce errors in booklet and multi-page printing.
- Eliminate manual page-order calculations.
- Make print terminology understandable to non-specialists.
- Generate reliable print-ready PDFs.
- Support common desktop and commercial printing workflows.
- Automatically adapt layouts according to paper size and binding method.
- Prevent accidental content clipping.
- Improve print quality by detecting low-resolution artwork.
- Allow imported PDFs to be reformatted without recreating their content.
- Clearly distinguish document page size from physical sheet size.
- Allow multiple output variants from the same source document.

---

# 4. Supported Document Types

The software shall provide presets for:

## 4.1 Single-Sheet Products

- Flyer
- Poster
- Certificate
- Card
- Invitation
- Business card
- Menu
- Leaflet

## 4.2 Folded Products

- Bi-fold pamphlet
- Tri-fold pamphlet
- Z-fold
- Gate fold
- Half fold
- Roll fold
- Accordion fold
- Custom fold

## 4.3 Multi-Page Products

- Saddle-stitched booklet
- Stapled booklet
- Perfect-bound book
- Spiral / coil-bound document
- Wire-O bound document
- Comb-bound document
- Ring-bound document
- Hardcover book
- Case-bound book
- Glue-bound manual
- Loose-leaf pages

## 4.4 Covers

- eBook front cover
- Paperback full-wrap cover
- Hardcover full-wrap cover
- Dust jacket
- Front/back cover pair
- Book cover with spine

---

# 5. File Import Requirements

The system shall support importing:

- PDF
- PDF/X where available
- PNG
- JPEG/JPG
- TIFF where technically feasible
- SVG where supported
- Individual page images
- Multiple PDFs combined into one project

For imported PDFs, the application shall:

- Detect page count.
- Detect page dimensions.
- Detect orientation.
- Detect mixed page sizes.
- Read PDF metadata where available.
- Preserve vector content whenever possible.
- Preserve embedded fonts whenever possible.
- Display thumbnail previews.
- Allow page reordering.
- Allow page rotation.
- Allow deletion of pages.
- Allow duplication of pages.
- Allow insertion of blank pages.
- Allow importing additional pages into an existing project.
- Detect encrypted/protected PDFs and inform the user if modification is restricted.

The application shall NEVER rasterize the entire PDF unnecessarily.

---

# 6. Project Setup Wizard

A guided setup wizard shall request:

1. Source
   - New document
   - Import PDF
   - Import images
   - Combine files

2. Desired Output
   - Normal PDF
   - Print-ready PDF
   - Booklet
   - Flyer/pamphlet
   - Bound book
   - Cover
   - N-up print sheet
   - Custom imposition

3. Final Trim Size

Examples:

- A3
- A4
- A5
- A6
- Letter
- Legal
- Tabloid
- B5
- Custom width × height

4. Printer Sheet Size

Examples:

- A3
- A4
- A5
- Letter
- SRA3
- 12 × 18 in
- 13 × 19 in
- Custom

5. Orientation

- Portrait
- Landscape
- Automatic

6. Printing Mode

- Single-sided
- Double-sided / duplex
- Manual duplex

7. Binding Method

- None
- Saddle stitch
- Staple
- Perfect bind
- Spiral
- Wire-O
- Comb
- Ring
- Hardcover
- Custom

---

# 7. DPI and Resolution Management

The software shall support and explain common print resolutions.

Recommended presets:

- 72–96 DPI: screen/web only
- 150 DPI: draft / large-format viewing
- 200 DPI: acceptable general print
- 300 DPI: recommended standard print
- 600 DPI: high-detail line art / premium print
- 1200 DPI: specialized high-resolution workflows

The software shall:

- Detect effective image DPI at final print size.
- Warn when an image falls below a configurable threshold.
- Distinguish image DPI from printer hardware DPI.
- Avoid degrading vector artwork.
- Allow resampling only when explicitly required.
- Show per-image resolution warnings.
- Recommend 300 DPI by default for photographic print artwork.

Example warning:

> Image on Page 7 will print at approximately 118 DPI. Recommended minimum: 200 DPI; preferred: 300 DPI.

---

# 8. Bleed, Trim and Safe Area

The application shall distinguish:

- Media box
- Crop box
- Trim box
- Bleed box
- Safe/content area

Bleed presets:

- 0 mm
- 2 mm
- 3 mm
- 5 mm
- 0.125 inch
- Custom

Default recommendation:

- 3 mm for most commercial printed materials unless printer specifications state otherwise.

The application shall visually overlay:

- Trim line
- Bleed line
- Safe margin
- Binding margin
- Fold lines

The software shall warn when:

- Background artwork does not extend into bleed.
- Text is too close to trim.
- Important objects are inside the binding danger area.

---

# 9. Margins and Gutter

The system shall allow independent control of:

- Top
- Bottom
- Left
- Right
- Inside
- Outside
- Gutter
- Binding margin

Suggested safe-margin presets:

- 3 mm
- 5 mm
- 8 mm
- 10 mm
- Custom

For bound documents, inside margins shall automatically be adjusted based on binding type.

The user shall be able to override recommended values.

---

# 10. Paper Thickness / GSM Guidance

The application shall provide guidance on paper grammage and approximate thickness.

Example categories:

- 70–90 GSM: standard text/office pages
- 90–120 GSM: premium text / brochures
- 130–170 GSM: flyers / light covers
- 200–250 GSM: cards / booklet covers
- 300–350 GSM: heavy covers/cards
- Custom GSM

The system shall allow:

- Interior paper GSM
- Cover paper GSM
- User-entered sheet thickness in mm
- Automatic approximate thickness calculation
- Manual override

The application must clearly state that actual caliper varies by paper manufacturer and finish.

---

# 11. Binding Rules

## 11.1 Saddle Stitch

The system shall:

- Require total pages to be divisible by 4.
- Offer automatic blank-page insertion when needed.
- Arrange pages into printer spreads.
- Support sheet signatures.
- Account for creep on thicker booklets.
- Show fold and staple positions.

Example:

For an 8-page booklet:

Sheet side A:
- Page 8 | Page 1

Sheet side B:
- Page 2 | Page 7

Next sheet:
- Page 6 | Page 3
- Page 4 | Page 5

The application shall calculate this automatically.

## 11.2 Perfect Binding

The application shall:

- Maintain normal reading page order in the content PDF.
- Support cover generation separately.
- Calculate approximate spine width.

Spine formula shall support:

Spine Width = Number of Sheets × Paper Caliper

or

Spine Width = Number of Pages / 2 × Paper Thickness

with configurable printer/manufacturer formulas.

## 11.3 Spiral / Wire-O / Comb

The system shall:

- Increase inside/binding margin.
- Allow binding side:
  - Left
  - Right
  - Top
- Recommend punch-safe clearance.
- Preview punch/binding exclusion zone.

## 11.4 Hardcover / Case Binding

The application shall support:

- Spine
- Hinge
- Wrap
- Board dimensions
- Turn-in allowance
- Front cover
- Back cover

Values shall be configurable based on printer specifications.

---

# 12. Page Imposition Engine

This is a CORE feature.

The system shall support:

- 1-up
- 2-up
- 3-up
- 4-up
- 6-up
- 8-up
- 9-up
- 12-up
- 16-up
- Custom rows × columns

For each mode users shall control:

- Sheet size
- Page size
- Orientation
- Page spacing
- Margins
- Rotation
- Scaling
- Centering
- Crop marks
- Registration marks
- Page labels
- Sheet labels

---

# 13. Imposition Modes

The application shall support:

## Normal N-Up

Pages printed sequentially across the sheet.

Example 4-up:

1 | 2  
3 | 4

## Step-and-Repeat

Repeat the same page multiple times on one sheet.

Example:

1 | 1  
1 | 1

Suitable for:

- Business cards
- Flyers
- Labels
- Invitations

## Cut-and-Stack

Arrange pages so that after sheets are cut and stacked, reading order becomes sequential.

## Booklet / Printer Spreads

Rearrange reading pages into folded-sheet order.

## Custom Imposition

Allow the user to manually assign page numbers to sheet positions.

---

# 14. Duplex Printing Logic

The application shall support:

- Flip on long edge
- Flip on short edge
- Printer-dependent duplex orientation
- Manual duplex

The preview shall clearly show:

- Sheet front
- Sheet back
- Required page rotations

For manual duplex, provide instructions such as:

1. Print front sides.
2. Reinsert printed sheets in specified orientation.
3. Print reverse sides.

The system should support a printer-test sheet to determine correct reinsertion orientation.

---

# 15. Page Count Handling

The application shall detect page-count incompatibilities.

For booklet mode:

- 4, 8, 12, 16, 20, 24... pages are directly supported.
- If the document contains 10 pages, the application shall offer:
  - Add 2 blank pages automatically.
  - Insert blanks at end.
  - Insert blanks before back cover.
  - Let user choose blank-page positions.

The software shall never silently add pages without informing the user.

---

# 16. Signature Support

For large books, the application shall support signatures.

Examples:

- 4-page
- 8-page
- 12-page
- 16-page
- 20-page
- 24-page
- 32-page signatures
- Custom

The system shall:

- Divide a large document into signatures.
- Impose each signature separately.
- Clearly label signature number.
- Allow optional blank-page balancing.
- Generate either:
  - One combined PDF
  - Separate PDF per signature

---

# 17. Creep / Shingling Compensation

For saddle-stitched and folded signatures, the application shall optionally compensate for page creep caused by paper thickness.

Inputs:

- Sheet count
- Paper caliper
- Fold count
- Maximum permitted creep

Options:

- No creep adjustment
- Automatic
- Custom compensation

The preview shall show simulated creep.

---

# 18. Scaling and Fit Modes

Available modes:

- Actual size / 100%
- Fit to printable area
- Fit to sheet
- Fill sheet
- Shrink oversized pages only
- Custom percentage
- Auto rotate and center

The application shall clearly warn if scaling changes the intended final trim size.

---

# 19. Cover Creator

## 19.1 eBook Cover

Features:

- Front-only layout
- Custom pixel dimensions
- Aspect ratio presets
- DPI guidance
- Export PNG/JPEG/PDF

## 19.2 Print Book Cover

The system shall create:

Back Cover + Spine + Front Cover

Inputs:

- Final trim width
- Final trim height
- Page count
- Paper thickness/caliper
- Binding type
- Bleed
- Spine width
- Printer wrap allowance

The software shall generate:

- Spine guides
- Trim guides
- Bleed guides
- Safe area
- Barcode-safe region option

---

# 20. Preview Requirements

The application shall provide multiple previews:

## Reading Preview

Shows pages in normal reading order.

## Sheet Preview

Shows exactly what is placed on each physical sheet.

## Duplex Preview

Shows front and reverse sides.

## Folded Booklet Preview

Simulates folding and resulting page sequence.

## Trimmed Preview

Shows final trimmed output.

## Binding Preview

Shows approximate binding position and content clearance.

Preview shall support:

- Zoom
- Pan
- Page navigation
- Sheet navigation
- Toggle guides
- Toggle crop marks
- Toggle bleed
- Toggle page numbers
- Toggle binding margin
- Front/back comparison

---

# 21. Print Marks

Optional marks:

- Crop marks
- Trim marks
- Registration marks
- Center marks
- Fold marks
- Color bars where applicable
- Page labels
- Sheet numbers
- Signature labels

Marks shall remain outside the final trim area wherever possible.

---

# 22. PDF Export Modes

The software shall support:

1. Reading PDF
   - Standard reading page order.

2. Print-Ready PDF
   - Correct trim/bleed and production settings.

3. Imposed PDF
   - Pages arranged on printer sheets.

4. Booklet PDF
   - Printer-spread order.

5. Signature PDFs
   - One file per signature or combined.

6. N-Up PDF
   - Multiple source pages per sheet.

7. Step-and-Repeat PDF
   - Multiple copies per sheet.

8. Cover PDF
   - Print-ready cover spread.

9. Preview PDF
   - Optional visible guides for proofing only.

---

# 23. PDF Quality Requirements

The PDF engine shall aim to:

- Preserve vectors.
- Preserve text as text where possible.
- Embed fonts where licensing permits.
- Preserve transparency correctly.
- Preserve source color information.
- Avoid unnecessary JPEG recompression.
- Allow optional image compression.
- Support selectable output quality.

Output presets:

- Draft
- Office Print
- High Quality
- Commercial Print
- Maximum Quality
- Custom

Where technically feasible, support PDF/X presets such as:

- PDF/X-1a
- PDF/X-4

---

# 24. Color Guidance

The application shall explain:

- RGB
- CMYK
- Grayscale
- Spot color concept

The system should:

- Detect RGB artwork when a commercial-print workflow expects CMYK.
- Warn without forcing conversion.
- Allow ICC profile selection where supported.
- Preserve original color space by default unless the user selects conversion.

---

# 25. Printer Capability Profile

Users may optionally create printer profiles containing:

- Printer name
- Maximum paper size
- Duplex supported
- Borderless supported
- Minimum printable margin
- Supported paper sizes
- Preferred orientation
- Known duplex-flip behavior

The application can use this profile to make safer recommendations.

---

# 26. AI Assistant Requirements

The AI assistant shall guide users conversationally.

Example:

User:
> I have a 36-page A5 PDF and want to print it as an A4 saddle-stitched booklet.

AI should determine:

- A5 final page size
- A4 landscape printer sheets
- 2 booklet pages per side
- Duplex printing
- 36 is divisible by 4
- 9 physical sheets
- correct booklet page sequence
- recommended bleed if present
- paper-weight guidance
- creep risk
- staple/fold recommendation

The AI shall explain assumptions before export.

---

# 27. AI Preflight Check

Before exporting, run an automatic preflight.

Check:

- Missing bleed
- Low DPI images
- Wrong page size
- Mixed page size
- Page count unsuitable for binding
- Text too near trim
- Text too near binding
- Content outside printable region
- Blank pages
- Unexpected rotations
- Incorrect duplex orientation
- Excessive scaling
- Missing fonts if detectable
- Transparency risks where relevant
- Very thin margins
- Cover/spine size mismatch

Severity levels:

- INFO
- WARNING
- ERROR

Errors should block export only where output would likely be invalid.

---

# 28. User Guidance Panel

Each technical option shall provide:

- Short explanation
- Recommended value
- Why the value matters
- Example
- Possible consequence if ignored

Example:

**Bleed: 3 mm recommended**

Bleed extends background artwork beyond the trim line so minor cutting variations do not create white edges.

---

# 29. Undo / Project Management

The system shall support:

- New project
- Save project
- Save As
- Autosave
- Recent projects
- Undo
- Redo
- Duplicate project
- Export project settings
- Import project settings

Project files shall preserve:

- Source references
- page order
- imposition
- trim
- bleed
- margins
- binding
- output settings
- printer profile
- custom marks

---

# 30. Non-Destructive Editing

All transformations shall be non-destructive.

Original files should remain unchanged.

Operations such as:

- crop
- rotation
- scaling
- imposition
- page order
- blank page insertion

shall be represented as project instructions until export.

---

# 31. Recommended UI Workflow

## Screen 1 — Home

Actions:

- New Print Project
- Import PDF
- Create Booklet
- Create Flyer/Pamphlet
- Create Cover
- N-Up / Imposition
- Recent Projects

## Screen 2 — Document Setup

Choose:

- Output type
- final size
- sheet size
- orientation
- printer mode

## Screen 3 — Pages

- thumbnails
- reorder
- rotate
- insert/delete
- blank-page handling

## Screen 4 — Print & Binding

- binding
- bleed
- margins
- paper
- duplex
- signatures
- creep

## Screen 5 — Imposition

- 1/2/4/N-up
- sheet layout
- sequence
- spacing
- marks

## Screen 6 — Preview

- reading
- sheet
- duplex
- fold/binding

## Screen 7 — Preflight

- warnings/errors
- AI recommendations
- auto-fix options

## Screen 8 — Export

- PDF output mode
- quality
- filename
- individual/combined signatures

---

# 32. Auto-Fix Features

Where safe, offer:

- Add blank pages
- Rotate pages
- Center pages
- Scale oversized pages
- Add bleed by extending suitable edge/background content
- Increase safe margin
- Select correct booklet order
- Fix reverse-side orientation
- Change sheet orientation
- Reduce N-up count if content becomes too small

Any AI-generated visual extension for bleed must be clearly disclosed and optional.

---

# 33. Calculations

The implementation shall include deterministic calculation modules for:

- page imposition
- booklet sequencing
- sheet count
- signature division
- spine width
- creep
- safe binding margin
- printable region
- effective DPI
- scale percentage
- bleed dimensions
- trim dimensions

Important:

AI may recommend settings, but mathematical page order and measurements shall be calculated by deterministic code rather than language-model inference.

---

# 34. Example Calculations

## Saddle-Stitch Sheet Count

Physical Sheets = Total Pages / 4

Example:

32 pages → 8 sheets.

## Perfect-Bound Sheet Count

Sheets = Pages / 2

Example:

200 pages → 100 sheets.

Approximate spine:

100 × paper caliper.

## 4-Up Simplex

100 pages at 4 pages per sheet:

ceil(100 / 4) = 25 sheets.

## 4-Up Duplex

8 document pages per physical sheet:

ceil(100 / 8) = 13 physical sheets.

The final sheet may contain blank positions.

---

# 35. Phase-Based Development Roadmap

## Phase 1 — PDF Foundation

Build:

- PDF import
- page thumbnails
- page reorder
- rotation
- delete
- duplicate
- blank insertion
- trim/page-size detection
- standard PDF export

Goal:
Reliable PDF manipulation foundation.

## Phase 2 — Basic Print Preparation

Build:

- trim size
- sheet size
- scaling
- margins
- bleed
- safe area
- DPI checker
- crop marks
- sheet preview
- print-ready PDF export

## Phase 3 — N-Up Imposition

Build:

- 1-up
- 2-up
- 4-up
- configurable grid
- step-and-repeat
- sequential N-up
- duplex front/back
- orientation controls

## Phase 4 — Booklet Printing

Build:

- saddle-stitch sequencing
- automatic blank insertion
- booklet preview
- duplex flip logic
- page rotation
- fold marks
- staple indicators

## Phase 5 — Binding Intelligence

Build:

- perfect bind
- spiral
- Wire-O
- comb
- hardcover guidance
- paper thickness
- binding margin
- spine calculation
- creep compensation

## Phase 6 — Signature Engine

Build:

- configurable signature sizes
- automatic splitting
- signature balancing
- signature PDF export
- labels

## Phase 7 — Cover Designer

Build:

- eBook covers
- paperback covers
- hardcover covers
- spine calculation
- bleed
- guides
- barcode-safe area

## Phase 8 — AI Assistant

Build AI features for:

- workflow recommendation
- explaining print terminology
- binding recommendation
- margin suggestions
- paper-weight guidance
- troubleshooting

## Phase 9 — Automated Preflight

Build:

- resolution detection
- bleed verification
- margin verification
- page-count rules
- imposition validation
- duplex checks
- binding checks
- auto-fix recommendations

## Phase 10 — Advanced Commercial Features

Optional:

- PDF/X
- ICC profiles
- CMYK workflows
- color bars
- registration marks
- custom printer profiles
- advanced cut-and-stack
- advanced signature planning
- gang-run optimization

---

# 36. Priority Classification

## MUST HAVE

- PDF import
- page management
- trim/sheet sizes
- bleed
- margins
- 1-up/2-up/4-up
- booklet imposition
- duplex logic
- preview
- blank-page handling
- print-ready PDF
- DPI validation
- saddle-stitch support
- binding guidance
- deterministic page sequencing

## SHOULD HAVE

- signatures
- creep
- perfect binding
- spiral/Wire-O support
- cover creator
- spine calculation
- step-and-repeat
- cut-and-stack
- printer profiles
- automated preflight

## COULD HAVE

- PDF/X
- ICC profiles
- advanced CMYK tools
- barcode generator
- nesting optimization
- automatic artwork bleed extension
- AI visual cover design
- cloud print-service integration

---

# 37. Technical Design Principles

1. Never alter the original source file.
2. Keep editing non-destructive.
3. Use vector-based PDF transformation wherever possible.
4. Use deterministic algorithms for print geometry and page order.
5. Use AI for assistance, recommendations, explanations, and problem detection.
6. Display measurements in:
   - mm
   - cm
   - inches
   - points
7. Internally use a consistent high-precision coordinate system.
8. Avoid rounding errors during repeated transformations.
9. Render previews using the same geometry used for export.
10. Keep print calculations covered by automated unit tests.

---

# 38. Validation / Acceptance Examples

## Scenario A — 20-Page A5 Booklet

Input:
- 20-page A5 PDF
- A4 paper
- saddle stitch
- duplex

Expected:

- 5 physical A4 sheets
- A4 landscape sheets
- 2 booklet pages per side
- correct outer/inner sequencing
- front/back preview
- print-ready PDF

## Scenario B — 22-Page Booklet

Expected:

- warning that page count is not divisible by 4
- recommendation to add 2 blank pages
- user chooses blank position
- output becomes 24 pages / 6 sheets

## Scenario C — 100 A6 Flyers on A3

User chooses step-and-repeat.

Expected:

- application calculates optimum rows/columns
- respects crop spacing
- shows number of copies per sheet
- calculates total sheets
- optionally adds crop marks

## Scenario D — 200-Page Perfect-Bound A5 Book

Expected:

- normal page-order content PDF
- binding gutter guidance
- paper-caliper input
- calculated spine width
- separate full-wrap cover PDF
- front/spine/back preview

## Scenario E — Imported A4 PDF Printed 2-Up on A3

Expected:

- two A4 pages per A3 side
- optional duplex
- scaling 100% where physically valid
- preview confirms orientation
- output imposed PDF

---

# 39. Safety Against Printing Mistakes

Before final export, display a summary:

**Final Document**
- Trim size
- Page count
- Bleed

**Physical Print**
- Sheet size
- Number of sheets
- Pages per side
- Simplex/duplex

**Binding**
- Binding type
- Gutter
- Spine/creep if applicable

**Quality**
- Minimum detected DPI
- Preflight warnings

Require confirmation if major warnings remain.

---

# 40. Final AI Agent Instruction

Implement this product incrementally according to the phases defined above.

For every print transformation:

1. Preserve the source.
2. Calculate page geometry deterministically.
3. Calculate page sequencing deterministically.
4. Generate a visual preview from the same transformation model.
5. Validate the output through preflight checks.
6. Explain important print decisions to the user.
7. Never silently resize, crop, add pages, or reorder pages.
8. Clearly distinguish:
   - source page
   - final trimmed page
   - imposed page
   - physical printer sheet
9. Allow the user to override recommendations.
10. Save project settings so the output can be reproduced exactly.

The finished system should make complicated professional print-preparation concepts understandable to ordinary users while remaining sufficiently accurate and configurable for experienced designers and print shops.
