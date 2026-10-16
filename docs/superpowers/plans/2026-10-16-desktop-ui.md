# Desktop UI Implementation Plan

> **For agentic workers:** use superpowers:executing-plans inline. User explicitly authorized implementing the approved design; no additional design approval is needed.

**Goal:** replace the migration preview UI with a coherent, fully functional native workbench.
**Architecture:** retain MetinApp, Editor, RuntimeHandle and Profile as owners; split presentation into shell, run, calibration, buffs, settings and shared widgets. One readiness model drives Start and its explanations. One canvas handles every crop.
**Tech Stack:** existing Rust/egui/eframe; dependency-free vector icons.
**Spec:** ../specs/2026-10-16-desktop-ui-design.md

## Global constraints
- Preserve every existing field, command, numeric range, migration and input guard.
- English product copy, both themes, minimum 800x560, no font-dependent icon glyphs.
- Continue on the existing clean codex/rust-egui-migration branch; no new checkout, push or production cutover.

## Review focus
- Migrated template pixels without a region must report ready consistently.
- Reordering/removing a skill must preserve selected detail and clear stale crop/binding references.
- Navigation, typing and global undo must not corrupt edits or create hidden mouse gestures.
- Unsupported backend / no window / dirty profile must explain the actual next action.
- Eight long-named skills, small windows and scaled fonts must retain navigation and scroll access.

## Tasks
- [x] T1: Add failing readiness/navigation/reorder tests in ui/tests.rs; implement shared widgets/style/vector icons, derived readiness and tab transitions in ui/{widgets,state,shell}.rs and mod.rs. Run desktop library tests.
- [x] T2: Replace fixed sidebar with task-specific Run/Calibration/Buffs/Settings layouts; preserve editor and runtime commands; remove obsolete forms and rendering paths. Tests cover crop readiness, skill selection/reorder/removal, typing/undo guards and all destinations. Run desktop tests.
- [x] T3: Add meaningful pointer-driven navigation/tooltips/forms tests; render native screenshots of all destinations at 800x560 and 1280x820 in light/dark. Inspect and correct clipping, labels, contrast, missing icons and hierarchy; record scenarios and limits in docs/rust/ui-verification.md.
- [x] T4: Run full workspace tests, fmt and strict Clippy on Windows/Linux; build both. Review final patch and evidence, correct findings, leave concrete review scenarios for independent normal-user UI review. No push/release.

## Final evidence

All four tasks completed. Windows and Linux each pass 78 tests (including 21 UI tests), formatting, strict Clippy and native desktop builds. Twenty Windows and four Linux screenshots were inspected. Three independent review findings were repaired and covered by regression tests; follow-up review found no remaining material issue. See docs/rust/ui-verification.md for limitations and review scenarios.
