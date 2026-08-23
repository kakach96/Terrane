# Terrane — Copilot instructions

> Entry point for GitHub Copilot (GitHub.com / PR context). The authoritative,
> full project instructions live in [`AGENTS.md`](../AGENTS.md) at the repo
> root — always read and follow it before writing or modifying code.

## Project overview

Terrane is a cloud-native spatial data server (a modern re-implementation of
GeoServer) powered by a **Rust (Actix-web)** backend and an **Angular 17 +
Material** frontend.

- Architecture & API contracts: `docs/ARCHITECTURE.md`
- Roadmap & milestones: `docs/ROADMAP.md`
- Local setup & conventions: `docs/DEVELOPMENT.md`
- Cloud-native status: `docs/IMPLEMENTATION_PLAN.md` §6

## Repository layout

| Path        | Description                                                                  |
| ----------- | ---------------------------------------------------------------------------- |
| `service/`  | Rust backend — `service/src/handlers/` (REST + OGC WMS/WFS/WCS/WMTS), `models/`, `store/` (SQLite/PostgreSQL, vector/raster/cache), `routes.rs`; `static/` (built frontend), `tests/`, `benches/`, `Cargo.toml` |
| `frontend/` | Angular 17 + Material — `src/app/components/`, `services/`, `models/`        |
| `docs/`     | Architecture, protocols, roadmap, development guides                         |

## Key conventions

- **Code comments & file descriptions must be written in English** (applies to
  Rust, TypeScript, configs, scripts, and Docker files alike).
- Commit message format: `type: changes content` — `type` is `feat`, `fix`,
  `refactor`, `chore`, etc.
- Backend is Windows-native (`build/build.bat`); API base path is `/geoserver`
  (`service/terrane.toml: api_context`).
- Storage is split: metadata store (SQLite/PostgreSQL) vs vector store vs
  raster store vs cache — see `AGENTS.md` "Storage split".
- **Run the CI checks before every commit** (fmt + clippy + test + frontend
  build + lint) and fix any issues so the CI pipeline does not fail after push —
  see `AGENTS.md` "Pre-commit CI checks". CI runs `cargo test --all` and the
  frontend build (`.github/workflows/ci.yml`).

## Tooling (installed)

- Rust formatting: `service/.rustfmt.toml` (run `cargo fmt` from `service/`)
- Frontend lint: `npm run lint` (ESLint flat config, `eslint.config.js`)
- Frontend format: `npm run format` / `npm run format:check` (Prettier, `.prettierrc`)
