# Designer Editing Basics Implementation Plan

> Focused slice of the roadmap in `docs/superpowers/plans/2026-11-17-lazarus-designer-roadmap.md`
> (phase 2, "daily-driver basics"). Steps use `- [ ]` for tracking.

**Goal:** Make routine canvas editing safe and fast: undo/redo for every designer edit,
delete and duplicate of the selected component, and zoom controls.

**Crate:** `crates/lazarus_designer/` (`canvas.rs`, `palette.rs`, `designer_view.rs`).

**Constraints:**

- `.lfm` stays the source of truth; no new persisted state.
- Undo/redo is designer-scoped and in-memory only.
- No view/editor regressions; `cargo test -p lazarus_designer` must pass.

## Design

Undo/redo uses whole-canvas snapshots (`components` + `selection`), not per-command
deltas. The designer state is small (a form has tens of components), so snapshots keep
the implementation simple and correct, including drag and inspector apply.

- `FormCanvas` owns `history` and `redo_stack` of `CanvasSnapshot`.
- `checkpoint()` records the current state before a mutation and clears the redo stack.
  It skips recording when nothing changed, so a click without a drag does not create a
  history entry and does not discard redo.
- `start_drag` checkpoints once, so an entire move/resize is a single undo step.
- `set_components` (reload) clears history.

## Tasks

### Task 1: Undo/redo in the canvas

**Files:** `crates/lazarus_designer/src/canvas.rs`, `src/canvas_tests.rs`

- [x] Add `CanvasSnapshot`, `history`, `redo_stack`, and the `checkpoint`, `undo`,
      `redo`, `can_undo`, `can_redo` methods.
- [x] Checkpoint before drag, placement, delete, duplicate, and inspector apply.
- [x] Tests: undo restores bounds, redo reapplies, a click without movement is not
      undoable, history is bounded, reload clears history.

### Task 2: Delete and duplicate

**Files:** `crates/lazarus_designer/src/canvas.rs`, `src/palette.rs`, `src/designer_view.rs`

- [x] `FormCanvas::delete_selected` removes the component and clamps the selection.
- [x] `FormCanvas::duplicate_selected` clones the component, offsets it by 8 px and
      gives it a fresh name (`Button1` -> next free `ButtonN`).
- [x] Designer toolbar buttons and `Delete` / `Ctrl+D` shortcuts.
- [x] Tests: deletion keeps other components, duplicate name is unique and properties
      are copied, undo restores a deleted component.

### Task 3: Zoom and shortcuts

**Files:** `crates/lazarus_designer/src/canvas.rs`, `src/designer_view.rs`

- [x] `zoom_in`, `zoom_out`, `reset_zoom` clamped to 25%..400% with multiplicative steps.
- [x] Toolbar buttons plus `Ctrl+=`, `Ctrl+-`, `Ctrl+0`.
- [x] Keyboard routing: the canvas focuses the designer on mouse down; the view handles
      `Ctrl+Z`, `Ctrl+Shift+Z` / `Ctrl+Y`, `Ctrl+D`, `Delete`.
- [x] Tests: zoom clamps, undo/redo restore components without touching zoom.

## Deferred

- Zoom-to-fit needs the canvas viewport size; the canvas does not measure its own
  bounds yet. Follow-up: track the viewport and add `Ctrl+Shift+0`.
- Pan (middle-drag / space-drag) and marquee selection stay in the next slice.

## Review focus

- Undo must never resurrect a stale selection index (`selection` is clamped on restore).
- Keyboard shortcuts must not fire while an inspector text field has focus.
- Duplicate names must be unique case-insensitively, matching Pascal identifier rules.
