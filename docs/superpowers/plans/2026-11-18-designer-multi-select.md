# Designer Multi-Select Implementation Plan

> Next slice of `docs/superpowers/plans/2026-11-17-lazarus-designer-roadmap.md`
> (phase 2, "multi-select"). Steps use `- [ ]` for tracking.

**Goal:** Select several components, move/delete/duplicate them as a group, leaving the
inspector bound to the primary selection.

**Crate:** `crates/lazarus_designer/` (`canvas.rs`, `designer_view.rs`, `palette.rs`).

**Constraints:**

- `.lfm` still stores components individually in source order.
- A group drag is one undo step.
- Group duplicate names must all be unique, including against each other.

## Design

- Replace `selection: Option<usize>` with a `Selection` value: a `BTreeSet<usize>` of
  indices plus a `primary` index. `primary` drives the inspector and resize handles.
- Selection helpers filter out indices that no longer exist after deletes/undo.
- `DragState` records every selected component's starting bounds. A move applies the
  same snapped delta to all of them; resize stays primary-only for now.
- `delete_selected` removes all selected indices from the end; `duplicate_selected`
  copies each one (offset +8 px) and selects the copies.

## Tasks

- [ ] Introduce `Selection` and migrate canvas/view call sites.
- [ ] Shift-click toggles membership; clicking an unselected component selects it alone.
- [ ] Group move with snapping, group delete and group duplicate, each undoable as one
      step.
- [ ] Inspector shows and edits the primary component only.
- [ ] Tests: toggle/primary semantics, group move offsets and undo, delete/duplicate
      groups, invalid indices filtered after undo.

## Deferred

- Marquee selection, group resize and alignment/distribution commands.
