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
2. Install PostgreSQL and create a database:

```bash
createdb cameroon_stats
```

3. Copy `.env.example` to `.env` and update the connection string if needed.

4. Run the app from the project root:

```bash
cargo run
```

5. Open `http://127.0.0.1:8080` in your browser.

## Environment

- `DATABASE_URL` must point to your PostgreSQL instance, for example:

```env
DATABASE_URL=postgres://postgres:postgres@127.0.0.1/cameroon_stats
```

## Endpoints

- `GET /api/summary` - national totals and hierarchy counts
- `GET /api/stats` - detailed dataset by region/department/commune
- `POST /api/stats/update` - insert or update a location record

## Frontend

The frontend is served from `static/index.html` and uses `static/app.js` to display the map, hierarchical filters, and detail table.
