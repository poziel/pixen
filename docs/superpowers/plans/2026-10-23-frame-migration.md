# Frame Migration Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task in this session.

**Goal:** Replace the Pixen egui desktop with a Frame/Vue/Tauri application while retaining the Rust engine, headless interfaces, and AI backend.

**Architecture:** Shared Rust desktop state and protocols are toolkit-independent. A thin Tauri host calls that state; Vue uses one Frame shell and renders the canvas and primary sidebar views.

**Tech Stack:** Rust, Tauri 2, Vue 3, TypeScript, Vite, Bun, local @frame/core.

**Spec:** ../../frame-migration-design.md

## Global Constraints

- Preserve checkpoint aff2986 and existing CLI/MCP contracts.
- No AI conversation frontend; preserve backend without starting AI automatically.
- One plain workspace; Layers, Timeline and AI placeholder in primary sidebar.
- No secondary sidebar, navigation bar, additional panels or document tabs.
- Do not modify Frame source or global package registrations.

## Review Focus

- Managed preview updates and EOF must work without an egui event loop.
- Reopening/saving cannot silently diverge from the active MCP document.
- Preview layer visibility, speed and inspection must not mutate saved artwork.
- Late asynchronous render responses must not replace newer selections.
- Closing a view must not stop playback; focused form controls must retain their keys.

### Task 1: Extract toolkit-independent backend and protocols

Files: src/agent.rs, src/viewer.rs, src/viewer_review.rs, src/lib.rs, src/desktop.rs, src/viewer_protocol.rs, Cargo.toml; backend tests.

Interfaces: `desktop::Application` owns document state; serializable snapshot/render requests expose IDs and revisions. `viewer_protocol::ViewerProcess` retains existing managed process API. AI uses `Wake` callback instead of egui Context.

- [x] Record baseline Rust suite results.
- [x] Add service tests for create/resize/save/reopen, preview-only visibility and managed message dispatch; observe missing implementation failures.
- [x] Extract protocols/DTOs and implement the application service using existing Rust Session/Desktop operations.
- [x] Replace agent toolkit wakeups; retain its existing behavioral tests.
- [x] Run backend tests and fmt; inspect the dependency graph for egui coupling.

### Task 2: Integrate Tauri and Frame/Vue host

Files: package.json, bun.lock, vite.config.ts, tsconfig.json, index.html, ui/*, src-tauri/*, Cargo.toml, src/bin/pixen.rs, .gitignore.

Interfaces: Tauri `snapshot`, `open`, `create`, `resize`, `save`, `render`, `review` commands delegate to Application. Managed stdio messages feed the same state and emit notifications to Vue.

- [x] Configure Bun/Vue/Vite with public Frame dependency, Tauri native host and required window permissions.
- [x] Implement one stable Frame shell with primary view registrations and restricted workspace/layout.
- [x] Implement canvas, layers, timeline, menus, dialogs, settings, status, inspection and review using shared Rust rendering.
- [x] Add frontend behavioral tests for playback, stale requests and view-only settings; run typecheck/build.
- [x] Switch GUI entry point and remove production egui dependencies after backend extraction.

### Task 3: Validate parity and document operation

Files: tests/*, scripts/*, docs/frame-migration-parity.md, README.md.

- [x] Run all Rust tests, formatting, frontend tests/typecheck/build, native compilation.
- [x] Verify managed protocol with the new executable: open, update, activate, activity, inspection/review, EOF.
- [x] Inspect running frontend/native app where available; record actual evidence and limitations.
- [x] Update build/run documentation and parity matrix; perform a fresh whole-change review and fix material findings.

## Execution ledger

- Synced main with origin/main (ac06803); created codex/merge-with-frame; fast-forwarded saved checkpoint aff2986 to retain current features.
- User explicitly approved implementation and instructed no further approval gates. Execute inline; use independent final review.

- Backend and frontend extraction implemented; superseded egui UI removed. Source engine and native argument tests pass.
- Review fixes: retain pending MCP targets/references, resolve alternate views, apply complete review isolation, invalidate annotated render cache by event serial. Regression tests observed failing before fixes; 5 frontend tests pass.
- Verification details and known limits: docs/frame-migration-parity.md. Baseline recovery checkpoint remains on origin at aff2986.
