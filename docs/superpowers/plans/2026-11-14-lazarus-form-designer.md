# Lazarus Form Designer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a native GPUI visual form designer for Pascal/Lazarus projects in Phocida (canvas, palette, drag-drop, inspector, `.lfm` round-trip).

**Architecture:** New crate `crates/lazarus_designer`: pure LFM parse/serialize + component model first (tasks 1–2), then GPUI canvas/palette/drag-drop/inspector UI composing on top (tasks 3–5), then workspace integration (task 6).

**Tech Stack:** Rust, GPUI (gpui crate), existing `languages` LFM grammar, `lazarus_project` parser, Zed-style element APIs.

**Spec:** `docs/superpowers/specs/2026-11-14-lazarus-form-designer-design.md`

## Global Constraints

- LFM (`.lfm`) is the source of truth; designer edits must round-trip back to it.
- Pascal language server / pasls must not be required for designer operations.
- No editor behavior regressions; `cargo test -p <crate>` and `cargo build -p zed -j 2` at the end.
- Platform: Windows (Win32), GPUI constraints of the fork apply.

## Review Focus

- Task 1: LFM parse of component bounds — a malformed LFM must not panic.
- Task 2: component model must preserve order + properties exactly.
- Task 3: canvas zoom/pan math with negative coordinates.
- Task 4: drag-drop coordinates must map to LFM coordinate space.
- Task 5: inspector edits must produce valid Pascal property values.

---

### Task 1: LFM parse/serialize core

**Files:**
- Create: `crates/lazarus_designer/src/lfm.rs`
- Test: `crates/lazarus_designer/src/lfm_tests.rs` (or inline `#[cfg(test)]`)

**Interfaces:**
- Produces: `pub fn parse_lfm(text: &str) -> Result<LfmDocument>`,
  `pub fn serialize_lfm(&LfmDocument) -> String`.

- [ ] **Step 1: Write the failing test**

```rust
#[test] fn parses_component_bounds() { /* sample .lfm with TButton at 10,20 size 100,30 */ }
#[test] fn malformed_lfm_is_error_not_panic() { /* truncated xml */ }
```

- [ ] **Step 2: Run test to verify it fails** — FAIL: module missing.

- [ ] **Step 3: Implement `lfm.rs`** — minimal XML-ish LFM parse (Lazarus LFM is XML-ish: `<Form ...><Component .../></Form>`).

- [ ] **Step 4: Run test to verify it passes** — PASS.

- [ ] **Step 5: Commit** — `feat: add lfm parse/serialize core for designer`.

### Task 2: Component model

**Files:**
- Create: `crates/lazarus_designer/src/model.rs`

**Interfaces:**
- Produces: `pub struct DesignerComponent { pub name: String, pub class: String, pub bounds: (f32, f32, f32, f32), pub properties: HashMap<String, String> }`,
  `pub fn components_of(doc: &LfmDocument) -> Vec<DesignerComponent>`.

- [ ] **Step 1: Write the failing test**

```rust
#[test] fn extracts_components_with_order() { /* two components, names preserved */ }
```

- [ ] **Step 2: Run test to verify it fails** — FAIL.

- [ ] **Step 3: Implement `model.rs`** from `LfmDocument`.

- [ ] **Step 4: Run test to verify it passes** — PASS.

- [ ] **Step 5: Commit** — `feat: add designer component model`.

### Task 3: Canvas element skeleton (GPUI)

**Files:**
- Create: `crates/lazarus_designer/src/canvas.rs`
- Create: `crates/lazarus_designer/src/lib.rs` (exports)

**Interfaces:**
- Produces: `pub struct FormCanvas { /* state: zoom, offset, selection */ }`,
  `impl Render for FormCanvas`.

- [ ] **Step 1: Write the failing test** — GPUI headless render test if feasible (`gpui::test`); else compile-only check.

- [ ] **Step 2: Run test to verify it fails.**

- [ ] **Step 3: Implement canvas render (background grid, components as rects, selection).**

- [ ] **Step 4: Run tests.**

- [ ] **Step 5: Commit** — `feat: add designer canvas element`.

### Task 4: Palette + drag-drop

**Files:**
- Modify: `crates/lazarus_designer/src/canvas.rs` (or new `palette.rs`)

**Interfaces:**
- Produces: `pub fn palette_components() -> Vec<ComponentDescriptor>`,
  `on_drop(component: ComponentDescriptor, bounds: (f32,f32,f32,f32)) -> DesignerComponent`.

- [ ] **Step 1: Write the failing test** — drop creates component with default bounds.

- [ ] **Step 2: Run test to verify it fails.**

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run test.**

- [ ] **Step 5: Commit** — `feat: add palette and drop placement`.

### Task 5: Inspector property editing

**Files:**
- Modify: `crates/lazarus_designer/src/model.rs`, `crates/lazarus_designer/src/inspector.rs` (new)

**Interfaces:**
- Produces: `pub fn set_property(&mut self, component: &mut DesignerComponent, key: &str, value: &str)`.

- [ ] **Step 1: failing test** — set property updates map.

- [ ] **Step 2: run — fail.**

- [ ] **Step 3: implement.**

- [ ] **Step 4: run — pass.**

- [ ] **Step 5: commit** — `feat: add designer inspector`.

### Task 6: Workspace integration

**Files:**
- Modify: `crates/languages/src/lib.rs` (register designer entry point, e.g. context menu / command)
- Modify: `crates/toolbar/src/toolbar.rs` or command palette (Design action)

**Interfaces:**
- Produces: action `toolbar::DesignProject` invoking designer for active project.

- [ ] **Step 1: failing test** — action exists / opens designer for `.lpr` buffer.

- [ ] **Step 2: run — fail.**

- [ ] **Step 3: implement wiring.**

- [ ] **Step 4: run — pass.**

- [ ] **Step 5: commit** — `feat: wire designer into workspace actions`.

---

Plan self-check: coverage of spec goals OK (canvas/palette/drop/inspector/round-trip covered by tasks 1–6). Review focus items map to tasks 1–5.

---

**Plan complete and saved to `docs/superpowers/plans/2026-11-14-lazarus-form-designer.md`. Please review the plan. Does it capture what you want?**

Execution method:
- **Subagent-driven** — fresh subagent per task + reviewer per task.
- **Native** — I implement task-by-task here, one fresh reviewer on the branch at the end.
