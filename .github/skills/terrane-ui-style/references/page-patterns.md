# Page Patterns

Reference layouts for the common Terrane page types. Follow these to keep every
page consistent.

## List Page (e.g. Layers, Data Sources, Workspaces)

```
page-header (title + subtitle + primary action)
filters-bar (search field + selects + refresh)
  → loading: centered mat-spinner
  → empty: empty-state (icon + title + hint + CTA)
  → data: mat-card grid OR mat-table
```

- `page-header`: title (28px, 600) + subtitle (14px, secondary) + one primary
  action on the right
- `filters-bar`: white card, `--radius-lg`, `--shadow-sm`, 16px gap
- Cards: `mat-card` with `--radius-lg`, hover `--shadow-md`
- Empty state: centered icon (80px) + title + hint + CTA

## Detail Page (e.g. Layer Detail)

- Back link / breadcrumb at the top
- `mat-card` sections grouping related information
- Key-value rows with secondary labels
- Actions in `mat-card-actions` (right-aligned)

## Form Page (e.g. Create Layer, Data Source)

- `mat-card` containing the form
- `mat-form-field appearance="outline"`, full-width in a responsive grid
- Section headers for grouped fields
- Footer actions: primary submit + cancel, aligned consistently

## Dialog (e.g. Seed Job, Style Editor)

- `mat-dialog-title` + `mat-dialog-content` + `mat-dialog-actions`
- Explicit confirm (primary) and cancel buttons
- Close on cancel and after a successful submit

## States

- **Loading**: centered `mat-spinner` + label
- **Empty**: icon + title + hint + optional CTA
- **Error**: `mat-snack-bar` with the error message; keep page state intact