# Hide the edit predictions button by default

**Intent:** Phocida is a Pascal/Lazarus IDE; the AI edit-prediction button
(status bar, `crates/edit_prediction_ui/src/edit_prediction_button.rs`) offers
providers (Zed/Copilot) that are not useful for Pascal code, where completions
come from pasls. Keep the feature for opt-in, hide the button by default.

**Change:** `edit_predictions.button` default flipped `true` → `false` in
`assets/settings/default.json` (plus the documented default in
`crates/settings_content/src/language.rs`). The edit-prediction code paths are
untouched; anyone can re-enable with one line in `settings.json`:

```json
"edit_predictions": { "button": true }
```

**Approaches considered:** remove the button entirely (rejected — throws away
working Zed machinery), leave as-is (rejected — default shows a button that
cannot do anything useful here), hide by default (chosen).

**Verification:** rebuild, confirm the button is gone from the status bar;
re-enabling via settings.json restores it.
