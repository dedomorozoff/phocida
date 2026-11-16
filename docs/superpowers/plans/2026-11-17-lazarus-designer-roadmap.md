# Lazarus Designer & IDE Feature Roadmap

> Living roadmap for bringing Lazarus-class form design and related IDE features to Phocida.
> Status: planning document (future work). Not an implementation spec for a single PR.
> Date: 2026-11-17

## Context

Phocida is a hybrid Pascal/Lazarus IDE (Zed fork). The form designer is the centerpiece of the
classic Lazarus workflow. The current implementation is an **MVP visual designer**:

- `.lfm` is the source of truth (parse/serialize, round-trip).
- Workspace item with canvas (form, grid, component rects), selection, drag/move and corner resize.
- Palette of common LCL components, click-to-place and drag-to-canvas placement.
- Property inspector: geometry + all `.lfm` properties as editable key/value, add/remove properties, Apply/Save/Reload.
- Opens from `.lfm`, a form unit `.pas/.pp`, or a program `.lpr/.dpr` (first referenced form).

This roadmap captures the remaining work to approach Lazarus IDE parity. It is intentionally broad;
items should be broken into focused specs/plans before implementation.

## Guiding principles

- **`.lfm` remains the source of truth.** Designer edits must round-trip; never invent a parallel format.
- **`pasls` / CodeTools independence for core designer ops** where practical; leverage LSP/CodeTools only when they add clear value (e.g. event wiring, identifier resolution).
- **Incremental delivery.** Each milestone must leave the designer usable and the project buildable.
- **Correct Pascal/LCL semantics** over pixel-perfect Lazarus UI replication.
- **No editor/LSP regressions.** `cargo check/test` for touched crates and `cargo build --bin phocida -j 2` per batch.

## Current gaps (high level)

- Property editing is untyped (raw strings); no Object Inspector semantics (defaults, categories, typed editors, events tab).
- No `.lfm` ↔ `.pas` synchronization (published fields, event handler declarations/implementations).
- No LCL layout engine behavior (anchors, align, constraints, autosize, parenting).
- No specialized component editors (menus, actions, images, collections).
- No undo/redo, multi-select, copy/paste, alignment tools, zoom/pan, or accessibility affordances.
- Limited component set and simplified visual representation.
- No frames, data modules, or form inheritance support.

---

## Roadmap by area

### 1. Object Inspector & property system (typed)

**Goal:** Evolve the key/value inspector into a Lazarus-like Object Inspector.

- **Typed properties & editors**
  - Boolean (checkbox), integer/float (validated), string/text (multiline), enumerations (dropdown), sets (multi-check).
  - Color, cursor, font, pen/brush, icon/bitmap/picture (with pickers/preview).
  - Component references (dropdown of compatible named components on the form).
  - Class/record sub-properties (expandable nodes, e.g. `Font.Name`, `Constraints.MaxWidth`).
  - Collections/lists editors (TStrings, Columns, Items) with add/remove/reorder dialogs.
- **Semantics**
  - Default values: distinguish stored vs default; omit defaults on save where appropriate.
  - Read-only/derived properties surfaced but not editable.
  - Property name resolution/canonical casing; case-insensitive matching consistent with LFM.
  - Validation and Pascal-compatible serialization for each type (quoting, sets, enums).
- **UX**
  - Categories/tabs (Properties / Events — see area 2).
  - Filter/search, inline description/hint, revert-to-default action.
  - Stable ordering and predictable focus/keyboard navigation.

**Milestones:** typed primitives → sub-properties → reference/collection editors → categories & polish.

### 2. Events & code synchronization (`.lfm` ↔ `.pas`)

**Goal:** Connect visual actions to Pascal source, matching Lazarus event workflow.

- **Events tab**
  - List publishable events for selected component/class; show assigned handler name or empty.
  - Create handler: generate method stub in the form unit and assign in `.lfm`.
  - Navigate to existing handler; rename/clear handler with safe updates.
- **Source synchronization**
  - Maintain published component fields in the form class declaration consistent with `.lfm` objects.
  - Keep `{$R *.lfm}` / form resource linkage intact.
  - Detect and surface conflicts (manual edits, missing unit/form, mismatched names).
- **Integration**
  - Prefer CodeTools/pasls-aware edits where reliable; fall back to structured text edits with clear diagnostics.
  - Double-click component/event in designer opens/creates the handler in the editor.

**Dependencies:** reliable unit/form discovery (project model), Pascal parsing/editing utilities, conflict handling UX.

### 3. Canvas layout, parenting & LCL behavior

**Goal:** Make placement and hierarchy reflect runtime LCL layout.

- **Hierarchy & parenting**
  - Correct parent/child containment (controls inside TPanel/TGroupBox/TScrollBox/TPageControl tabs, etc.).
  - Reparenting via drag, z-order, bring-to-front/send-to-back.
  - Visual containment clipping and child coordinate spaces.
- **Layout properties**
  - Align, Anchors, BorderSpacing, Margins, Constraints, AutoSize.
  - Design-time visualization of anchors/alignment; optional live layout preview rules.
- **Interaction**
  - Snap-to-grid (configurable), snap-to-guidelines/sibling edges, rulers (optional).
  - Multi-select, group move/resize, alignment/distribution commands.
  - Resize handles on all edges/corners, keyboard nudge, size-to-content where applicable.
  - Correct hit-testing with overlapping/contained controls.

**Milestones:** parenting & z-order → align/anchors visualization → snapping/multi-select tools.

### 4. Specialized component & property editors

**Goal:** First-class editors for complex LCL components beyond plain properties.

- **Menus & actions:** TMainMenu/TMenuItem editor, TPopupMenu, TActionList/TAction editor.
- **Data/visual lists:** TImageList editor, TListView columns/items (design-time), TTreeView nodes (as applicable), TStringList editor.
- **Frames & composites:** TFrame editing/embedding, component templates (stretch).
- **In-place editing:** caption/text in-place edit on canvas (where safe) with commit/cancel.
- **Custom component editors/hooks:** extensibility points for component-specific dialogs (future).

### 5. Form/module kinds & project model

**Goal:** Support the range of Lazarus design-time modules.

- **Form kinds:** standard TForm, data modules, frames; correct create/use patterns and file triples (`.lfm/.pas/.lpi/.lpr` context).
- **Inheritance:** visually open/edit inherited forms (read-only base vs overridden), VFI awareness.
- **Project awareness**
  - Resolve which form belongs to which unit/project; handle multiple forms/projects in workspace.
  - Create-new-form flow (scaffold `.pas` + `.lfm`, register in project) — coordinated with project tooling.
  - Non-destructive handling when `.lfm`/`.pas` disagree.

### 6. Palette & component coverage

**Goal:** Broader, accurate component availability and representation.

- **Component set:** expand beyond the starter set to core LCL (Standard, Additional, Common Controls, Dialogs, etc.), prioritized by real project usage.
- **Accurate defaults:** sensible default bounds/properties per class.
- **Visual fidelity:** lightweight design-time rendering per component class (not runtime-perfect, but recognizable); graceful fallback for unknown classes.
- **Palette UX:** grouping/tabs, search/filter, recently used.
- **Extensibility (later):** discover/register components from packages (ambitious; depends on environment/toolchain).

### 7. Canvas UX, tools & productivity

**Goal:** Make the designer efficient for daily use.

- **Viewport:** zoom in/out/fit, pan, scroll-into-view for selection, high-DPI correctness.
- **Editing commands:** undo/redo stack (designer-scoped), copy/cut/paste/duplicate, delete, lock/hide (design-time).
- **Selection tools:** marquee select, select-parent, tab-order traversal.
- **Alignment/distribution:** align edges/centers, equalize size/spacing.
- **Design-time aids:** component name labels toggle, bounds/position readout, status hints.
- **Accessibility & input:** keyboard-driven workflows, focus management, consistent shortcuts.

### 8. Reliability, performance & diagnostics

**Goal:** Robustness on real-world Lazarus projects.

- **LFM fidelity:** preserve unknown properties/ordering/constructs where possible; avoid data loss on round-trip; handle binary/object data blocks.
- **Error handling:** tolerant parse with actionable diagnostics; recoverable designer state on malformed/partial forms.
- **Performance:** efficient rendering for large forms, incremental updates, avoid UI stalls on save/reload.
- **Testing:** unit tests for parse/serialize/property mapping/layout math; GPUI view tests for interactions; corpus tests against real `.lfm` files.
- **Observability:** non-fatal logging for ignored constructs to guide future coverage.

---

## Suggested phasing (indicative)

This is a sequencing aid, not a commitment; revisit per milestone.

1. **Foundations (done / in progress):** MVP canvas, palette, drag/resize, key=value inspector, save/reload, basic round-trip.
2. **Daily-driver basics:** undo/redo, delete/duplicate, zoom/pan, multi-select & alignment, parenting/z-order, expanded component set & defaults.
3. **Code integration:** `.lfm` ↔ `.pas` sync, events tab & handler generation/navigation.
4. **Typed Object Inspector:** typed editors for common property kinds, sub-properties, component references, collections.
5. **LCL layout semantics:** anchors/align/borderspacing/constraints/autosize with correct design-time behavior.
6. **Specialized editors & module kinds:** menus/actions/imagelists, frames/data modules, form creation/inheritance scenarios.
7. **Parity & polish:** broad component fidelity, package/extensibility exploration, performance hardening on large real projects.

## Definition of "usable for real work" (target)

A Pascal/Lazarus developer can open a typical Lazarus form project in Phocida, visually place and arrange standard controls, edit their key properties and events, have handlers wired into source, and build/run — without needing to hand-edit `.lfm` for routine tasks.

## Open questions / risks

- **Source editing strategy:** how much to rely on pasls/CodeTools vs bespoke Pascal rewrites for safe `.pas` synchronization.
- **LCL behavior emulation:** degree of runtime-accurate layout/painting vs pragmatic design-time approximation.
- **Component ecosystem:** handling third-party/package components and unknown properties without data loss.
- **LFM dialect/edge cases:** binary streams, inline objects, legacy constructs across Lazarus/FPC versions.
- **Scope control:** avoid a multi-year parity trap; prioritize workflows that unlock real users first.

## References

- Spec: `docs/superpowers/specs/2026-11-14-lazarus-form-designer-design.md`
- Initial plan: `docs/superpowers/plans/2026-11-14-lazarus-form-designer.md`
- Current implementation: `crates/lazarus_designer/`, `crates/toolbar/src/toolbar.rs`
