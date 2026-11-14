# Zed + Lazarus hybrid: restore the title bar

**Intent:** keep all of Zed's editor power (its chrome, menus, commands) and add
Lazarus features on top of it, not instead of it.

**Decision (hybrid, approach 1):** re-enable `title_bar::init(cx)` in
`crates/zed/src/main.rs`. The fork stopped calling it (only
`visual_test_runner` did), which is why the window has no decorations and no
menu bar. The crate and its dependency entry already exist and compile.

**What it gives:**
- Client-side window decorations on Windows: close / minimize / maximize buttons.
- The Zed application menu bar (File / Edit / Selection / View / Go / ...).
- The Lazarus-style toolbar (Build / Run) renders directly below it — the
  workspace region order was already fixed to titlebar → toolbar in
  `73b2ade906`, including focus/region navigation order.

**Non-goals:** a custom Lazarus-styled menu bar; custom window buttons;
removing Zed-specific title bar items (project/branch/user menu can be trimmed
later via `TitleBarSettings` if they get in the way).

**Verification:** `cargo build -p zed`, launch, screenshot: decorations present,
menu bar present, toolbar sits under the menu; `cargo check` stays warning-free.

**Follow-up (separate, not in this change):** add `save.svg` / `save_all.svg`
to `assets/icons` so Save / Save all stop looking like two more "document"
glyphs.
