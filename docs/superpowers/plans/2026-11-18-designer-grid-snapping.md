# Designer Grid Snapping Implementation Plan

> Next slice of `docs/superpowers/plans/2026-11-17-lazarus-designer-roadmap.md`
> (phase 2, "snap-to-grid"). Steps use `- [ ]` for tracking.

**Goal:** Align moved and resized components to the visible 8 px grid, with a toggle for
free positioning, matching the Lazarus default.

**Crate:** `crates/lazarus_designer/` (`canvas.rs`, `designer_view.rs`).

**Constraints:**

- Snapping is a view/interaction setting; it never rewrites `.lfm` beyond the bounds
  already synced from a drag.
- Rounding must be deterministic and independent of zoom.

## Design

- `FormCanvas` gains `snap_to_grid: bool` (default on) and `GRID_SNAP: f32`.
- `update_drag` rounds the resulting `left`/`top` (Move) or the dragged edges
  (Resize) to the grid after applying the mouse delta. Rounding happens in LFM units, so
  the same drag lands on the same coordinates at any zoom.
- Toggling snapping keeps existing components untouched.
- The toolbar gets a `Snap` button that reflects the current state.

## Tasks

- [x] Add the snap toggle plus `snap_value`/edge snapping in `update_drag`.
- [x] Toolbar `Snap` button.
- [x] Tests: move snaps to the nearest grid line, resize snaps the moved edge, a drag
      with snapping off keeps sub-grid positions.

## Review focus

- Resizing from the north/west edges must keep the opposite edge fixed when snapping.
- A snap must not collapse a component below the 8 px minimum size.
