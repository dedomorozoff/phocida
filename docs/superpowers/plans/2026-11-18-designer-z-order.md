# Designer Z-Order Implementation Plan

> Next slice of `docs/superpowers/plans/2026-11-17-lazarus-designer-roadmap.md`
> (phase 2, "parenting/z-order"). Steps use `- [ ]` for tracking.

**Goal:** Let the designer change stacking order, which is the component order written to
`.lfm`, with undo support.

**Crate:** `crates/lazarus_designer/` (`canvas.rs`, `designer_view.rs`).

**Constraints:**

- Z-order is the order of `object` blocks in `.lfm`; reordering must round-trip.
- Reordering must be undoable and keep the same component selected.

## Design

- The canvas renders `components` in source order, so later entries paint on top.
- `raise_selected` / `lower_selected` swap with the neighbour; `bring_selected_to_front`
  and `send_selected_to_back` move to the ends.
- Each operation checkpoints first, returns `false` at the edge, and updates `selection`
  to follow the moved component.

## Tasks

- [x] Canvas operations for raise/lower/front/back with checkpoints.
- [x] Toolbar `Front` / `Back` buttons and `Ctrl+Shift+Up/Down`, `Ctrl+Up/Down`.
- [x] Tests: reordering, edge no-ops, selection follows, undo restores order.

## Review focus

- A no-op at the top of the stack must not create an undo entry.
- Selection must still point at the moved component after reordering.
