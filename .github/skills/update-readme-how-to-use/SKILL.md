---
name: update-readme-how-to-use
description: 'Keep the "How to use" section of README.md in sync with the app. Use whenever a change affects how the viewer is used: new or changed keys, mouse controls, buttons, file formats, export behavior, defaults, command-line or build/run steps, or removed features. Also use when adding to the "Changes from upstream" list.'
---

# Update the README user manual

The repository README has a **How to use** section (the user manual) and a **Changes from upstream** section. Both must stay accurate.

## When to apply

Apply this skill after any change that alters what a user sees or does, for example:

- A key binding or mouse control is added, changed, or removed (see `handle_key`, `window_event`, and `device_event` in `src/app.rs`, and `src/camera.rs`).
- A button or overlay is added to `index.html`.
- The F1 help screen (`HELP_TEXT` in `src/renderer/renderer.rs`) no longer matches the app.
- Supported file formats, export output, file locations, or default orientation change.
- Run or build steps change.

Skip it for pure refactors, shader internals, or performance work with no user-visible effect.

## Steps

1. Read the current `README.md`, especially the **How to use** section.
2. Identify the user-facing behavior that changed, and read the code to confirm exact keys and behavior rather than relying on memory.
3. Update **How to use**:
   - Keep the controls table as the single list of keys and mouse actions: one row per input with a short description.
   - Add new inputs, edit changed rows, and delete rows for removed inputs.
   - Describe export, loading, and any platform differences (native vs web) in the prose below the table.
4. If the change is a new addition or modification compared to the upstream project (https://github.com/abist-co-ltd/wgpu-gs-viewer), add or update a bullet in **Changes from upstream**.
5. Keep `index.html`'s help overlay consistent with the controls table when the web build is affected.
6. Update the in-app F1 help screen so it matches: edit the `HELP_TEXT` constant in `src/renderer/renderer.rs`. Keep it ASCII only (the built-in font has no other characters), keep lines short enough to fit the window, and keep it consistent with the controls table, the export keys, and the export description.
7. Do not create separate documentation files. README.md is the only manual.

## Style

- Short, factual sentences. No marketing language.
- Use the exact key names shown in the table (for example `Shift`, `F2`).
- Mention units where relevant (for example degrees).
