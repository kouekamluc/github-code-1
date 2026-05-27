# Cameroon Mobile Phone Monitor

A full-stack web application with a Rust backend, PostgreSQL persistence, and a Bootstrap + Leaflet frontend for tracking mobile phone ownership across Cameroon.

## Features

- Rust backend using `actix-web`
- PostgreSQL storage with `sqlx`
- Dynamic hierarchical region data: region, department, commune
- Map-based visualization using Leaflet
- Summary cards, filters, and an update form

## Setup

1. Install Rust and Cargo: https://www.rust-lang.org/tools/install
2. Start PostgreSQL. With Docker Desktop running, use:

```bash
docker compose up -d
```

Or install PostgreSQL locally and create a database:

```bash
createdb cameroon_stats
```

3. Copy `.env.example` to `.env` and update the connection string if needed. The default local value is:

```env
DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/cameroon_stats
SERVER_HOST=127.0.0.1
SERVER_PORT=8081
```

4. Run the app from the project root:

```bash
cargo run
```

5. Open `http://127.0.0.1:8081` in your browser.

## Environment

- `DATABASE_URL` must point to your PostgreSQL instance, for example:

```env
DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/cameroon_stats
```

## Endpoints

- `GET /api/summary` - national totals and hierarchy counts
- `GET /api/auth/context` - request actor, role, and first-pass permissions from headers
- `GET /api/audit-events` - workflow audit trail, optionally filtered by entity type and id
- `GET|POST /api/operator-imei-events` - operator/API IMEI compliance feed for Cameroon device-clearance events
- `GET /api/overview` - Rust-computed business cockpit with KPIs, opportunity pipeline, trust risks, and next actions
- `GET /api/stats` - detailed dataset by region/department/commune
- `POST /api/stats/update` - insert or update a location record
- `GET /api/workspaces/dashboard` - workspace health, projects, sites, campaigns, and recent decisions
- `GET|POST /api/site-profiles` - physical field sites linked to projects
- `GET|POST /api/survey-campaigns` - offline-first GPS/photo survey campaign planning
- `GET|POST /api/decision-snapshots` - decision records with score, budget, rationale, and next action

## Frontend

The frontend is served from `static/index.html` and uses `static/app.js` to display the map, hierarchical filters, and detail table.

## Local development notes

- The backend creates the `mobile_phone_stats` table on startup if it does not already exist.
- The backend seeds Cameroon administrative data automatically from `data/cameroon_admin3_seed.tsv`.
- The seed covers 10 regions, 58 departments, and 360 commune/arrondissement-level administrative units.
- Administrative names, p-codes, areas, and GPS centroids come from OCHA COD-AB Cameroon, sourced from Institut National de Cartographie (INC), reviewed 30 October 2025, valid from 04 January 2019.
- Phone ownership and population metrics are calculated by a transparent matrix when measured local values are missing.
- The matrix uses the OCHA GPS centroid and area, region-level weighting, proximity to major urban anchors, 2025 Cameroon population, and the 2024 World Bank mobile-subscription baseline.
- Manual measured updates override matrix estimates for the matching administrative unit.
- Operator IMEI events are stored as hashed device identifiers with optional last-four matching, supporting Cameroon customs/operator compliance workflows without storing subscriber identity.
- Docker Desktop must be running before `docker compose up -d` will work on Windows.

## Production readiness checklist

- Replace sample figures with a trusted data source, documented collection dates, and source attribution.
- Integrate a verified telecom/population dataset if commune-level phone ownership is required.
- Add authentication and role-based permissions before allowing location updates.
- Move schema changes into versioned migrations instead of startup DDL.
- Add automated backend tests for validation, summaries, and update behavior.
- Add frontend tests for filtering, form validation, and map rendering.
- Add observability: structured logs, health checks, request metrics, and error tracking.
- Configure production secrets outside `.env` files and rotate database credentials.
- Serve through HTTPS behind a reverse proxy, with CORS and security headers configured.
- Add database backups, restore testing, and deployment rollback procedures.
- Decide hosting for map tiles or use a paid tile provider that allows production traffic.
