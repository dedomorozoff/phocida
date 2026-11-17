# Designer Viewport Navigation Implementation Plan

> Next slice of `docs/superpowers/plans/2026-11-17-lazarus-designer-roadmap.md`
> (phase 2, "zoom/pan"). Steps use `- [ ]` for tracking.

**Goal:** Make the canvas navigable on forms larger than the window: pan by middle-drag
and wheel, zoom to fit, and a 100% view that centers the form.

**Crate:** `crates/lazarus_designer/` (`canvas.rs`, `designer_view.rs`).

**Constraints:**

- Viewport state is not part of undo/redo or the `.lfm`.
- Zoom stays clamped to 25%..400%.

## Design

- `FormCanvas` gains `viewport: (f32, f32)` measured from a full-size probe child via
  `on_children_prepainted`; reading the child bounds avoids hardcoding pane sizes.
- `pan_by`, `zoom_at(cursor, factor)`, `zoom_to_fit` and `reset_view` own the view math,
  so the UI only forwards input.
- Middle-drag pans; the wheel pans; `Ctrl`/`Cmd` + wheel zooms around the cursor.
- The probe writes viewport size without notifying, so prepaint stays side-effect free
  for rendering.

## Tasks

### Task 1: Viewport measurement

- [x] Add a probe child and record its size as `viewport`; ignore non-positive sizes.
- [x] GPUI test: rendering the canvas records a non-zero viewport.

### Task 2: Pan

- [x] Middle-drag pan (`start_pan` / `update_pan` / `end_pan`).
- [x] Wheel pan: pixels or lines converted to pixels, horizontal delta respected.
- [x] Tests: middle-drag offset math, wheel direction.

### Task 3: Zoom-to-fit and 100% view

- [x] `zoom_at` keeps the LFM point under the cursor fixed.
- [x] `zoom_to_fit` clamps to the zoom range, pads by 24 px and centers the form.
- [x] `reset_view` sets 100% and centers the form.
- [x] Toolbar `Fit` button and `Ctrl+Shift+0`; `Ctrl+0` now resets and centers.
- [x] Tests: cursor-anchored zoom, fit math, reset centering.

## Review focus

- Zooming at a cursor must not drift when clamped at 25% or 400%.
- Pan must not fight an active move/resize drag: a left-button drag wins.
- Undo/redo must not change zoom, offset or viewport.

## Deferred

- Snap-to-grid, rulers, marquee and multi-select are separate slices.
