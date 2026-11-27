# WT Native Instrument Design System (`DESIGN.md`)

> **Canonical Ecosystem Standard**: This document is the canonical design standard for **all WT applications** (`z-controller`, `y-storage`, `WT Manager`, and future WT native utilities).
> **Reference Implementation**: **`z-controller`** (`Y:\Repos\z-controller`) and **`WT_UI_LIBRARY.md §0`** (`C:\Users\Win\.gemini\config\skills\wt-ui-library\references\WT_UI_LIBRARY.md`) define the monochrome theme engine, shade ladder, neutral surfaces, and shared control geometry.
> **Adoption Rule**: Any WT application adopting this standard MUST copy `DESIGN.md` into its repository root and implement **Part I — Shared Core** unchanged. Application-specific layouts, domain widgets, and intentional deviations belong strictly inside that application's own section under **Part II — App-Specific Sections**.

**Last Updated**: 2026-11-28 (`theme-engine` — synced with `WT_UI_LIBRARY.md §0` v2 Monochrome Theme Engine)

---

# Part I — Shared Core (WT Family Standard)

All rules, tokens, geometries, and component behaviours in Part I apply identically across every WT application.

## 1. North Star & Anti-Patterns

WT applications are precision instruments, not dashboards: every pixel, colour, border, and type tier must encode real system state or measured quantity, and nothing exists as decoration for its own sake. We design explicitly against the **generic AI UI failure mode**:

1. Repeating identical bordered cards around every list or ledger row (`cards-inside-cards` when expanded).
2. Applying a single uniform corner radius (`6px` on panels, badges, tracks, and buttons alike).
3. Mixing multiple brand hues on one UI screen (`Blue` vs `Pink` vs `Lavender`), tinting canvas/panel backgrounds with theme colour, or using cross-hue gradients on functional bars, rims, or buttons.
4. Centring empty-state labels or orphaned section headers in dead vertical space.
5. Rendering secondary metadata (publishers, provenance/basis strings, kind labels, supporting counts) at equal size and brightness as primary quantities.
6. Placing badge chips on static labels (`READ-ONLY`, mode banners, descriptors) rather than live states or user decisions, or conveying semantic status via colour alone.

---

## 2. Monochrome Theme Engine & Colour Tokens (`WT_UI_LIBRARY.md §0` v2)

All WT applications generate UI accents from a **single selected base colour** (`ThemeAccent`) rendered as a **4-step monochrome shade ladder** over **strictly neutral** black/white/grey surfaces (`Dark` or `Light`). Theme (`preset` or `custom` + `Dark` / `Light`) is an app-level user setting, never a per-component variant choice.

### 2.1 Neutral Surface, Border & Typography Luminance Tokens (Never Theme-Tinted)

| Token | Dark Mode (`R=G=B`) | Light Mode (`R=G=B`) | Role |
|---|---|---|---|
| `bg` (`S0` Canvas) | `#0C0C0C` | `#F4F4F4` | Window backdrop and header canvas |
| `panel_alt` (`S1` Recessed) | `#141414` | `#EAEAEA` | Toolbar headers, expanded drawers, pinned footer |
| `panel` (`S2` Primary) | `#191919` | `#FFFFFF` | Cards, ledger wrappers, modal & picker surfaces |
| `widget` (`S3` Rest) | `#202020` | `#F7F7F7` | Neutral interactive surfaces & selected cards |
| `widget_hover` (`S3` Hover) | `#2A2A2A` | `#EDEDED` | Neutral hover backdrop on subtle controls |
| `widget_active` (`S3` Press) | `#333333` | `#E2E2E2` | Neutral pressed surface |
| `input_bg` (`Well`) | `#101010` | `#F4F4F4` on `panel` | Recessed text input wells (`#FFFFFF` if directly on `bg`) |
| `border` (`Hairline`) | `#2E2E2E` | `#D6D6D6` | `1px` crisp structural borders, SubtleOutline strokes, hairline dividers |
| `text` | `#F0F0F0` | `#1A1A1A` | Primary headlines, card identities, dominant quantities |
| `muted` | `#A0A0A0` | `#5E5E5E` | Secondary metadata, quiet totals, SubtleOutline labels, disabled text |
| `subtle_text` | `#858585` | `#757575` | Inline descriptors, publishers, hexadecimal `VID:PID` |
| `hint_text` | `#666666` | `#999999` | Unfocused, empty input placeholder hints only |

### 2.2 Monochrome Shade Ladder, Presets & Custom Normalization

From one base colour (`source`), the engine derives four within-family steps:

1. **Deep / Ink step (`deep`)**: Primary solid button fills on Light, checked marks/ink, small text and borders on `#FFFFFF` (contrast-normalised to `>= 4.5:1` on white; curated `#0090D9` for Blue, `#FA29AA` for Pink).
2. **Accent step (`accent_dark` / `accent_line`)**: Dark-mode borders, active glyphs, toggles, chips, and rim center stop (contrast-normalised to `>= 5.5:1` on `#191919` Dark panels; curated `#69CBFC` for Blue, `#FA80CB` for Pink; Light mode binds `accent_line = deep`).
3. **Source step (`source`)**: Original preset/custom hue (`#7CCBF2` for Blue) used for soft highlights, large swatches, and bright rim endpoint.
4. **Pastel / Soft Fill step (`soft_fill` / `soft_fill_hover`)**: `accent_dark` blended at `16%` / `26%` over `#191919` in Dark mode; `source` blended at `25%` / `35%` over `#FFFFFF` in Light mode.
5. **Solid Interactive Fill (`solid_fill` / `on_solid`)**: Pressed controls, checked checkboxes, and latched CTAs (`accent_dark` in Dark, `deep` in Light) paired with contrast-maximised ink (`>= 4.5:1`).

**Curated App-Level Presets**:
- **Blue (`#7CCBF2`, default)** -> Light `#0090D9` / Dark `#69CBFC` / Source `#7CCBF2`
- **Pink (`#F2A9D6`)** -> Light `#FA29AA` / Dark `#FA80CB` / Source `#F2A9D6`
- **Purple (`#9B8AFB`)**, **Green (`#3ECF6E`)**, **Orange (`#FB923C`)**, **Yellow (`#FACC15`)**, **Red (`#F87171`)** -> contrast-normalised automatically into the 4-step ladder
- **Custom RGB / `#RRGGBB` input**: Normalised in HSL space until `deep` achieves `>= 4.5:1` on `#FFFFFF` and `accent_dark` achieves `>= 5.5:1` on `#191919`; raw low-contrast user hexes are never rendered as text directly.

### 2.3 Fixed Semantic Status Tokens (`WT_UI_LIBRARY.md §2.4` & `§4B`)

Status colours belong to a fixed semantic family orthogonal to the user's chosen UI theme:

| Token | Dark Mode | Light Ink | Role |
|---|---|---|---|
| `STATUS_GREEN` | `#28D66C` | `#0A7A3C` | Positive / Connected / Working / Cloaked (`✓` icon + label) |
| `STATUS_AMBER` | `#F5B83D` | `#A66C07` | Warning / Near-limit (`!` icon + label) |
| `STATUS_RED` | `#F04458` | `#E02444` | Stopped / Missing / Low battery `<= 20%` / Danger (`✕` / `!` icon + label) |

---

## 3. Typography Scale & Luminance Hierarchy

WT applications pair **`Segoe UI`** for proportional interface text with **`Consolas`** monospace tabular numerals for measured quantities, coordinates, and timestamps. Minimum readable type floor across all UI copy: **`10.5pt`**. No rendered text may be smaller than `10.5pt`.

- **Numeral Weight Rule**: All numerals (`Total size`, `Own size`, `RAM`, versions, `VID:PID`, percentages) use `Consolas Regular` — NEVER Strong/Bold.
- **Strong Weight Rule**: **`Strong` weight (`Segoe UI Semibold`) is reserved strictly for row/card primary identities and modal/drawer titles.** Headlines, metadata, badges, and buttons stay Regular.

---

## 4. Spacing Scale & Corner Radii

### 4.1 Spacing Scale (Multiples of 4 Only)
`SPACE_4 = 4.0`, `SPACE_8 = 8.0`, `SPACE_12 = 12.0`, `SPACE_16 = 16.0`. All outer margins, panel padding, header control gaps, and vertical section rhythms derive strictly from `{4, 8, 12, 16}`.

### 4.2 Corner-Radius Scale (Differentiated by Role)
- **`0.5 * badge_height` (`9.0px` on `18px` pills)**: Non-interactive semantic/constraint pill badges.
- **`8.0px` (`CARD_ROUNDING`)**: Elevated cards, modals, theme picker popover, active gradient rims.
- **`6.0px` (`RADIUS_CONTROL`)**: Outline buttons, `ThemeToggle`, `ThemeSwatch`, text inputs, ComboBoxes.
- **`4.0px` (`RADIUS_PILL`)**: `16x16` checkboxes, `20x20` collapse chevrons, compact preset swatches.
- **`3.5px`–`4.0px` outer / `2.5px`–`3.0px` fill**: Rounded meter tracks.

---

## 5. Within-Family Highlight Rim & Meter Sweep Rule

- **No cross-hue gradients in UI**: The blue→pink logo gradient belongs **exclusively** to the WT logo mark / app icon (`assets/logo-mark.png` / `.ico`) and MUST NOT appear on card rims, meter bars, progress bars, or buttons.
- **Active Container Rim (`8.0px`, `SEGS = 16`, `BANDS = 5`)**: Active controller cards and expanded drawers render the continuous inner-falloff perimeter rim (`offsets = [-4.5, -2.0, -1.6, -0.4, 0.0]`, `alphas = [0, 52, 255, 255, 0]`) using the active theme's **within-family 3-stop monochrome sweep** (`pal.rim_stops`).
- **Meters & Progress Bars**: Healthy/operational meters (triggers, capacity bars, battery level `> 20%`) use the current theme's **within-family monochrome sweep** or solid theme accent — never cross-hue mixing and never button gradients.

---

## 6. Family Button & Interactive Control Standard

All interactive buttons strictly forbid gradient fills or gradient borders:

| Variant / State | Background Fill | `1px` Outline Stroke | Text / Glyph Foreground | Typical Use |
|---|---|---|---|---|
| **`PrimaryOutline`** (rest) | `soft_fill` | `accent_line` | `pill_ink` | Primary CTA (`Edit Profile`, `Install …`, `Apply`) |
| **`SecondaryOutline`** (rest) | `TRANSPARENT` | `lerp(border, accent_line, 0.72)` | `pill_ink` | Header actions (`ThemeSwatch`, `ThemeToggle`, `Refresh`, `Locate…`, `+ New`) |
| **`SubtleOutline`** (rest) | `TRANSPARENT` | `border` | `muted` | Utility / dismissive actions (`Cancel`, `Clear`, `Dismiss`, `Delete`, `×`) |
| **Hovered** (theme variant) | `soft_fill_hover` / `widget_hover` | `accent_line` | `text` | Pointer hover across enabled controls |
| **Pressed / Latched** | `solid_fill` | `accent_line` | `on_solid` | Pointer-down press & active/dirty latched state (`Apply & Save *`) |
| **`DangerStatus`** (`§4B S4`) | `pill_red_fill` (solid `STATUS_RED` press) | `status_red_text` / `STATUS_RED` | `status_red_text` (white press) | Destructive / system reboot actions (`Restart Windows`) |

- **Scrollbars**: Thumb uses theme accent blended at `50%` over neutral surface at rest, `100%` `accent_line` on hover/drag, width `6.0px`, `handle_min_length = 24.0`.
- **WT Header Lockup (`§6.4`)**: Left-aligned `24x24` unmodified `assets/logo-mark.png` + `8px` + `16.0pt` Regular app name + `8px` + `11.5pt` muted caption; right-aligned `[ThemeSwatch][ThemeToggle][Primary/Secondary Header CTAs]`.

---

## 7. Chip Discipline

Non-interactive semantic pill badges (`rounding = 0.5 * height`, `10.5pt` Regular) encode live states or requirements (`REQUIRED`, `OPTIONAL`, `-> Xbox 360`). Never wrap static descriptors or mode banners in bordered badge chips.

---

## 8. Status-Colour Saturation & Non-Colour Redundancy Rule (`§0.8`)

Saturated Green/Amber/Red (`STATUS_GREEN` / `STATUS_AMBER` / `STATUS_RED`) is reserved strictly for real semantic status. Because the monochrome theme engine allows selecting **Green**, **Orange**, **Yellow**, or **Red** themes, every semantic status indicator **MUST always pair colour with an unambiguous vector icon AND text label**:
- **Healthy / Working / Connected / Active / Cloaked**: `12px` `STATUS_GREEN` circle with crisp vector **`✓`** + paired status label (`Working`, `OK`, `Cloaked`, `Active`).
- **Stopped / Missing / Visible / Disconnected**: `12px` `STATUS_RED` circle with crisp vector **`✕`** + paired status label (`Stopped`, `Missing`, `Visible`, etc.).
- **Critical Battery (`<= 20%`)**: Solid `STATUS_RED` bar + vector **`!`** warning triangle + `"{pct}% Low"` text.

---

## 9. Shared Layout & Family Critique Checklist (`FC1`–`FC10`)

- **FC1 (No Card Soup)**: Ledger items stay inside hairline-divided panels rather than nested card soup.
- **FC2 (Restrained Numeral Weight)**: Measured figures use `Consolas Regular`; `Segoe UI Semibold` is reserved for primary item names & modal/drawer titles.
- **FC3 (Within-Family Monochrome Rims & Meters)**: Highlight rims and operational meters sweep strictly within the active single-hue theme ladder (zero cross-hue gradient outside the WT logo mark/icon).
- **FC4 (Multi-Tier Corner Radii)**: Distinct radii (`9px` pills, `8px` panels/rims, `6px` buttons/inputs, `4px` checkboxes/swatches/chevrons, `3.5px–4px` meters).
- **FC5 (Status Icon + Label Redundancy)**: Every green/amber/red status indicator pairs semantic colour with `✓` / `!` / `✕` icon + label.
- **FC6 (Outline Button Contrast & State Ladder)**: Clean `1px` crisp rings, monochrome hover tint, and `>= 4.5:1` contrast solid pressed states; zero button gradients.
- **FC7 (`>= 10.5pt` Floor)**: All visible text, numerals, badges, and tooltips are `>= 10.5pt`.
- **FC8 (Pure Neutral Backgrounds)**: Dark `#0C0C0C`/`#141414`/`#191919` and Light `#F4F4F4`/`#EAEAEA`/`#FFFFFF` surfaces have zero theme tint across all presets & custom colours.
- **FC9 (Content-Hugging)**: Sparse states align top-left and compact windows hug content.
- **FC10 (Unmodified WT Mark Lockup & App Icon)**: Original transparent WT mark (`24px` header lockup + multi-size `.ico`).

---

# Part II — App-Specific Sections

Each section below documents only what is unique to an individual WT application beyond Part I.

## 10. App-Specific: `z-controller`

- **Active Controller Cards**: Active virtual controller cards render the `8.0px` within-family monochrome rim (`§5`); standby cards use neutral `panel` surfaces with `1px border`.
- **Battery Instrument Meter**: `124.0 x 8.0 px` rounded track (`Rounding 4.0` outer / `3.0` fill) using the active theme's **within-family monochrome sweep** above `20%`, and semantic `STATUS_RED` + `!` warning icon + `Low` label at or below `20%`.
- **Live Device Status**: Paired with `12px` semantic `✓` / `✕` status icons (`STATUS_GREEN` / `STATUS_RED`) and explicit labels.
- **Theme Setting Persistence**: `theme_mode` (`Dark`/`Light`) + `theme_accent` (Preset `Blue` default, `Pink`, `Purple`, `Green`, `Orange`, `Yellow`, `Red`, or contrast-normalised `Custom` RGB/hex) persist in `%APPDATA%\z-controller\config.toml`.
