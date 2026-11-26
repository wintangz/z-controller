# WT Native Instrument Design System (`DESIGN.md`)

> **Canonical Ecosystem Standard**: This document is the canonical design standard for **all WT applications** (`z-controller`, `y-storage`, `WT Manager`, and future WT native utilities).
> **Reference Implementation**: **`z-controller`** (`Y:\Repos\z-controller`) is the reference implementation for all shared controls, theme tokens, rim shaders, and outline button geometry.
> **Adoption Rule**: Any WT application adopting this standard MUST copy `DESIGN.md` into its repository root and implement **Part I — Shared Core** unchanged. Application-specific layouts, domain widgets, and intentional deviations belong strictly inside that application's own section under **Part II — App-Specific Sections**.

**Last Updated**: 2026-11-26 (`ui-fix-3`)

---

# Part I — Shared Core (WT Family Standard)

All rules, tokens, geometries, and component behaviours in Part I apply identically across every WT application.

## 1. North Star & Anti-Patterns

WT applications are precision instruments, not dashboards: every pixel, colour, border, and type tier must encode real system state or measured quantity, and nothing exists as decoration for its own sake. We design explicitly against the **generic AI UI failure mode**:

1. Repeating identical bordered cards around every list or ledger row (`cards-inside-cards` when expanded).
2. Applying a single uniform corner radius (`6px` on panels, badges, tracks, and buttons alike).
3. Using raw pastel accents (`#54B9F2`, `#D8A6FF`, `#F7A8D8`) or saturated status tones (`#28D66C`, `#F04458`) as large opaque surface fills.
4. Centring empty-state labels or orphaned section headers in dead vertical space.
5. Rendering secondary metadata (publishers, provenance/basis strings, kind labels, supporting counts) at equal size and brightness as primary quantities.
6. Placing badge chips on static labels (`READ-ONLY`, mode banners, descriptors) rather than live states or user decisions.

---

## 2. Colour Tokens (Dark & Light Modes)

All WT applications share one dual-mode palette calibrated against `z-controller` (`Y:\Repos\z-controller\apps\z-controller\src\ui.rs`).

### 2.1 Surface, Border & Typography Luminance Tokens

| Token | Dark Mode | Light Mode | Role |
|---|---|---|---|
| `bg` (`S0` Canvas) | `#0A0A10` | `#EEF4FA` | Window backdrop and top header canvas |
| `panel_alt` (`S1` Recessed) | `#10101A` | `#F4F8FC` | Sticky table headers, expanded active drawers, pinned footer (`z-controller` `faint_bg`) |
| `panel` (`S2` Primary) | `#14141F` | `#FFFFFF` | Ledger wrappers, summary/instrument pods, Settings cards |
| `widget` (`S3` Rest) | `#181826` | `#F5F8FC` | Filter pill backgrounds, inactive interactive surfaces |
| `widget_hover` (`S3` Hover) | `#1F1F31` | `#E9F2FA` | Hovered buttons, theme toggle, interactive toggles, hovered rows |
| `widget_active` (`S3` Press) | `#182230` | `#DCEBF8` | Pointer-down active buttons and pressed controls |
| `input_bg` (`Well`) | `#0A0A10` | `#EEF4FA` on `panel` (`#FFFFFF` directly on `bg` canvas) | Text input wells (maintains distinct recess vs `#FFFFFF` cards and `#EEF4FA` canvas) |
| `border` (`Hairline`) | `#26263A` | `#D9E4F0` | `1px` crisp structural borders, SubtleOutline strokes, hairline ledger dividers |
| `scrollbar_thumb` | `#52567A` | `#98B0CA` | Slim vertical scrollbar thumb at rest (minimum visible contrast against track) |
| `scrollbar_thumb_hover` | `#7A86B8` | `#648CB8` | Slim vertical scrollbar thumb on hover (brightens to `accent_line` when dragged) |
| `text` | `#EDEDF2` | `#1C2733` | Primary headlines, row identities, dominant measurements, button hover/press text |
| `muted` | `#9A9AB0` | `#5F7186` | Secondary metadata, quiet totals, SubtleOutline labels, disabled text |
| `subtle_text` | `#82829A` | `#748499` | Low-contrast inline descriptors, publishers, truncated basis copy |
| `hint_text` | `#64647A` | `#98A6B8` | Unfocused, empty input placeholder hints only (hidden on focus or non-empty value) |

### 2.2 Accent & Semantic Status Tokens

| Token | Dark Mode | Light Mode | Role |
|---|---|---|---|
| `LOGO_BLUE` / `accent_line` | `#54B9F2` | `#2F9BE8` (`#54B9F2` rim stop) | Active outlines, Primary/Secondary button accents, primary meters, chevrons, gradient stop `0.0` |
| `LOGO_LAVENDER` (`lavender_text`) | `#D8A6FF` | `#D8A6FF` (`#783EB8` small text) | Secondary meters/ticks, qualifier badge stroke, gradient stop `0.5` |
| `LOGO_PINK` | `#F7A8D8` | `#F7A8D8` | Pressed/latched control outline, subtle unattributed ticks, gradient stop `1.0` |
| `pill_blue_fill` | `#121C28` | `#EBF6FF` | Subtle fill for `PrimaryOutline` buttons, active filter pills, checkboxes, provenance badges |
| `pill_lav_fill` | `#20182E` | `#F3ECFF` | Soft backdrop for lavender qualifier badges |
| `STATUS_GREEN` (`status_green`) | `#28D66C` | `#0FA84F` (`#28D66C` dots/marks) | Saturated positive/healthy semantic status (`§8`) |
| `pill_green_fill` | `#102E20` | `#E4F9EC` | Soft backdrop for saturated green semantic status pills |
| `STATUS_RED` (`status_red`) | `#F04458` | `#E02444` (`#F04458` dots/bars) | Saturated alert/growth/locked semantic status (`§8`) |
| `pill_red_fill` | `#2C121E` | `#FDE8ED` | Soft backdrop for saturated red alerts and `Active-Highlight` latched toggles |

---

## 3. Typography Scale & Luminance Hierarchy

WT applications pair **`Segoe UI`** for proportional interface text with **`Consolas`** monospace tabular numerals for measured quantities, coordinates, and timestamps. Minimum readable type floor across all UI copy: **`10.5pt`**. No rendered text may be smaller than `10.5pt`.

- **Numeral Weight Rule (`perf-pass-1`)**: **All numerals (`Total size`, `Own size`, drive `free`/`used` figures, pinned ledger sizes, deltas, and counts) are Medium — or Regular (`Consolas Regular`) where Medium is unavailable in the bundled font — NEVER Strong/Bold.** Heavy bold numeral slabs add visual noise without aiding legibility.
- **Strong Weight Rule**: **`Strong` weight (`Segoe UI Semibold`) is reserved strictly for the app name in a row and an expanded drawer's title line.** Headlines, metadata, badges, and numerals stay Regular or Medium.

| Tier | Size & Weight | Luminance Token | Role |
|---|---|---|---|
| **T1 — Instrument Headline** | `16.0pt` `Segoe UI` Regular (`14.5pt` `Consolas Regular` for primary pod metrics) | `text` | View titles and top-level live readouts (`drive free space`, primary instrument header values) |
| **T2 — Dominant Row Quantity** | `13.0pt`–`13.5pt` `Consolas Regular` (Medium where available; never Strong) | `text` | Single right-aligned primary quantity on every list row or card (dominant by size & brightness, never bold weight) |
| **T3 — Primary Row Identity & Drawer Title** | `13.0pt`–`14.0pt` `Segoe UI Semibold` (`Strong`) on row app names & expanded drawer titles; `12.0pt`–`12.5pt` Regular for pinned ledger titles, button labels, input values | `text` (controls follow button state table `§6`) | Row app names, expanded drawer titles, pinned section titles, button captions, active filter/input copy |
| **T4 — Secondary Metadata & Quiet Numbers** | `10.5pt`–`11.5pt` Regular (`11.5pt` `Consolas Regular` paths/supporting sizes; `11.0pt` column headers & publishers; `10.5pt` section eyebrows, badges, word-capped `"…"` provenance) | `muted` / `subtle_text` | Sticky column headers, secondary row attributes, sub-table quantities, full-text hover tooltips when capped |

---

## 4. Spacing Scale & Corner Radii

### 4.1 Spacing Scale (Multiples of 4 Only)
`SPACE_4 = 4.0`, `SPACE_8 = 8.0`, `SPACE_12 = 12.0`, `SPACE_16 = 16.0`. All outer margins, panel padding, header control gaps, table cell gaps, and vertical section rhythms derive strictly from `{4, 8, 12, 16}`. Rows hug their content cleanly without trailing dead whitespace; empty/sparse states align top-left rather than floating centred in dead vertical space.

### 4.2 Corner-Radius Scale (Differentiated by Role)
Different architectural tiers carry distinct corner radii:
- **`0.5 * badge_height` (~`9.0px` `RADIUS_BADGE_PILL`)**: Non-interactive status/zone/confidence/qualifier **pill badges** (`NO-TOUCH`, `LOCKED`, `proven`, `inferred`, `shared`, `missing`, `● GREEN`, `● RED`, `BODY`, `Roaming config`, `ProgramData · shared`, kind chips).
- **`8.0px` (`RADIUS_PANEL` / `CARD_ROUNDING`)**: Elevated containers — summary pods, ledger wrappers, active/expanded containers & gradient rims, Settings panels, overlays.
- **`6.0px` (`RADIUS_CONTROL`)**: Shared interactive controls synced with `z-controller` — `ThemeToggle`, outline action buttons (`Rescan`, `Settings`, `Clear`, `Add exclusion`, `Remove`, overlay `Close`/`All`), segmented sort controls, text input wells.
- **`4.0px` (`RADIUS_PILL`)**: Compact interactive single-select filter buttons, inline `used by N apps >` triggers, and `16x16` toggle checkboxes.
- **`3.5px`–`4.0px` (Track Outer) / `2.5px`–`3.0px` (Track Fill)**: Chunky `7px`–`8px` rounded capacity and battery meters.
- **`0.0px`**: Hairline-divided ledger rows, inline `<= 2px` proportion rules, and horizontal dividers.

---

## 5. Gradient-Rim State Rule (WT Signature)

The signature continuous gradient rim is a **state signal**, never decorative chrome:

- **Where permitted**: Strictly **active or expanded containers** — an active virtual/controller card (`z-controller`) or an expanded inspection drawer (`y-storage`). Collapsed rows, idle cards, meters, and static containers MUST NOT carry gradient rims (`1px border` hairlines only).
- **Exact geometry (`z-controller` `paint_active_card_gradient_border`)**:
  - **Outer rounding**: `8.0px` (`CARD_ROUNDING`), **`SEGS = 16`** corner arc segments, **`BANDS = 5`**.
  - **Edge profile**: `offsets = [-4.5_f32, -2.0_f32, -1.6_f32, -0.4_f32, 0.0_f32]`, `alphas = [0_u8, 52_u8, 255_u8, 255_u8, 0_u8]` (`1.2pt` solid perimeter core + `2.5pt` soft inner glow), paired with an inner content fill polygon `rect.shrink(1.0)` at **`7.0px`** rounding (`panel_alt` when expanded over a ledger).
  - **Diagonal gradient stops**: parameter `t = clamp(((x - rect.left()) / w) * 0.65 + ((y - rect.top()) / h) * 0.35, 0.0, 1.0)` interpolating **`LOGO_BLUE` (`#54B9F2`)** at `0.0` -> **`LOGO_LAVENDER` (`#D8A6FF`)** at `0.5` -> **`LOGO_PINK` (`#F7A8D8`)** at `1.0`.

---

## 6. Family Button & Interactive Control Standard

All shared interactive controls mirror **`z-controller`** (`Y:\Repos\z-controller\apps\z-controller\src\ui.rs`) so headers, forms, dialogs, and scrollbars feel built from one binary toolkit.

### 6.1 Crisp Outline Geometry & Padding
- **Perimeter ring rendering**: Controls use pixel-snapped `RADIUS_CONTROL = 6.0px` geometry via `paint_crisp_bordered_box` / `paint_crisp_ring_mesh` (`1.0px` stroke inset normalized by `pixels_per_point`). When `fill == Color32::TRANSPARENT`, ONLY the unbroken `1px` perimeter mesh is drawn — avoiding solid-fill quad artifacts across 96 DPI and HiDPI scales.
- **Horizontal padding**: `SPACE_12` (`12px`) for standard controls (`min_size.y > 22.0`); `SPACE_8` (`8px`) for compact table/inline action buttons (`min_size.y <= 22.0`, e.g., row cross-reference triggers).

### 6.2 Outline Button Variants & Interactive States

| Variant / State | Background Fill | `1px` Outline Stroke | Text / Glyph Foreground | Typical Use |
|---|---|---|---|---|
| **`PrimaryOutline`** (rest) | `pill_blue_fill` (`#121C28` / `#EBF6FF`) | `accent_line` (`#54B9F2` / `#2F9BE8`) | `accent_line` (`#54B9F2` / `#2F9BE8`) | Primary header/panel CTA (`Rescan`, `Continue`, `Apply`) |
| **`SecondaryOutline`** (rest) | `TRANSPARENT` | `lerp(border, accent_line, 0.72)` | `accent_line` (`#54B9F2` / `#2F9BE8`) | Navigation, view toggles, secondary actions (`Settings`, `Back to Apps`, `ThemeToggle`, `Add exclusion`, `used by N apps >`) |
| **`SubtleOutline`** (rest) | `TRANSPARENT` | `border` (`#26263A` / `#D9E4F0`) | `muted` (`#9A9AB0` / `#5F7186`) | Dismissive, clear, or low-priority utility actions (`Clear`, `Remove`, overlay `Close` / `All`) |
| **Hovered** (any enabled variant) | `widget_hover` (`#1F1F31` / `#E9F2FA`) | `accent_line` (`#54B9F2` / `#2F9BE8`) | `text` (`#EDEDF2` / `#1C2733`) | Pointer hover across all enabled buttons |
| **Pressed** (`is_pointer_button_down_on`) | `widget_active` (`#182230` / `#DCEBF8`) | `LOGO_PINK` (`#F7A8D8`) | `text` (`#EDEDF2` / `#1C2733`) | Active pointer press feedback (universal WT press border) |
| **Active-Highlight** (latched toggle) | `pill_red_fill` (`#2C121E` / `#FDE8ED`) | `LOGO_PINK` (`#F7A8D8`) | `text` (`#EDEDF2` / `#1C2733`) | Latched warning/record/toggle state |
| **Disabled** | `TRANSPARENT` | `border` (`#26263A` / `#D9E4F0`) | `muted` (`#9A9AB0` / `#5F7186`) | Unavailable action (inert `Sense::hover`) |

### 6.3 Shared Header & Form Controls
- **`ThemeToggle` Icon Button**: Square `28x28 px` (`HEADER_CONTROL_H`) `6.0px` `SecondaryOutline` container sharing exact `Hovered`/`Pressed` rules above.
  - Dark mode draws an 8-ray sun glyph in `accent_line` (`text` on hover/press).
  - Light mode draws a crescent moon in `accent_line` (`r = 5.3`) punched cleanly by an offset cutout circle (`r = 4.4`) painted in `bg` when rest/transparent, or the active hover/press `fill` during interaction.
- **Text Inputs**: Recessed `RADIUS_CONTROL = 6.0px` frame filled with `input_bg` (or `header_input_bg` `#FFFFFF` when sitting directly on `#EEF4FA` Light header canvas), `1px` `border` (`accent_line` while focused), `12.0pt` `text` value, `12.0pt` `hint_text` visible **only** when unfocused and empty.
- **Checkboxes**: `16x16 px` `RADIUS_PILL = 4.0px` box (`pill_blue_fill` + `accent_line` + `1.75pt` `accent_line` check stroke when checked; `widget_hover` + `accent_line` hovered; `input_bg` + `border` unchecked).
- **Scrollbars**: Non-floating `6.0px` vertical thumb (`scrollbar_thumb` -> `scrollbar_thumb_hover` -> `accent_line`), `handle_min_length = 24.0`, `SPACE_8` (`8px`) inner gutter (`bar_outer_margin = 0.0`).

### 6.4 WT Header Lockup & Application Icon Standard
- **Original Brand Mark Only**: Every WT application embeds the original unmodified WT mark (`assets/logo-mark.png`, `include_bytes!`, transparent background — no added background tile, container border, or app-specific glyph fusion) for both the multi-size `.exe`/window icon (`16px`–`256px` across title bar, taskbar, Alt-Tab, and Explorer) and the in-app header lockup.
- **Header Lockup Geometry**: Left-aligned on the top header row (`HEADER_CONTROL_H = 28.0px` vertical centreline):
  1. **Logo Mark**: **`24 x 24 px`** logical (`24 * pixels_per_point` physical, premultiplied-alpha area-downsampled from source artwork), transparent backdrop, optically centred vertically inside the `28.0px` row (`2.0px` top/bottom margin).
  2. **Logo-to-Name Gap**: **`SPACE_8` (`8.0px`)**.
  3. **App Name**: **`T1` `16.0pt` `Segoe UI` Regular** in primary `text` (`#EDEDF2` Dark / `#1C2733` Light), optically centred on the row centreline.
  4. **Name-to-Caption Gap**: **`SPACE_8` (`8.0px`)**.
  5. **Muted Inline Caption**: **`T4` `11.5pt` `Segoe UI` Regular** in `muted` (`#9A9AB0` Dark / `#5F7186` Light; semantic status colour permitted only during active transient operations), optically centred cleanly and single-line truncated before the right-hand action control cluster.
- **Settings / About Surface**: Any Settings or About surface repeats the same `24px` WT logo mark + `16.0pt` app name + `11.5pt` muted descriptor lockup.

---

## 7. Chip Discipline

1. **Pill Badges Exist Only for Decisions or State**: Non-interactive semantic badges render as full **pill shapes** (`corner_radius = 0.5 * height`, e.g., `9.0px` on `18px` pills; `10.5pt` Regular) strictly when encoding an actionable constraint, safety boundary, verification confidence, qualifier, or status mark (`§8`). Never render large heavy dark solid badge fills in Light mode — both Dark and Light modes use outline (`1px`) + soft tinted backdrop + high-contrast ink (`#E3F2FD` / `#2F9BE8` / `#1565B8` for Light blue `proven` pills; `#121C28` / `#54B9F2` / `#7CCBFF` for Dark).
2. **Forbidden Static Chips**: Never wrap persistent modes (`READ-ONLY`), hardware/OS classifications (`SYSTEM DRIVE`), prose category names (`Category` column text, `shared runtime` in tables), internal spec terms (`Constitutional`), or quiet steady-state labels in bordered badge chips.
3. **Interactive Filter Buttons**: Single-select filter controls use `RADIUS_PILL = 4.0px` (`pill_blue_fill` + `1px accent_line` + `text` when active; `widget` + `1px border` + `muted` when inactive) so interactive controls remain visually distinct from non-interactive pill badges.

---

## 8. Status-Colour Saturation Rule

Across every WT instrument, **saturated green/red (`STATUS_GREEN` / `STATUS_RED`) is reserved exclusively for real semantic status**, while structural accents and metadata stay at calm pastel or neutral contrast levels.

| Category | Element Examples | Palette Treatment |
|---|---|---|
| **Takes Saturated Status** | • `8px` (`r = 4.0`) health, device-link, and reconciliation status dots (`diff 0 B`) + paired status text<br>• Critical/threshold-driven capacity & battery meters (`z-controller` battery `> 20%` `STATUS_GREEN` / `<= 20%` `STATUS_RED`; `y-storage` drive bar `>= 85%` full / `<= 15%` free `STATUS_RED`)<br>• Signed change deltas (`-` shrank/favorable green, `+` grew/unfavorable red)<br>• Safety locks, alerts, and missing-resource badges (`NO-TOUCH`, `LOCKED`, `missing`, `diff != 0`) using `status_red` outline/text over `pill_red_fill`<br>• Explicit safety-zone badges (`● GREEN`, `● RED`) featuring a `5px` saturated dot over `pill_green_fill` / `pill_red_fill` | **`STATUS_GREEN`** (`#28D66C` Dark / `#0FA84F` Light text & stroke; `#28D66C` solid dot)<br>**`STATUS_RED`** (`#F04458` Dark / `#E02444` Light text & stroke; `#F04458` solid dot & bar) |
| **Stays Pastel or Quiet Neutral** | • Healthy non-battery allocation meters (`LOGO_BLUE` / `LOGO_LAVENDER`)<br>• Active container gradient rims (`LOGO_BLUE -> LOGO_LAVENDER -> LOGO_PINK`) and pressed `LOGO_PINK` borders<br>• Provenance & qualifier badges (`proven` in `pill_blue_fill` + `accent_line` + `text`; qualifiers in `pill_lav_fill` + `LOGO_LAVENDER` + `lavender_text`)<br>• Neutral structural badges (`BODY`, `shared`, `inferred` in `TRANSPARENT`/`border`/`muted`)<br>• Inline `<= 2px` proportional table bars, hairline dividers, and `T4` metadata | **Pastel (`LOGO_BLUE`, `LOGO_LAVENDER`, `LOGO_PINK`)** or **Quiet Neutral (`border`, `muted`, `subtle_text`)** |

---

## 9. Shared Layout & Family Critique Checklist (`FC1`–`FC9`)

Tabular instruments align numbers in fixed right-aligned monospace columns under a sticky `panel_alt` sortable column-header row (`T4` `11.0pt` labels acting directly as sort triggers with a single directional arrow, never duplicated in a separate bar).

Before accepting screenshots of any WT application, verify **`FC1`–`FC9`**:
- **FC1 (No Card Soup)**: List rows and summary lines sit inside hairline-divided ledger panels rather than repeating stacked individual cards.
- **FC2 (Restrained Numeral Weight & Clear Row Hierarchy)**: Right-aligned `T2` totals lead cleanly by scale and luminance (`13.0pt`–`13.5pt` Regular/Medium, never heavy Bold slabs) alongside `T3` `Segoe UI Semibold` app names and quiet `T4` metadata.
- **FC3 (State-Signal Gradient Rim Only)**: Diagonal gradient rims (`#54B9F2 -> #D8A6FF -> #F7A8D8`) appear strictly on active/expanded containers; zero collapsed containers or non-status pills use solid pastel fills.
- **FC4 (Multi-Tier Corner Radii)**: Visible separation across `8px` panels/rims, `6px` buttons/inputs, `4px` filter pills/checkboxes, `3.5px–4px` chunky meter tracks, full-pill (`0.5 * h`) status badges, and `0px` hairline ledger rows.
- **FC5 (Saturated Status Discipline)**: Saturated `#28D66C` / `#F04458` (`#0FA84F` / `#E02444` Light) marks appear strictly on live status dots, threshold/battery alerts, signed deltas, safety locks, and zone marks — while metadata, proportions, and qualifiers stay calm pastel/neutral.
- **FC6 (Outline Button Hierarchy & Crisp `1px` Rings)**: `3x` cropped header/panel buttons show unbroken `1px` rings across Dark and Light modes, clear `PrimaryOutline` / `SecondaryOutline` / `SubtleOutline` hierarchy, and sharp moon/sun icon contrast.
- **FC7 (`>= 10.5pt` Floor)**: All visible labels, badges, captions, and tooltips meet or exceed `10.5pt`.
- **FC8 (Dual-Mode Surface & Input Separation)**: Both Dark (`#0A0A10` / `#10101A` / `#14141F`) and Light (`#EEF4FA` / `#F4F8FC` / `#FFFFFF`) preserve distinct surface tiers and recessed `input_bg` wells.
- **FC9 (Content-Hugging & Top-Left Sparse States)**: Empty or sparse panels align top-left cleanly without floating centred whitespace or orphaned headers.
- **FC10 (WT Brand Lockup & Unmodified Mark)**: Main header, Settings/About surface, and OS window chrome render the original transparent WT mark (`24px` in-app lockup + `SPACE_8` + `16.0pt` app name + `SPACE_8` + `11.5pt` muted caption) with crisp alpha anti-aliasing in both Dark and Light modes and zero added tile or border.

---

# Part II — App-Specific Sections

Each section below documents only what is unique to an individual WT application beyond Part I.

## 10. App-Specific: `z-controller`

- **Active Controller Cards**: The active virtual controller card renders the `8.0px` Active Gradient Rim (`§5`); inactive/standby cards use standard `panel` surfaces with `1px border`.
- **Battery Instrument Meter**: `124.0 x 8.0 px` single-colour chunky rounded track (`Rounding 4.0` `bg` + `1px border` outer, `Rounding 3.0` inner fill) using `STATUS_GREEN` (`#28D66C`) above `20%` and `STATUS_RED` (`#F04458`) at or below `20%` (never the 3-stop gradient).
- **Live Device Status**: Paired with `8px` (`r = 4.0`) saturated `STATUS_GREEN` / `STATUS_RED` connection indicator dots.

---

## 11. App-Specific: `y-storage`

- **Header & Settings Lockup**: Top header row (`Main`, `Settings`, `Dependencies`) renders `[24x24 WT mark] + 8px + "y-storage" (16.0pt text) + 8px + "Read-only · Last scan: <timestamp> · <duration>s"` (`11.5pt muted`, switching to live progress telemetry while scanning), paired with right-aligned `[ThemeToggle] [Rescan] [Settings / Back to Apps]`. `Settings` closes with an `8px` `panel` About card repeating the `24px` WT lockup (`"y-storage"` + `"v0.1.0 · WT family read-only storage & dependency inspector"`).
- **Drive Instrument Header**: Physical fixed drives sit in horizontal `8px` pods whose headline figure is **free space** (`T1`, `14.5pt` `Consolas Regular`), anchored by a full-width `7px` chunky rounded capacity meter (`Rounding 3.5` outer, `Rounding 2.5` fill) rendered in the unified 3-stop WT horizontal gradient (`LOGO_BLUE #54B9F2 -> LOGO_LAVENDER #D8A6FF -> LOGO_PINK #F7A8D8` mapped across `0%..100%` of the full track width, clipped to fill fraction; saturated `STATUS_RED` only if a drive scan fails or is unreachable).
- **Tabular App Footprint Ledger & Inspection Drawer**:
  - Sticky `26px` sortable column header (`Chevron · App name · Publisher · Category · Growth delta · Total size`), clicking `App name`, `Publisher`, `Category`, `Growth delta`, or `Total size` sorts (`Size` desc default).
  - Fixed `18x18 px` extracted Windows app icon (or consistent neutral folder-tile glyph) keeping column x-positions fixed across rows.
  - `Total size` embeds a quiet `<= 2px` (`PROP_BAR_W = 76.0`, `PROP_BAR_H = 2.0`) proportional bar relative to the largest unfiltered app and exposes exact bytes + `%` share on hover. Missing growth baselines display muted `"new"`.
  - Expanding an app row converts it into an `8px` `panel_alt` inspection drawer wrapped in the continuous **WT Active Gradient Rim** (`§5`), suppressing the row hairline above it, showing its `Category` + rule basis (plus a `used by N apps >` pill when referenced as a dependency), and aligning two sub-tables cleanly to the main `Total size` right edge:
    - `Locations (N)`: `Zone · Path · Basis · Size` (`● GREEN` / `● RED` full-pill zone badges, `BODY` quiet structural pill, `ProgramData (Machine-shared)` lavender pill, segment-boundary `"…"` `Basis` with hover tooltip).
    - `Dependencies`: `Dependency name · Kind · Confidence · Used by · Own size` (`proven` / `inferred` / `shared` / `missing` pills, `SecondaryOutline` `used by N apps >` overlay buttons, segment-boundary `basis` underneath).
- **Collapsible & Resizable Reconciliation Ledger Footer**: Pinned storage groups (`Windows & system`, `Your files`, `Shared & system caches`, `Unattributed`) sit in one `8px` ledger block with a top `6px` drag-resize handle (`156px .. 50%` viewport height), a `"Pinned storage ledger"` collapse chevron folding to a single inline summary line (`Win/sys · Your files · Shared caches · Unattributed` + `● diff 0 B`), and semantic `2px` colour ticks (`muted`, `muted`, `STATUS_GREEN`, muted neutral) right-aligned to the main ledger size column. Beneath them sits the live drive reconciliation equation ending with an `8px` (`r = 4.0`) saturated `STATUS_GREEN` dot + green `diff 0 B` text (or `STATUS_RED` alert pill if `diff != 0`), while the main app list paints a soft `22px` bottom gradient fade above the footer when scrollable content overflows.
- **`y-storage` Domain Critique Checks (`YS1`–`YS3`)**:
  - **YS1 (Shared Size Column Alignment)**: Sticky header `Total size`, main row totals, expanded `Locations`/`Dependencies` size columns, and pinned summary totals share one exact right vertical edge (accounting for the `14px` `6px + SPACE_8` scrollbar gutter).
  - **YS2 (Drive Headline Dominance)**: Drive free space (`14.5pt Consolas Regular`) visibly anchors the top strip above app rows (`13.0pt`).
  - **YS3 (Subordinate Shared Dependencies)**: Inside expanded drawers, `Dependencies` own sizes (`11.5pt` `muted`) visually recede behind the app's own total (`13.0pt` `text`) so shared runtimes never read as additive footprint.
