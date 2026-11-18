# Designer marquee selection

**Intent:** The multi-select slice lets a user build a group only by shift-clicking. A
designer expects to drag a rectangle over empty canvas and select everything it touches.
This is the first deferred item of
`docs/superpowers/plans/2026-11-18-designer-multi-select.md`.

**Goals:**

- Dragging on empty canvas draws a translucent rectangle and selects every component whose
  pixel rect intersects it.
- Shift-drag adds the hits to the current selection; a plain drag replaces it.
- A background click with no drag clears the selection (shift-click keeps it).
- Works under zoom and pan; the rectangle is drawn and tested in canvas pixels.
- No `.lfm` change and no undo entry: selection is not document state.

**Non-goals / anti-goals:**

- Group resize and alignment/distribution commands (later slices from the same deferred
  list).
- Containment-only selection, auto-scroll while dragging, or marquee over the palette.
- Changing how shift-click builds a selection.

**Constraints:**

- Component mouse-downs and resize handles call `stop_propagation`, so only empty canvas
  reaches the marquee handler.
- The existing left-drag (`DragMode`) and middle-drag pan must keep working.
- `.lfm` round-trip and save/reload are untouched.

**Approaches:**

1. **Pixel-space marquee (chosen).** Store `start`/`current` in canvas pixels, build a
   `PixelRect`, and intersect it with each `component_pixel_rect`. Pros: matches the render
   space and the existing drag math, so zoom/pan are correct for free. Cons: none
   meaningful at this scale.
2. **LFM-space marquee.** Convert the pointer to LFM units and compare with
   `component.bounds`. Pros: rectangle values are in form units. Cons: extra conversion in
   both directions, and the overlay still has to be drawn in pixels.

**Behavior details:**

- A drag shorter than 2 px on both axes counts as a click.
- Intersection is inclusive of touching edges and ignores components entirely outside.
- The primary of a marquee result is the highest selected index (frontmost in paint order);
  an additive marquee raises the highest newly added index to primary.
- `finish_marquee` always clears the in-progress rectangle, even when nothing is selected.

**Verification:** unit tests for rectangle selection, replace/additive/click semantics and
zoomed coordinates; a GPUI test for the overlay bounds; a GPUI interaction test that drags
on the background and asserts the resulting selection.
