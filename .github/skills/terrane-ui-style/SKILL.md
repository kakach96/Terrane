---
name: terrane-ui-style
description: 'Terrane frontend UI style constraints (Material Design). Use when: building or modifying Angular frontend UI — components, pages, dialogs, forms, tables, styles, layouts. Enforces official Angular Material components, simple pages with generous whitespace, rounded corners, layered shadows, and accessible color contrast.'
user-invocable: true
---

# Terrane UI Style (Material Design)

## What This Skill Does

Codifies the Terrane frontend visual language so every page looks consistent:
Material Design, official Angular Material components, simple pages with
generous whitespace, rounded corners, layered shadows, and accessible color
contrast.

## When to Use

- Creating or editing any Angular component, page, dialog, or form under `frontend/src/app/`
- Writing or refactoring SCSS for the frontend
- Choosing which UI component to use for a feature
- Reviewing a UI change for style consistency

## Core Principles

1. **Material-first** — Prefer official Angular Material components (`mat-*`).
   Never hand-roll buttons, inputs, selects, tables, dialogs, or menus.
2. **Simplicity** — Keep pages clean and focused. Generous whitespace, minimal
   decoration, one primary action per view.
3. **Visual language** — Rounded corners (8–16px), soft layered shadows, and
   accessible color contrast (WCAG AA).
4. **Consistency** — Always reuse the global design tokens from
   `frontend/src/styles.scss` (`--primary-color`, `--radius-md`, `--shadow-sm`,
   ...). Never hardcode colors, spacing, radii, or shadows.
5. **Accessibility** — Sufficient contrast, visible focus states, semantic
   structure, keyboard navigation.

## Procedure

1. Read the relevant reference before building UI:
   - [Design tokens](./references/design-tokens.md) — colors, typography, spacing, radii, shadows
   - [Component guidelines](./references/component-guidelines.md) — which `mat-*` component to use and how
   - [Page patterns](./references/page-patterns.md) — list / detail / form / dialog layouts
2. Prefer official Material components over custom HTML/CSS.
3. Use design tokens for all colors, spacing, radii, and shadows.
4. Keep the page simple: one clear purpose, generous whitespace, minimal decoration.
5. Verify contrast and focus states for interactive elements.

## Do / Don't

| Do | Don't |
|----|-------|
| Use `mat-*` components (button, card, form-field, table, dialog, ...) | Hand-roll custom buttons, inputs, tables, or dialogs |
| Use design tokens from `styles.scss` | Hardcode hex colors, px spacing, or shadows |
| Keep generous whitespace and one primary action | Cram dense layouts or multiple competing CTAs |
| Use rounded corners (8–16px) and soft shadows | Sharp corners, harsh shadows, heavy borders |
| Ensure WCAG AA contrast and visible focus states | Low-contrast text, invisible focus indicators |
| Use `mat-icon` + Material Icons | Inline SVG or emoji for UI icons |
| Use `\| translate` pipe for all user-facing text | Hardcoded UI strings |