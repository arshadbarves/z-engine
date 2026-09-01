# Changelog

All notable changes to the **Z Engine** project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.4.1] - 2026-09-01

### Fixed
- **Auto-Updater Signature Verification**: Updated `pubkey` in `tauri.conf.json` to match the Minisign signing key used in GitHub Actions release workflow.
- **Unified Version Architecture**: Centralized version management via `scripts/bump-version.sh` and dynamic version extraction in packaging and UI.

### Added
- **Integrated Changelog Viewer**: Added dynamic changelog fetching and markdown viewer inside Settings > About with offline embedded fallback.

---

## [1.4.0] - 2026-08-31

### Added
- **Master-Detail Workbench Changes**: Replaced accordion cards with a sleek Master-Detail review layout featuring a file explorer sidebar on the left and full-height syntax-highlighted diff viewer on the right.
- **Virtualized Diff Chunking**: High-performance line chunking and file pagination capable of rendering repositories with 10,000+ changed files smoothly.
- **Dual Diff Scopes**: Switch between **Current Session** net modifications and **Git Working Tree** uncommitted changes.
- **Fluid Docking Animations**: Added `0.24s cubic-bezier` slide-in and slide-out transitions on opening and closing side panels.
- **Docked Worktree Panel**: Migrated Git Worktree creation from a floating modal to a docked side container.
- **TopBar Redesign**: Search trigger converted to a compact icon button with centered draggable window titlebar badge.
- **Sidebar Marquee & Fade Blur**: Added right-edge gradient fade mask and hover marquee effect for long workspace titles.

### Fixed
- **Typography Descender Clipping**: Corrected line-height and vertical baseline alignment to prevent descender clipping (`g`, `y`, `p`, `q`, `j`) in file lists.
- **Floating Terminal Overlay**: Decoupled terminal popup from composer flex flow and fixed autocomplete dropdown keyboard scrolling.

---

## [1.3.0] - 2026-08-27

### Added
- **Custom Luxury Provider Picker**: Custom dropdown selector in settings for OpenRouter, Anthropic, OpenAI, and DeepSeek.
- **Live Memory & Headroom Stats**: Integrated real-time cache analytics and token headroom metrics.
- **Apple Fluid Physics**: Spring physics on dialogs and overlays.

---

## [1.2.0] - 2026-08-20

### Added
- **Multi-Workspace Management**: Support for switching and managing multiple git repositories from the sidebar.
- **Session Checkpoint & Rewind**: Tree-based session checkpointing and step-by-step turn revert.
- **Model Catalog Integration**: Live OpenRouter and provider model catalog fetching with context window stats.

---

## [1.1.0] - 2026-08-15

### Added
- **Tauri Desktop GUI Frontend**: Svelte 5 + Bits UI high-performance desktop client.
- **Dual Frontend Architecture**: Terminal UI (Ratatui) and Desktop GUI sharing unified `z-engine-core`.
- **Streaming Tool Approvals**: Interactive permissions engine with per-command rule persistence.
