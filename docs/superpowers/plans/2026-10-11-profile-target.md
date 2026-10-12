# Profiles and individual metin targeting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Nahradit frontový režim jednotlivými kliky, kalibrací barvy a panelu, s oddělenými profily serverů.

**Architecture:** Jeden profil obsahuje všechnu kalibraci; jeden Worker a Runtime řídí nejvýše jednu akci. Barva hledá kandidáty uvnitř oblasti minus masky, šablona hledá panel cíle, řadič potvrzuje jeho životní cyklus. Windows hranice a kontroly zrušení se zachovají.

**Tech Stack:** Existující Python 3.14, Tkinter, OpenCV, numpy, Pillow, ctypes; žádné další závislosti.

**Spec:** `docs/superpowers/specs/2026-10-11-profile-target-design.md`, schváleno uživatelem „tak to udelej teda“.

## Global Constraints

- Jednotlivé běžné levé kliky bez Ctrl; žádný přepínač na starý frontový režim.
- Místní profily a PNG šablony; snímky ani konfigurace se neodesílají.
- Po kliku potvrdit panel před vyhodnocováním zmizení. Nepřítomnost neplatného snímku není zmizení.
- Aktivní panel blokuje nové kliky i rotaci. Bez potvrzení nebo po timeoutu omezené hledání/pauza.
- Změna geometrie, ztráta popředí, F8 a zavření přeruší vstupy; všechny držené vstupy se uvolní.
- 20 s příchod/potvrzení, 120 s aktivní cíl, 4 neúspěšné otočky; prahy nastavitelné a nenaměřené.
- Aktivace je asynchronní požadavek, před vstupy musí být skutečně ustálené popředí. UAC jen ručně.
- Bez WASD, léčby, loginu a multi-client běhu. Živý důkaz nelze nahrazovat testy.
- Složka není Git repo: úpravy přímo v tomto workspace, žádné commity/worktrees.

## Review Focus

- Šablona jiného profilu nebo chybějící PNG nesmí tiše spustit detekci: úloha 1.
- Podobná barva v UI nesmí dostat klik; masky platí i po morfologii: úloha 2.
- Kamera/překrytí neznamenají ukončení cíle a nesmí dovolit rychlé překlikávání: úloha 3.
- Zrušení mezi pohybem a klikem musí zabránit stisku: úloha 4.
- Přepnutí profilu za běhu musí nejdřív zastavit starý runtime: úloha 5.

## Files

- `config.py`: Region, ColorConfig, PanelConfig, TimingConfig, Profile; striktní typed validace.
- `profiles.py`: ProfileStore, JSON/PNG persistence, poslední profil, jednorázová migrace staré konfigurace.
- `vision.py`: Candidate, pick_color, detect_candidates, panel_score, CandidateTracker, annotate.
- `controller.py`: Observation, Action(kind/point), Status, Controller; čisté přechody.
- `windows.py`: jediný guarded InputDriver.click místo fill; discovery více procesů, asynchronní aktivace.
- `runtime.py`: Profile + template snapshot, snímání/detekce/řadič/akce.
- `app.py`: profily, kalibrační módy picker/oblast/maska/panel/šablona, živý náhled, Worker.
- `tests/*`: nahradit obsolete beam/queue testy; zachovat relevantní Windows a lifecycle testy.
- README, příklad profilu, check_capture, verification/progress: současný jediný tok a pravdivé důkazy.

## Task 1: Profiles and persistence

**Interfaces:** `Profile(id, name, process_name, region, masks, color, panel, timing, frame_size)`; `Profile.ready -> bool`; `ProfileStore.list() -> list[Profile]`, `load(id) -> tuple[Profile, ndarray | None]`, `save(profile, template=None) -> None`, `migrate_legacy(path) -> Profile | None`. IDs UUID, assets local file names only. ROI normalized; frame_size records calibration geometry. Template stored to unique asset first, JSON atomic second; obsolete asset removed after commit.

- [ ] Write failing tests: profile roundtrip, cross-profile template isolation, unsafe ID/asset rejection, NaN/unknown-field rejection, failed write preserves committed data, migrate once with backup and incomplete calibration. `assert not migrated.ready`, `assert first.color != second.color`.
- [ ] Run `python -m unittest discover -s tests -p test_profiles.py -v`; confirm RED.
- [ ] Implement types/store/migration; replace old schema and example, retain Region validation including client coordinate limits.
- [ ] Run config/profile tests, expect PASS; record evidence in progress.

## Task 2: Color candidates and target panel

**Interfaces:** `pick_color(frame, x, y) -> tuple[int,int,int]` median HSV; `detect_candidates(frame, profile) -> list[Candidate]`; Candidate rect + pixel click point from actual detected pixels; `panel_score(frame, template, panel_region) -> float`; `CandidateTracker.update(candidates, now) -> list[Candidate]`; `annotate(frame, candidates, profile, score) -> ndarray`.

- [ ] Write failing tests: hue wrap 179/0, selected colored blobs, min/max area, world ROI, masks after morphology, click point lies on selected pixel, unstable/old candidates rejected, template present/absent, uniform/small template rejected. `assert candidates == []` inside excluded UI; panel match near1 for identical nonuniform sample.
- [ ] Run vision tests; confirm RED.
- [ ] Implement one HSV mask pipeline, bounded geometry, one template matcher inside panel ROI, temporal candidate confirmation with cadence tolerance. No beam code retained.
- [ ] Run tests and diagnostic evidence from supplied images/video; sample candidates are evidence only, not certification for live Verzum.

## Task 3: Single-target controller

**Interfaces:** `Observation(ready, candidates: tuple[Candidate,...], panel_present: bool, reason)`; `Action(kind: CLICK|ROTATE, point: tuple[int,int] | None)`; `Controller(Profile).start(now)`, `pause(reason)`, `step(now, observation) -> Action | None`. Runtime supplies fresh validated observations; controller owns confirmation counters, deadlines and temporary failed candidate suppression.

- [ ] Write failing tests: initial existing panel adopted, click once, never-seen panel is not success, acquisition timeout, active panel blocks clicks/rotation, three absent frames allow next target, invalid observation pauses, attack timeout independent of animation, retries bounded to4, transient absence doesn't advance, failed candidate cooldown expires/reset on rotation.
- [ ] Run controller tests; confirm RED.
- [ ] Implement seeking/acquiring/active/settling/paused states with explicit deadlines; label completed targeting rather than destroyed stones.
- [ ] Run controller tests, expect PASS. Remove queue timing/marker-layout business logic.

## Task 4: Guarded click and runtime

**Interfaces:** `InputDriver.click(window, point, cancelled) -> None`; replace Ctrl-drag fill completely. `Runtime(profile, window, api, template)` owns a configuration/template snapshot; Sample exposes frame/candidates/panel score/state/reason. `WindowsAPI.list_windows(process_name=None)` may discover all eligible visible process windows, each GameWindow carries process basename; runtime still binds explicit HWND/PID.

- [ ] Write failing click tests: focus/geometry/held input refusal, cancellation before button-down, failure after button-down releases, failed delivery never clicks. Runtime tests: panel observation passed to controller, wrong calibration dimensions pause, incomplete profile refuses start, invalid capture preserves pause reason, close during click exits Worker and unregisters hotkeys.
- [ ] Run Windows/runtime/lifecycle tests; confirm RED.
- [ ] Implement click, discovery, runtime wiring; reuse existing scan codes, synchronous delivery acknowledgement, privilege preflight and cancellation guards. No parallel input backend.
- [ ] Run relevant tests, expect PASS. Remove FILL and all call sites/obsolete tests.

## Task 5: Calibration UI and final evidence

**Interfaces:** GUI obtains profiles through ProfileStore only; selection pauses/configures Worker with immutable profile/template. Calibration modes WORLD, MASK, COLOR, PANEL_REGION, TEMPLATE act on frozen frame with known size. Color click and drag coordinates clamp to displayed image. Start saves validated profile, requests activation once and waits through existing Worker gate.

- [ ] Write failing tests around non-Tk calibration logic: display->client mapping, mask exclusion, setting template freezes correct crop, change profile cancels old runtime, save refuses invalid selected window/config and no background start after error.
- [ ] Run tests; confirm RED.
- [ ] Implement profile create/select/name/process binding, controls for tolerance/geometry/timing/template threshold, preview overlays and separate calibration actions. Clear stale selected window/template on profile changes; incomplete profile remains stopped with actionable reason.
- [ ] Replace README/tool/example and mark old spec as superseded. Run full unittest, compileall, pip check; inspect rendered own UI and save/reload two isolated profiles.
- [ ] Use Computer Use for any live UI interaction. Validate actual click/panel/continuation/F8 on Verzum where permissions allow. If blocked, retain source and report exact limitation without claiming live farming.
- [ ] Remove obsolete code/config/deps/docs references for the active flow. Fresh whole-change review after passing tests; document remaining calibrated/live limitations and evidence in verification/progress.

## Execution handoff

Preserve previously chosen direct/native execution in this chat. Plan ready for review. No implementation of the new mode begins before this written plan is confirmed.
