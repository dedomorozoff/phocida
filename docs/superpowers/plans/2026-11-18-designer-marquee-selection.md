# Designer Marquee Selection Implementation Plan

> Next slice after `docs/superpowers/plans/2026-11-18-designer-multi-select.md`
> (deferred list: marquee selection, group resize, alignment/distribution).
> Design: `docs/superpowers/specs/2026-11-18-designer-marquee-selection-design.md`.

**Goal:** Drag a rectangle on empty canvas to select every component it intersects, with
shift to add and a plain click to clear.

**Crate:** `crates/lazarus_designer/` (`canvas.rs`, `canvas_tests.rs`).

**Constraints:**

- Marquee geometry and hit-testing are canvas pixels, so zoom/pan are correct.
- Selection is not document state: no checkpoint, no undo entry, no `.lfm` change.
- Existing component drag, middle-drag pan and shift-click must not regress.

## Task 1: Marquee state and rectangle selection

### Steps

- [x] Add failing unit tests to `canvas_tests.rs`:

```rust
fn unselected_two_button_canvas() -> FormCanvas {
    // Same components as two_button_canvas(), no selection.
}

#[test]
fn marquee_selects_intersecting_components() { /* drag (0,0)..(120,60) -> [0] */ }

#[test]
fn marquee_replaces_the_current_selection() { /* select 1, drag over 0 -> [0] */ }

#[test]
fn shift_marquee_adds_to_the_selection() { /* select 1, additive over 0 -> [0,1], primary 0 */ }

#[test]
fn marquee_click_clears_the_selection_but_shift_click_keeps_it() { ... }

#[test]
fn marquee_ignores_components_outside_the_rectangle() { ... }

#[test]
fn marquee_hit_tests_in_canvas_pixels_under_zoom_and_pan() {
    // zoom 2, offset (100,100): a rect (0,0)..(119,139) misses Button1,
    // while (0,0)..(125,145) hits it.
}
```

- [x] Run `cargo test -p lazarus_designer marquee_` and confirm a compile error for the
      missing methods.
- [x] Add to `canvas.rs`:

```rust
#[derive(Debug, Clone, Copy)]
struct MarqueeState {
    start: Point<Pixels>,
    current: Point<Pixels>,
    additive: bool,
}

// on FormCanvas
marquee: Option<MarqueeState>,

pub fn start_marquee(&mut self, mouse: Point<Pixels>, additive: bool)
pub fn update_marquee(&mut self, mouse: Point<Pixels>)
pub fn is_marqueeing(&self) -> bool
pub fn marquee_rect(&self) -> Option<PixelRect>
pub fn finish_marquee(&mut self) -> bool
fn selection_in_rect(&self, rect: PixelRect) -> Vec<usize>
```

- [x] Implement `PixelRect::intersects` (inclusive of shared edges) and
      `selection_in_rect` over `component_pixel_rect(component.bounds, self.zoom, self.offset)`.
- [x] `finish_marquee`: take the state; a rect under `2.0` px in both axes is a click
      (plain click clears, additive click keeps). Otherwise replace with the hits, or
      toggle each hit for an additive drag. Ends with the highest result index as primary.
- [x] Run `cargo test -p lazarus_designer marquee_`. Expected: green.

## Task 2: Marquee overlay

### Steps

- [x] Add a failing GPUI test:

```rust
#[gpui::test]
fn marquee_renders_an_overlay_while_dragging(cx: &mut TestAppContext) {
    // start at (0,0), update to (50,40), notify, run_until_parked,
    // debug_bounds("designer-marquee") is 50x40 at (0,0).
}
```

- [x] Run it and confirm `debug_bounds("designer-marquee")` is `None`.
- [x] At the end of `FormCanvas::render`, append when `marquee_rect()` is `Some`:

```rust
div()
    .id("designer-marquee")
    .absolute()
    .left(px(rect.left))
    .top(px(rect.top))
    .w(px(rect.width.max(1.0)))
    .h(px(rect.height.max(1.0)))
    .bg(gpui::rgba(0x2f6feb33))
    .border_1()
    .border_color(gpui::rgb(0x2f6feb))
```

- [x] Run the test. Expected: green.

## Task 3: Wire canvas mouse input

### Steps

- [x] Add a failing GPUI interaction test:

```rust
#[gpui::test]
fn dragging_on_the_background_selects_components(cx: &mut TestAppContext) {
    // simulate_mouse_down((0,0), Left); simulate_mouse_move((120,60), Some(Left));
    // simulate_mouse_up((120,60), Left); assert selection [0].
}
```

- [x] Run it and confirm the selection stays empty.
- [x] In the canvas `on_mouse_down(Left)` handler, after focusing, call
      `this.start_marquee(event.position, event.modifiers.shift)`.
- [x] In `on_mouse_move`, add a branch: if `this.is_marqueeing()`, `update_marquee` and
      notify (before the pan branch, after the drag branch).
- [x] In both `on_mouse_up(Left)` and `on_mouse_up_out(Left)`, finish an active marquee
      and notify.
- [x] Run the test and then the full suite `cargo test -p lazarus_designer`. Expected:
      green.

## Deferred

- Group resize and alignment/distribution remain from the multi-select plan's deferred
  list.
