# `.lpi` Project Tasks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Opening an `.lpi` file in Phocida offers real lazbuild tasks — one per build mode, labelled by the project's title — driven by a native minimal parser of the `.lpi` XML.

**Architecture:** A small new crate `crates/lazarus_project` parses `.lpi`/`.lpk` text with `quick-xml` and exposes title/main unit/build modes. A dynamic context provider in `crates/languages/src/lpi.rs` turns the open `.lpi` buffer's text into `TaskTemplates` and is attached to the existing `lpi` language registration in `crates/languages/src/lib.rs`.

**Tech Stack:** Rust (toolchain pinned in `rust-toolchain.toml`), `quick-xml = "0.41"` (already a workspace dependency), existing `task::TaskTemplate` plumbing.

**Spec:** `docs/superpowers/specs/2026-11-06-lpi-project-design.md`

## Global Constraints

- Toolchain: build/run cargo through the pinned toolchain — plain `cargo ...` is enough on this checkout (`rust-toolchain.toml` pins 1.98.1); never pass `--release`.
- License header: the module under `crates/` is GPL-3 in `crates/languages/Cargo.toml`; the new crate copies that `License = "GPL-3.0-or-later"` style; do not introduce new license files.
- Parsing is text-in/text-out: no filesystem access, no process spawning, no time control inside `crates/lazarus_project`.
- Labels/copy follow the spec exactly: `build project '<title>' with lazbuild` and `build project '<title>' with lazbuild (<mode>)`.
- Do not modify the heuristic lazbuild/fpc templates in `pascal_task_context`.
- MSRV/GPUI types: provider code mirrors `TypeScriptContextProvider::associated_tasks` (same signatures, same `gpui` entity access patterns).

## Review Focus

1. `.lpi` with no `BuildModes` section at all (Lazarus did it once with tools) — parser must yield one mode `Default`; the plan's fixture omits `BuildModes` in a dedicated test. *Pinned by test `default_mode_without_build_modes` (Task 1).*
2. `.lpi` where the first `CONFIG` child is `PackageOptions` (a package file, `.lpk` shares the `lpi`/^`lpk` suffix in `path_suffixes`) — must parse as `LazarusFile::Package` and yield **no** lazbuild tasks. *Pinned by test `lpk_is_package` (Task 1) and no-task assertion (Task 2).*
3. `.lpi` saved by an old Lazarus version where `General/MainUnit` is missing but `Units/Unit0` exists — main unit falls back to index 0. *Pinned by test `main_unit_falls_back_to_zero` (Task 1).*
4. Title absent but main unit present — label uses the `.lpr` stem; expected `project1`. *Pinned by test `title_falls_back_to_main_unit` (Task 2).*
5. More than two build modes — every non-default mode gets its own task and its own `--build-mode` flag, no dedup/ordering surprises. *Pinned by test `three_modes_each_get_a_task` (Task 2).*

If any of these fails during review, the fix belongs in the owning task's file — not in a follow-up.

---

### Task 1: `crates/lazarus_project` — minimal `.lpi` parser

**Files:**
- Create: `crates/lazarus_project/Cargo.toml`
- Create: `crates/lazarus_project/src/lib.rs` (tests live in the same file, as is the project's style, e.g. `crates/languages/src/pascal.rs`)
- Modify: `Cargo.toml` (workspace root — two single-line additions)

**Interfaces:**
- Consumes: nothing (leaf crate; `quick-xml` from workspace deps).
- Produces (used verbatim by Task 2):
  ```rust
  pub enum LazarusFile { Project(LazarusProject), Package }
  pub struct LazarusProject {
      pub title: Option<String>,
      pub main_unit: Option<String>,   // e.g. Some("project1.lpr")
      pub build_modes: Vec<String>,    // e.g. ["Debug", "Release"] or ["Default"]
  }
  pub fn parse(input: &str) -> Result<LazarusFile>   // anyhow::Result
  ```

- [ ] **Step 1: Scaffold the crate**

`crates/lazarus_project/Cargo.toml`:

```toml
[package]
name = "lazarus_project"
version = "0.1.0"
edition.workspace = true
publish.workspace = true
license = "GPL-3.0-or-later"

[lints]
workspace = true

[lib]
doctest = false

[dependencies]
quick-xml.workspace = true
anyhow.workspace = true
```

Root `Cargo.toml`: add `"crates/lazarus_project",` to the `members` array (alphabetical position among the `crates/*` entries) and add to `[workspace.dependencies]`:

```toml
lazarus_project = { path = "crates/lazarus_project" }
```

- [ ] **Step 2: Write the failing tests**

In `crates/lazarus_project/src/lib.rs`, module-level tests covering, exactly:

1. `parses_real_project` — the fixture below parses as `LazarusFile::Project`, with `title == Some("app1")`, `main_unit == Some("project1.lpr")`, `build_modes == ["Debug", "Release"]`:

```rust
const FIXTURE: &str = r#"<?xml version="1.0"?><CONFIG>
  <ProjectOptions>
    <Version Value="12"/>
    <General>
      <MainUnit Value="0"/>
      <Title Value="app1"/>
      <ProjectType Value="Application"/>
    </General>
    <BuildModes Count="2" Active="0">
      <Item1 Name="Debug" Default="True"/>
      <Item2 Name="Release"/>
    </BuildModes>
    <Units Count="1">
      <Unit0>
        <Filename Value="project1.lpr"/>
        <IsPartOfProject Value="True"/>
      </Unit0>
    </Units>
  </ProjectOptions>
</CONFIG>"#;
```

2. `default_mode_without_build_modes` — same fixture with the `<BuildModes ...>...</BuildModes>` block deleted: `build_modes == ["Default"]`, `main_unit` still resolved.
3. `main_unit_falls_back_to_zero` — fixture with `General/MainUnit` element removed (Units intact): `main_unit == Some("project1.lpr")` (Unit0 by index 0).
4. `lpk_is_package` — minimal package fixture `<CONFIG><PackageOptions/></CONFIG>` (quoted `Version Value="11"` inside if wanted): result is `LazarusFile::Package` and needs no other fields.
5. `malformed_is_an_error` — input `"not xml at all"` returns `Err`.

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p lazarus_project` (fails earlier too — crate not registered; if so, run after completing Step 1).

Expected: compile error (`parse` not yet defined / cannot find crate).

- [ ] **Step 4: Implement `parse` with `quick-xml`**

Event-driven read (`quick_xml::Reader` over `input.as_bytes()`): track an element path stack of simple names (`CONFIG`, `ProjectOptions`, `General`, `BuildModes`, `Units`, `Unit{\d+}`, `Item{\d+}`, `Title`, `MainUnit`, `Filename`, `PackageOptions`) plus two small "last seen" state slots: the index of the `UnitN` node currently entered, and the `MainUnit` `Value`. Mapping mirrors the spec's three fields; `PackageOptions` as first `CONFIG` child short-circuits to `LazarusFile::Package`. Return `anyhow::bail!` on non-element garbage.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p lazarus_project`

Expected: PASS (all five).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/lazarus_project
git commit -m "parse the essentials of a Lazarus .lpi project file"
```

---

### Task 2: lazbuild task templates from a parsed project

**Files:**
- Create: `crates/languages/src/lpi.rs`
- Modify: `crates/languages/Cargo.toml` (add `lazarus_project.workspace = true` in `[dependencies]`)
- Test: `crates/languages/src/lpi.rs` (module tests)

**Interfaces:**
- Consumes (from Task 1, exact): `lazarus_project::{parse, LazarusFile, LazarusProject}`.
- Produces (used by Task 3):
  ```rust
  pub(super) struct LpiContextProvider;
  impl task::ContextProvider for LpiContextProvider {
      fn associated_tasks(&self, buffer: Option<Entity<Buffer>>, cx: &App)
          -> gpui::Task<Option<TaskTemplates>>;
  }
  pub(super) fn lazbuild_task_templates(parsed: &LazarusProject, relative_path: &str)
      -> TaskTemplates;
  ```

- [ ] **Step 1: Write the failing tests**

Module tests in `lpi.rs` call only `lazbuild_task_templates` (pure; no GPUI app needed), using a `LazarusProject { title: None→Some("app1"), main_unit: Some("project1.lpr"), build_modes: vec![...] }` and `relative_path = "src/project1.lpi"`:

1. `task_labels_match_spec` — modes `["Debug", "Release"]`: first template label is `build project 'app1' with lazbuild` with args `["src/project1.lpi"]` (no flag); second is `build project 'app1' with lazbuild (Release)` with args `["--build-mode=Release", "src/project1.lpi"]`; order is guaranteed as written.
2. `title_falls_back_to_main_unit` — `title: None`, `main_unit: Some("project1.lpr")`: both labels use `project1` (stem of the main unit).
3. `default_mode_yields_one_task` — `build_modes: ["Default"]`: exactly one template, no `--build-mode` flag anywhere.
4. `three_modes_each_get_a_task` — `["Default", "Nightly", "Release"]`: three templates in order; `Nightly` and `Release` each get their own `--build-mode=<name>` first arg.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p languages lpi`

Expected: FAIL — module doesn't exist.

- [ ] **Step 3: Implement the provider and templates**

`lazbuild_task_templates` maps `build_modes` to `TaskTemplate`s: index 0 plain, the rest prefixed with `--build-mode=<mode>`. Label fallback for `title: None` uses `main_unit` stem (strip extension, e.g. `Path::new(main_unit).file_stem()`), trailing fallback `project` if that is `None` too.

`LpiContextProvider::associated_tasks` mirrors `TypeScriptContextProvider::associated_tasks` (`crates/languages/src/typescript.rs:~impl ContextProvider`): take buffer text (`buffer.read(cx).text().to_string()`), run `parse`, return `Task::ready(Some(...))`; `Package`/`Err`/`None` buffer → `Task::ready(None)`. Relative path comes from `project::File::from_dyn(buffer.file()).path().as_ref()` guarded exactly as the TS precedent does.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p languages lpi`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/languages
git commit -m "offer lazbuild tasks per build mode from an open .lpi"
```

---

### Task 3: Wire the provider to the `lpi` language

**Files:**
- Modify: `crates/languages/src/lib.rs` (module decl near `mod pascal;` — actually anywhere in the mod block; and the `LanguageInfo { name: "lpi" }` entry)

**Interfaces:**
- Consumes: `lpi::LpiContextProvider` from Task 2 (`pub(super)` is enough).
- Produces: at runtime, opening an `.lpi` in the editor and opening the task palette shows lazbuild tasks for the project.

- [ ] **Step 1: Attach the provider**

In `crates/languages/src/lib.rs`: add `mod lpi;`, create `let lpi_context_provider = Arc::new(lpi::LpiContextProvider);` next to the other context providers, and in the `LanguageInfo { name: "lpi", adapters: vec![], ..Default::default() }` entry replace `..Default::default()` context absence with `context: Some(lpi_context_provider)`.

- [ ] **Step 2: Build and run the crates' tests to verify nothing broke**

Run: `cargo build -p zed && cargo test -p languages && cargo test -p lazarus_project`

Expected: build succeeds; all tests pass (existing `languages` tests unchanged in count for `pascal` module).

- [ ] **Step 3: Commit**

```bash
git add crates/languages/src/lib.rs
git commit -m "attach the .lpi task context to the lpi language"
```
