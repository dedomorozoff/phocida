# Toolbar save icons + Phocida Help menu

Two small changes, both approved in conversation.

## 1. Save / Save all icons

**Why:** the toolbar's Save and Save all buttons currently use file-like glyphs
(`FileTextFilled`, `FileMultiple`) and look like two more "documents".

**What:** add `assets/icons/save.svg` (floppy disk) and `assets/icons/save_all.svg`
(two stacked floppies, partial back copy — same pattern as `file_multiple.svg`),
following the set's conventions: 16×16 viewBox, `stroke-width="1.2"`, `#DCE0E5`
strokes. Add `IconName::Save` / `IconName::SaveAll` (strum snake_case matches the
file names), point the toolbar buttons at them. `New` stays `FileGeneric`.

**Verification:** the existing pair `test_all_icons_exist` / `test_no_dangling_icons`
enforces enum ↔ files consistency; `cargo test -p icons` and `cargo test -p toolbar`
must pass; visual check on a screenshot.

## 2. Help menu (variant A)

**Why:** the Help menu is full of Zed leftovers.

**Keep:** About Phocida, Show Welcome, Documentation (zed.dev/docs — the editor
docs still describe this fork), View Dependency Licenses (GPL obligation),
File Bug Report… → `OpenBrowser` to the Phocida repo issues
(`https://github.com/dedomorozoff/phocida/issues`).

**Drop:** View Release Notes Locally, View Telemetry, Request Feature, Email Us,
Zed Repository, Join the Team. Remove now-unused imports
(`auto_update_ui`, `feedback`, `onboarding` if unused elsewhere).

**Verification:** `cargo check -p zed`, launch, screenshot the Help menu.
