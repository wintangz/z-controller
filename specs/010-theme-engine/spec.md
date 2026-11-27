# Feature Specification: `theme-engine`

**Feature Branch**: `main` (continuing from `b368ce4`)
**Created**: 2026-11-28
**Status**: Active

## Requirements
- **Monochrome Theme Engine (`WT_UI_LIBRARY.md §0` v2)**:
  - Single hue family per screen rendered as a 4-step shade ladder (`deep` / `accent_dark` / `source` / `soft_fill`) over strictly neutral white/black/grey backgrounds (`R == G == B`, never theme-tinted).
  - Default theme = WT Blue (`#7CCBF2`, Light `#0090D9`, Dark `#69CBFC`).
  - Curated presets (`Blue`, `Pink`, `Purple`, `Green`, `Orange`, `Yellow`, `Red`) plus custom `#RRGGBB` / RGB input normalised for WCAG contrast (`>= 4.5:1` Light ink on `#FFFFFF`, `>= 5.5:1` Dark accent on `#191919`, `>= 4.5:1` ink on solid fills).
- **Live & Persistent App-Level Theme Picker**:
  - Header swatch button opens a popover with all 7 preset chips and custom Hex/RGB sliders/input applying live across Light and Dark modes across Main and Profile Editor windows, persisting alongside controller settings in `config.toml`.
- **No Cross-Hue Mixing / Button Gradients / Colour-Only Status**:
  - Highlight rims and meter bars (including healthy battery >20%) use within-family monochrome sweeps or solid accent only. Cross-hue gradient remains exclusively on the WT logo mark / app icon (exe icon untouched).
  - Status indicators pair semantic `STATUS_GREEN` / `STATUS_RED` with `✓` / `✕` / `!` vector icons + explicit text labels (`Working`, `OK`, `Cloaked`, `Visible`, `Low`).
- **DESIGN.md Shared-Core Sync**:
  - Update `DESIGN.md` Part I to describe the monochrome theme engine, neutral surface ladder, and icon+label status rule.
