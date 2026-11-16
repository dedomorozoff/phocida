# Lazarus form designer milestone

**Intent:** Phocida is a hybrid Lazarus IDE: editor power + Lazarus features.
The single biggest missing Lazarus feature is the **visual form designer**
(`TForm`, drag-drop, property inspector, `.lfm` output). Today Phocida only has
`lfm` *grammar* highlighting; there is no designer UI.

**Why:** "Смесь Zed + фичи Лазаруса" — визуальное проектирование форм —
операционный центр классического Lazarus workflow. Без него Pascal-IDE не
прощает себя: ты пишешь .lpr/.lpi вручную.

**Current state:**
- `crates/languages/src/lib.rs`: `lfm` language = highlighting + grammar only
  (`adapters: vec![]`, no context beyond grammar).
- `pasls` does not parse LFM itself (no matches in source), but Lazarus
  CodeTools provide the component model.
- No designer crate, no canvas/palette/inspector in Phocida.

**Goals:**
- Visual canvas showing the form of an open `.lfm` (or auto-create for a
  `.lpr` main window).
- Component palette: LCL classes usable for Pascal projects (TButton, TLabel,
  TEdit, TListBox, ...).
- Drag-drop onto canvas → creates component in `.lfm`/`.lpr`.
- Property inspector editing component properties → updates `.lfm`.
- `.lfm` remains source of truth; designer edits round-trip through it.

**Non-goals / anti-goals:**
- Full visual designer parity with Lazarus IDE (anchors, constraints, designer
  layout). Anti-goal: pixel-perfect replication.
- Form inheritance hierarchy UI. Out of scope.
- Embedded Lazarus IDE binary (`designer.exe`) — rejected: not headless-friendly
  and fragile; build a native GPUI designer instead.

**Constraints:**
- Zed/GPUI rendering model: custom element with a viewport, entities for
  components, async layout. No off-the-shelf designer crate fits Pascal/LFM.
- Persistence: must keep LFM XML format in sync (hand-rolled round-trip risk).

**Approaches:**

1. **Native GPUI designer (recommended)** — new crate `crates/lazarus_designer`:
   canvas element + palette + drag/drop + inspector + LFM serialization.
   Pros: full control, consistent look, Phocida is a fork so this fits.
   Cons: biggest implementation, layout math, undo.

2. **Embed Lazarus IDE designer binary** — run `designer.exe` from
   `C:\lazarus\designer` as a child window per project. Pros: feature-complete
   in days. Cons: native Lazarus app inside GPUI window, IPC, fragile on
   Windows.

3. **Code-first structural editing** — designer-like UI that edits generated
   Pascal code / LFM programmatically (e.g., click-to-place via code blocks).
   Pros: small. Cons: not visual drag-drop; weak UX.

**Decision pending:** which approach.

**Verification:** designer edits a `.lfm`, round-trips, build still works.

---

Next: design doc approval → write plan (`superpowers:writing-plans`) → implement.
