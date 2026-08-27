# Design Tokens

Source of truth: `frontend/src/styles.scss` (`:root`). Always reference these
CSS variables — never hardcode values in component styles.

## Colors

| Token | Value | Usage |
|-------|-------|-------|
| `--primary-color` | `#3f51b5` | Primary actions, links, active states |
| `--primary-light` | `#7986cb` | Hover / light primary surfaces |
| `--primary-dark` | `#303f9f` | Pressed / emphasis |
| `--accent-color` | `#009688` | Accent highlights, secondary actions |
| `--accent-light` | `#4db6ac` | Accent hover surfaces |
| `--accent-dark` | `#00796b` | Accent pressed |
| `--warn-color` | `#f44336` | Errors, destructive actions |
| `--success-color` | `#4caf50` | Success states |
| `--info-color` | `#2196f3` | Informational states |
| `--bg-main` | `#f5f5f7` | Page background |
| `--bg-card` | `#ffffff` | Card / surface background |
| `--text-primary` | `#1a1a2e` | Primary text |
| `--text-secondary` | `#666680` | Secondary / muted text |
| `--border-color` | `#e0e0e8` | Dividers, borders |

## Typography

- Font family: `Inter` (400/500/600/700), self-hosted via `@fontsource/inter`
- Mono: `JetBrains Mono` for code / coordinates / bounds values
- Headings: 600–700 weight; page title 28px, card title 18px
- Body: 14px regular; secondary text 14px in `--text-secondary`

## Spacing

Use a consistent 8px-based scale: 4 / 8 / 12 / 16 / 24 / 32 / 48.

- Card padding: 24px
- Page header bottom margin: 32px
- Filters bar gap: 16px
- Form field gaps: 16–24px

## Radii

| Token | Value | Usage |
|-------|-------|-------|
| `--radius-sm` | 8px | Small elements, chips |
| `--radius-md` | 12px | Cards, inputs, buttons |
| `--radius-lg` | 16px | Large cards, filters bar, empty states |

## Shadows (elevation)

| Token | Value | Usage |
|-------|-------|-------|
| `--shadow-sm` | `0 2px 8px rgba(0,0,0,0.08)` | Default cards |
| `--shadow-md` | `0 4px 16px rgba(0,0,0,0.12)` | Hover, dialogs |
| `--shadow-lg` | `0 8px 32px rgba(0,0,0,0.16)` | Overlays, modals |

## Icons

- Use `mat-icon` with Material Icons (self-hosted `material-icons` package)
- Standard size 24px; large empty-state icons 80px
- Semantic icons: `add` (create), `search` (filter), `refresh` (reload),
  `visibility` (preview), `layers` (layers), `folder` (workspace/store),
  `storage` (data source), `grid_view` (tiles), `monitoring` (server)