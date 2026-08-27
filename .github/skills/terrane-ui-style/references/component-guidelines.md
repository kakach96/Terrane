# Material Component Guidelines

Prefer official Angular Material components. All required modules are already
imported in `frontend/src/app/app.module.ts` — reuse them, do not add custom
UI primitives.

## Component Selection

| Need | Use | Notes |
|------|-----|-------|
| Action button | `mat-button` / `mat-raised-button` / `mat-icon-button` | Primary CTA: `mat-raised-button color="primary"`; secondary: flat `mat-button`; compact icon action: `mat-icon-button` + `matTooltip` |
| Text input | `mat-form-field appearance="outline"` + `matInput` | Outline appearance everywhere for consistency |
| Select | `mat-select` inside `mat-form-field` | |
| Checkbox / toggle | `mat-checkbox` / `mat-slide-toggle` | |
| Card / grouping | `mat-card` | Header + content + actions |
| Table | `mat-table` | Tabular data; add `mat-sort` / `mat-paginator` when needed |
| List / nav | `mat-list` / `mat-nav-list` | |
| Dialog | `MatDialog` service + `mat-dialog-title` / `mat-dialog-content` / `mat-dialog-actions` | Confirmations, forms, pickers |
| Feedback | `MatSnackBar` service | Transient notifications |
| Tooltip | `matTooltip` directive | |
| Menu | `mat-menu` | Overflow actions |
| Tabs | `mat-tab-group` | Sub-navigation within a page |
| Progress | `mat-spinner` / `mat-progress-bar` | Loading states |
| Status | `mat-chip` / `mat-badge` | Tags, counts |
| Icon | `mat-icon` | Material Icons only |

## Best Practices

- **Buttons**: one primary action per view (`mat-raised-button color="primary"`).
  Use `mat-icon-button` for compact icon actions with a `matTooltip`.
- **Forms**: `appearance="outline"` everywhere; label + placeholder; full-width
  fields in a responsive grid; disable submit until the form is valid.
- **Cards**: `mat-card-header` for title/subtitle, `mat-card-content` for body,
  `mat-card-actions` right-aligned for actions.
- **Tables**: `mat-table` with column headers, hover rows, and an empty state.
- **Dialogs**: clear title, body, and explicit confirm/cancel actions; close on
  cancel and after a successful submit.
- **Feedback**: `MatSnackBar` for success/error toasts; `MatDialog` for
  blocking decisions.
- **Icons**: use `mat-icon` with Material Icons; never inline SVG or emoji for UI.

## Anti-patterns

- Hand-rolled `<button>`, `<input>`, `<select>`, `<table>` without Material
- Hardcoded colors / spacing / shadows instead of design tokens
- Dense layouts with no whitespace
- Multiple competing primary buttons on one view
- Low-contrast text or missing focus states