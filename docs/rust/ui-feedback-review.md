> Historical verification report. Use the repository README and docs/release-readiness.md for current installation/release status. Earlier commands, counts and package paths below describe their original snapshots.

# Independent review of the eight UI feedback fixes

Reviewed 2026-10-16 against `docs/rust/ui-feedback-plan.md` and the working tree based on `7e820f3`. This review made no product changes and ran no Cargo commands. It inspected the final source, the native probe report, and all 16 freshly supplied screenshots at 1280 × 820 and 800 × 560 in dark and light themes.

## Verdict

No outstanding correctness or visual finding remains in the eight fixes after the follow-up corrections. The disabled reorder-arrow affordance was also corrected and verified against all eight newly rebuilt Buffs screenshots. Runtime and interaction acceptance is bounded by the evidence below; static screenshots are not live gameplay acceptance.

## Resolved finding: disabled reorder-arrow contrast

The initial P3 finding is resolved. Final source explicitly applies `ui.visuals().disabled_alpha()` when `response.enabled()` is false, including inherited disabled state. All eight refreshed periodic/inactive Buffs screenshots were reopened after this fix. The first-row Up arrow is visibly dimmer than enabled arrows in both themes and sizes; the last-row Down arrow is dimmed in the large views. The last row is below the viewport in small views, so its appearance there is supported by the shared source path rather than directly pictured. No new clipping or layout problem was introduced.

## Requirement coverage

| Requirement | Review result and evidence |
| --- | --- |
| 1. Reorder arrows inside skill rows, preserving all eight slots and selection | The screenshots show icon-only Up/Down controls beside each row, with eight configured slots. Source swaps the skills and associated units, preserves the selected skill, and retires stale skill gestures. Boundary arrows now visibly reflect their disabled state. |
| 2. Icon recognition only for Activate when inactive | Periodic screenshots omit the recognition card; inactive large screenshots show it. Source checks `BuffMode::Inactive` before rendering the card and does not delete stored crops when switching modes. The small inactive screenshots require scrolling to reach the card. |
| 3. HWND-only capture with geometry, identity, cancellation and freshness guards | Source has one Windows Graphics Capture path, with client crop bounds, RowPitch-aware staging readback, queue draining, source-QPC timestamp conversion, rejection of frames before the request or older than 250 ms, bounded presentation-time wait, resize recreation, and before/after window checks. There is no desktop-copy fallback. The rebuilt native v3 report records occlusion, delayed-queue freshness, resized client geometry and cancellation success. |
| 4. Wheel magnitude once per event | Source reads raw MouseWheel events and their units/magnitude once on the first egui pass; it no longer consumes smoothing every animation frame. A screenshot cannot prove the temporal behavior. The source regression test explicitly exercises one event followed by empty frames. |
| 5. Canonical HSV color and vector readiness marks | Calibration screenshots show the HSV values, matching red swatch and green vector checks. Source routes 8-bit sRGB HSV through the sRGB picker conversion instead of interpreting it as linear RGB. Red cross rendering is present in source but these complete-profile screenshots show only ready checks. |
| 6. Explicit armed edit tool, instructions, Done/cancel and frozen-only editing | Final source uses `editing` independently of the selected mode, gates editing on a frozen frame, clears tools on navigation/live mode and gates blue resize handles on `editing`. Supplied Calibration screenshots show an unarmed frozen canvas without resize handles and explain how to select a tool. Armed badge, Done and Escape transitions were inspected in source; they are not pictured in this screenshot set. |
| 7. Shared hit testing for gestures and cursors | Final source uses the same `gesture_at` classification for cursor selection and drag start, including resize handles, movement and creation. These static screenshots do not show cursor/drag transitions. |
| 8. Read-only Run view and controller-authoritative current/next selection | Run screenshots replace the duplicate connection card with a snapshot explicitly labelled not live, a color legend and an actionable Calibration link. Source uses the controller's selected point and `next_preview_target`, sharing candidate availability/failure exclusion and excluding the selected target during acquisition/combat. The earlier after-step Seeking-only problem is fixed. The supplied screenshots are stopped and cannot visually verify live amber/green overlays. |

## Screenshot walkthrough

1. **Run, large — healthy.** Activity and actionable readiness are on the left; the read-only snapshot is on the right. The snapshot is correctly labelled as not live. Session and buff schedule remain readable.

   ![Dark Run large](../../build/ui-redesign/dark-run-large.png)
   ![Light Run large](../../build/ui-redesign/light-run-large.png)

2. **Run, small — healthy with vertical scrolling.** Cards switch to a single column. The preview continues below the viewport; source confirms the page is a vertical ScrollArea. This is viewport clipping of scrollable content, not a clipped control or overlapping layout.

   ![Dark Run small](../../build/ui-redesign/dark-run-small.png)
   ![Light Run small](../../build/ui-redesign/light-run-small.png)

3. **Calibration, large — healthy.** The sampled swatch, HSV values, vector readiness checks and unarmed tool instruction are readable. No armed-looking blue resize handles remain after editing is inactive.

   ![Dark Calibration large](../../build/ui-redesign/dark-calibration-large.png)
   ![Light Calibration large](../../build/ui-redesign/light-calibration-large.png)

4. **Calibration, small — healthy with sidebar scrolling.** Preview controls wrap cleanly. Lower checklist items continue below the sidebar viewport, which has its own ScrollArea. The frame retains aspect ratio and usable canvas space.

   ![Dark Calibration small](../../build/ui-redesign/dark-calibration-small.png)
   ![Light Calibration small](../../build/ui-redesign/light-calibration-small.png)

5. **Periodic Buffs, large and small — healthy.** Reorder controls are compact vectors inside each row. The periodic details omit unused icon recognition. Lower rows/details continue in their respective scrolling panes, including the eighth slot.

   ![Dark periodic Buffs large](../../build/ui-redesign/dark-buffs-large.png)
   ![Light periodic Buffs large](../../build/ui-redesign/light-buffs-large.png)
   ![Dark periodic Buffs small](../../build/ui-redesign/dark-buffs-small.png)
   ![Light periodic Buffs small](../../build/ui-redesign/light-buffs-small.png)

6. **Inactive Buffs, large and small — healthy.** The large screenshots show the recognition card under inactive-mode details. Small screenshots preserve the row layout and rely on the detail pane's vertical scrolling to reach recognition controls. The inactive crop portion is below the visible area in this set, so its appearance is not independently verified here.

   ![Dark inactive Buffs large](../../build/ui-redesign/dark-buffs-inactive-large.png)
   ![Light inactive Buffs large](../../build/ui-redesign/light-buffs-inactive-large.png)
   ![Dark inactive Buffs small](../../build/ui-redesign/dark-buffs-inactive-small.png)
   ![Light inactive Buffs small](../../build/ui-redesign/light-buffs-inactive-small.png)

## Verification evidence and limits

- Inspected `build/rust-migration/window-capture-occluded-v3.json`: `success`, `occluded_window_pixels_match`, `source_time_fresh_after_queue_delay`, `resize_matches_client`, and `cancellation` are true; `game_input` is false. Reported initial client size is 584 × 381. This is acceptance for the controlled native fixture, not compatibility proof for every game renderer.
- The implementer separately reported the Windows workspace 98/98, UI 39/39, strict Clippy and build as passing. This reviewer did not rerun Cargo or independently execute those test commands.
- The screenshot set contains stopped fixture views. It does not prove live gameplay, keyboard navigation, screen-reader output, wheel timing, actual cursor changes, drag/cancel transitions, or all scrolled controls. Those claims require interaction or relevant test evidence.
- Both themes retain readable hierarchy at the two reviewed sizes. Readiness uses shape plus color, not color alone. Icon-only arrows have source-provided accessible labels/tooltips, and the disabled icon contrast was verified after correction. No full accessibility-compliance claim is made.
