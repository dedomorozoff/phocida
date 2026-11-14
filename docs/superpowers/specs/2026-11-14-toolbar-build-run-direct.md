# Toolbar Build/Run build the .lpi project directly

**Why:** Run (and Build) on the toolbar does nothing but open the task modal.
Root cause: both buttons search the task list for a template tagged `lazbuild`,
but `task_inventory::list_tasks` only exposes templates from the *active buffer's
language*. The `lazbuild`-tagged templates live on the `.lpi` language provider,
so with a `.lpr` (Pascal) buffer active nothing matches and the toolbar falls
back to `Spawn::ViaModal`.

**Decision:** stop going through the task list. The toolbar finds the project's
`.lpi` and constructs the lazbuild task itself:

- Discovery: first an open `.lpi` item; otherwise the sibling `.lpi` of the
  active `.lpr`/`.lpp`/`.dpr` buffer (Lazarus convention: `foo.lpi` next to
  `foo.lpr`).
- Template: `command: lazbuild`, args `[<lpi path>]` — default build mode (first
  one) is the default, so no `--build-mode` flag; label
  `build project '<title>' with lazbuild` (title from the `.lpi`, falling back to
  the main-unit stem, then "project"); tag `lazbuild` for consistency.
- Scheduling: `TaskSourceKind::UserInput` resolved via `tasks_ui::task_contexts`
  + `TaskContexts::resolve_task`, run through
  `schedule_resolved_task_with_completion`; on success the existing
  `after_success` runs (Run = launch the exe). No `.lpi` found → the existing
  `Spawn::ViaModal` fallback (user picks any task manually).

**Non-goals:** no changes to the task inventory or the `.lpi` provider; the
single-file `fpc` task stays available from the tasks modal / command palette.

**Verification:** new unit tests in `predicate.rs` (template construction,
sibling discovery, open-item preference); `cargo test -p toolbar`; manual Run on
the `laz` sample project builds and launches `project1.exe`.
