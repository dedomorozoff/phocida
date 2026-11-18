# Designer Multi-Select Implementation Plan

> Next slice of `docs/superpowers/plans/2026-11-17-lazarus-designer-roadmap.md`
> (phase 2, "multi-select"). Steps use `- [ ]` for tracking.

**Goal:** Select several components, move/delete/duplicate them as a group, leaving the
inspector bound to the primary selection.

**Crate:** `crates/lazarus_designer/` (`canvas.rs`, `canvas_tests.rs`, `designer_view.rs`,
`palette.rs`).

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

## Task 1: Introduce the `Selection` value

**Files:** `crates/lazarus_designer/src/canvas.rs`, `canvas_tests.rs`, `designer_view.rs`,
`palette.rs` (tests only).

### Steps

- [x] Add these tests to `canvas_tests.rs` first, importing `Selection`. They must fail
      to compile because the type does not exist yet.

```rust
#[test]
fn selection_starts_empty() {
    let selection = Selection::new();
    assert!(selection.is_empty());
    assert_eq!(selection.primary(), None);
    assert_eq!(selection.len(), 0);
}

#[test]
fn selecting_replaces_membership_and_sets_primary() {
    let mut selection = Selection::new();
    selection.select(3);
    assert_eq!(selection.primary(), Some(3));
    assert!(selection.contains(3));
    assert_eq!(selection.indices().collect::<Vec<_>>(), vec![3]);

    selection.select(1);
    assert_eq!(selection.primary(), Some(1));
    assert!(!selection.contains(3), "select replaces the whole membership");
    assert_eq!(selection.len(), 1);
}

#[test]
fn toggling_adds_a_secondary_and_removes_it_again() {
    let mut selection = Selection::new();
    selection.select(0);
    selection.toggle(2);
    assert_eq!(selection.primary(), Some(2), "toggled-in index becomes primary");
    assert_eq!(selection.indices().collect::<Vec<_>>(), vec![0, 2]);

    selection.toggle(2);
    assert!(!selection.contains(2));
    assert_eq!(selection.primary(), Some(0), "primary falls back to a member");
    assert_eq!(selection.len(), 1);

    selection.toggle(0);
    assert!(selection.is_empty());
    assert_eq!(selection.primary(), None);
}

#[test]
fn stale_indices_are_filtered() {
    let mut selection = Selection::new();
    selection.select(1);
    selection.toggle(4);

    selection.retain_within(2);

    assert_eq!(selection.indices().collect::<Vec<_>>(), vec![1]);
    assert_eq!(selection.primary(), Some(1));
    assert!(selection.contains(1));
}
```

- [x] Run `cargo test -p lazarus_designer selection_` and confirm it fails because
      `Selection` is missing. Expected: `cannot find type Selection`.

- [x] Add `Selection` to `canvas.rs` near `DragState`:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    indices: BTreeSet<usize>,
    primary: Option<usize>,
}

impl Selection {
    pub fn new() -> Self { Self::default() }
    pub fn is_empty(&self) -> bool { self.indices.is_empty() }
    pub fn len(&self) -> usize { self.indices.len() }
    pub fn primary(&self) -> Option<usize> { self.primary }
    pub fn contains(&self, index: usize) -> bool { self.indices.contains(&index) }
    pub fn indices(&self) -> impl Iterator<Item = usize> + '_ { self.indices.iter().copied() }
    pub fn clear(&mut self) { self.indices.clear(); self.primary = None; }
    pub fn select(&mut self, index: usize) { self.indices.clear(); self.indices.insert(index); self.primary = Some(index); }
    pub fn toggle(&mut self, index: usize) {
        if self.indices.remove(&index) {
            if self.primary == Some(index) { self.primary = self.indices.iter().next_back().copied(); }
        } else {
            self.indices.insert(index);
            self.primary = Some(index);
        }
    }
    pub fn set_primary(&mut self, index: usize) {
        if self.indices.contains(&index) { self.primary = Some(index); }
    }
    pub fn retain_within(&mut self, len: usize) {
        self.indices.retain(|index| *index < len);
        if let Some(primary) = self.primary {
            if primary >= len { self.primary = self.indices.iter().next_back().copied(); }
        } else {
            self.primary = if self.indices.is_empty() { None } else { self.indices.iter().next_back().copied() };
        }
    }
}
```

- [x] Run `cargo test -p lazarus_designer selection_`. Expected: all four tests pass.

- [x] Migrate `FormCanvas`:
  - `pub selection: Selection`, initialized with `Selection::new()`.
  - `CanvasSnapshot.selection: Selection`; snapshot clones it.
  - `restore` calls `selection.retain_within(self.components.len())`.
  - `set_components` clears the selection.
  - Every `let Some(index) = self.selection` becomes
    `let Some(index) = self.selection.primary()`.
  - Every `self.selection = Some(index)` becomes `self.selection.select(index)`;
    `None` becomes `self.selection.clear()`.
  - Delete: set membership to the clamped index, or clear when empty.
  - `select(&mut self, selection: Option<usize>)` delegates to `Selection::select` /
    `clear`. Keep the signature so `palette.rs` tests stay valid.
  - `start_drag` selects the index alone when it is not already a member, otherwise
    only makes it primary.
  - Render uses `self.selection.contains(index)`; resize handles stay on
    `self.selection.primary() == Some(index)`.
- [x] Replace `.selection` reads in `designer_view.rs`:
  - `canvas.selection?` becomes `canvas.selection.primary()?`
  - `let selection = self.canvas.read(cx).selection;` becomes
    `.selection.primary()` (both in `sync_inspector` and `render`).
  - `apply_inspector` reads `.selection.primary()`.
  - Test at line ~1295 uses `.selection.primary()`.
- [x] Update test assertions in `canvas_tests.rs`, `palette.rs`, `designer_view.rs` from
      `canvas.selection` / `selection == Some(n)` to `canvas.selection.primary()`.
- [x] Run `cargo test -p lazarus_designer`. Expected: all tests pass; `selection_`
      tests plus the migrated suite green.

## Task 2: Shift-click selection in the canvas

**Files:** `crates/lazarus_designer/src/canvas.rs`, `canvas_tests.rs`.

### Steps

- [x] Add failing tests to `canvas_tests.rs`:

```rust
#[test]
fn clicking_an_unselected_component_selects_it_alone() {
    let mut canvas = stacked_canvas();
    canvas.select(Some(0));
    canvas.press_component(2, false, point(px(0.0), px(0.0)));
    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![2]);
    assert_eq!(canvas.selection.primary(), Some(2));
}

#[test]
fn clicking_a_selected_component_makes_it_primary_without_dropping_the_group() {
    let mut canvas = stacked_canvas();
    canvas.select(Some(0));
    canvas.toggle_selection(2);
    canvas.press_component(0, false, point(px(0.0), px(0.0)));
    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0, 2]);
    assert_eq!(canvas.selection.primary(), Some(0));
}

#[test]
fn shift_click_toggles_membership_and_starts_no_drag() {
    let mut canvas = stacked_canvas();
    canvas.select(Some(0));

    canvas.press_component(2, true, point(px(0.0), px(0.0)));
    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0, 2]);
    assert!(canvas.drag().is_none());

    canvas.press_component(2, true, point(px(0.0), px(0.0)));
    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0]);

    canvas.press_component(0, true, point(px(0.0), px(0.0)));
    assert!(canvas.selection.is_empty());
    assert!(canvas.drag().is_none());
}
```

- [x] Run `cargo test -p lazarus_designer click` and confirm it fails with
      `no method named press_component`.

- [x] Add to `FormCanvas`:

```rust
pub fn toggle_selection(&mut self, index: usize) {
    if index < self.components.len() {
        self.selection.toggle(index);
    }
}

/// Handles a left mouse-down on a component: shift toggles membership,
/// a plain click on an unselected component selects it alone, and a plain
/// click on a selected component makes it primary. Plain clicks also start
/// a move drag; shift-click never starts one.
pub fn press_component(&mut self, index: usize, shift: bool, mouse: Point<Pixels>) {
    if index >= self.components.len() {
        return;
    }
    if shift {
        self.selection.toggle(index);
        return;
    }
    if !self.selection.contains(index) {
        self.selection.select(index);
    } else {
        self.selection.set_primary(index);
    }
    self.start_drag(index, DragMode::Move, mouse);
}
```

- [x] Change `start_drag` to preserve an existing multi-selection:

```rust
pub fn start_drag(&mut self, index: usize, mode: DragMode, mouse: Point<Pixels>) {
    if index >= self.components.len() { return; }
    if !self.selection.contains(index) {
        self.selection.select(index);
    } else {
        self.selection.set_primary(index);
    }
    self.drag_origin = Some(self.snapshot());
    self.drag = Some(DragState { index, mode, start_mouse: mouse, start_bounds: self.components[index].bounds });
}
```

- [x] In `render`, replace the component's `start_drag(index, DragMode::Move,
      event.position)` with
      `this.press_component(index, event.modifiers.shift, event.position)`.
      Resize handles keep calling `start_drag` with their primary index.
- [x] Run `cargo test -p lazarus_designer`. Expected: green.

## Task 3: Group move, delete and duplicate

**Files:** `crates/lazarus_designer/src/canvas.rs`, `canvas_tests.rs`,
`designer_view.rs`.

### Steps

- [x] Add failing tests:

```rust
fn two_button_canvas() -> FormCanvas {
    // A at (10, 20, 100, 30) and B at (150, 60, 40, 20), both selected.
    // select(0), toggle_selection(1)
}

#[test]
fn group_drag_moves_every_selected_component_by_the_same_delta() { ... }

#[test]
fn group_drag_is_one_undo_step() {
    // drag both, end, undo -> both original, one history entry
}

#[test]
fn group_move_snaps_the_primary_and_keeps_relative_offsets() {
    // drag primary by 21,9 with snap -> primary at (32,32), secondary at +delta
}

#[test]
fn group_delete_removes_all_selected_and_one_undo_restores_them() { ... }

#[test]
fn group_duplicate_copies_each_selected_with_unique_names() {
    // select 0 and 1, duplicate -> names Button3/Edit2 style all distinct,
    // including against each other, copies offset by 8 and selected.
}

#[test]
fn group_duplicate_is_one_undo_step() { ... }
```

- [x] Run `cargo test -p lazarus_designer group_` and confirm failures are about
      missing group behavior (second component not moving, only one deleted, etc.).

- [x] Add `group: Vec<(usize, Bounds)>` to `DragState`, filled in `start_drag` from
      `self.selection.indices()` with each component's starting bounds (primary
      included; for `DragMode::Resize` it may contain only the primary). Update the
      `DragState` construction and any struct literal.
- [x] In `update_drag`:

```rust
DragMode::Move => {
    let (start_left, start_top, ..) = drag.start_bounds;
    let mut target_left = start_left + dx;
    let mut target_top = start_top + dy;
    if self.snap_to_grid {
        target_left = snap_bound(target_left);
        target_top = snap_bound(target_top);
    }
    let delta = (target_left - start_left, target_top - start_top);
    for (index, bounds) in &drag.group {
        if let Some(component) = self.components.get_mut(*index) {
            component.bounds.0 = bounds.0 + delta.0;
            component.bounds.1 = bounds.1 + delta.1;
            sync_bounds_to_properties(component);
        }
    }
    return;
}
```

  Keep the existing single-component resize branch.
- [x] Rewrite `delete_selected` to delete all selected indices from the end after one
      `checkpoint()`, then clamp `primary` to the remaining length and reduce the
      membership to that index (or clear when empty).
- [x] Change `duplicate_selected` to return `Vec<usize>`: copy every selected index in
      ascending order, offset +8, collecting names into a growable list so each copy is
      unique against the originals and the earlier copies. Select all copies and make
      the primary the copy of the primary component. Return the new indices.
- [x] In `designer_view::duplicate_selected`, replace `.is_some()` with
      `!new_indices.is_empty()`.
- [x] Run `cargo test -p lazarus_designer`. Expected: green.

## Task 4: Inspector follows the primary only

**Files:** `crates/lazarus_designer/src/designer_view.rs`.

### Steps

- [ ] Add a test in the `designer_view` test module:

```rust
#[gpui::test]
async fn inspector_follows_the_primary_of_a_group_selection(cx: &mut TestAppContext) {
    // open a designer over SAMPLE_LFM, place a second component,
    // select both, make the first primary, assert selected_component()
    // returns the first and inspector_for tracks the primary.
}
```

- [ ] Run `cargo test -p lazarus_designer inspector_follows` and confirm it fails
      (group selection either lost or inspector rebuilt for the wrong index).
- [ ] Ensure `selected_component`, `sync_inspector`, `apply_inspector` and `render` all
      read `selection.primary()` (Task 1 already migrates them; this task proves it).
      Rebuilding the inspector must compare `inspector_for` with the primary, not the
      whole membership.
- [ ] Run `cargo test -p lazarus_designer`. Expected: green.

## Task 5: Selection staleness and no-op edges

**Files:** `crates/lazarus_designer/src/canvas.rs`, `canvas_tests.rs`.

### Steps

- [ ] Add failing tests:

```rust
#[test]
fn undo_filters_selection_indices_that_no_longer_exist() { ... }

#[test]
fn group_operations_without_a_selection_are_no_ops() {
    // delete_selected false, duplicate_selected empty, no history
}
```

- [ ] Run `cargo test -p lazarus_designer`. Expected: the two new tests fail.
- [ ] Ensure `restore` and `set_components` call `Selection::retain_within`, and that
      `delete_selected`/`duplicate_selected` return early without checkpointing when
      the selection is empty.
- [ ] Run `cargo test -p lazarus_designer`. Expected: all tests pass.

## Deferred

- Marquee selection, group resize and alignment/distribution commands.
