# InfraPulse Cameroon Backend Architecture

InfraPulse keeps Rust as the backend authority. Actix Web owns HTTP routing, `sqlx` owns PostgreSQL access, and the frontend receives plain JSON from Rust-built workflows.

## Backend Shape

- `organizations`: councils, NGOs, telecom partners, operators, and other client workspaces.
- `projects`: pilot or operational scopes under an organization, with bilingual and field-channel defaults.
- `site_profiles`: real physical places such as water clusters, clinics, schools, towers, probes, and pump stations.
- `survey_campaigns`: offline-first GPS/photo validation campaigns for field agents.
- `infrastructure_assets`: monitored operational assets linked back to projects and sites where possible.
- `field_reports`: human-submitted evidence, now carrying evidence quality and campaign/site/project context.
- `alerts`: operational triggers linked to assets, projects, and sites.
- `maintenance_tickets`: work execution with SLA hours for field dispatch planning.
- `iot_readings`: telemetry from probes and monitored equipment.
- `decision_snapshots`: board/client-ready decision records with priority score, budget estimate, rationale, and next action.

## Cameroon-Market Design Assumptions

- Mobile-first and low-bandwidth: APIs return compact JSON and support offline survey campaign semantics.
- Trust-first: GPS coordinates, evidence quality, site-level proof, confidence scores, and named local context are first-class.
- Bilingual reality: project and survey records keep a `language_mode` field so French/English execution can be modeled explicitly.
- Field operations: alerts, SLA hours, and maintenance tickets are designed for technicians, councils, NGOs, and operators who need clear dispatch decisions.
- Adaptable clients: the same model can support municipal councils, NGOs, water operators, solar operators, telecom partners, and public-asset monitoring.

## Current Core Endpoints

- `GET /api/workspaces/dashboard`
- `GET|POST /api/organizations`
- `GET|POST /api/projects`
- `GET|POST /api/site-profiles`
- `GET|POST /api/survey-campaigns`
- `GET|POST /api/assets`
- `GET|POST /api/reports`
- `GET|POST /api/alerts`
- `PATCH /api/alerts/{id}`
- `GET|POST /api/tickets`
- `PATCH /api/tickets/{id}`
- `GET|POST /api/iot/readings`
- `GET /api/priority-zones`
- `GET /api/decision-report`
- `GET|POST /api/decision-snapshots`

## Rust Module Layout

- `src/main.rs`: process entrypoint only.
- `src/app.rs`: environment loading, database connection, startup sequence, and Actix server boot.
- `src/routes.rs`: HTTP route registration.
- `src/models.rs`: API DTOs, database row models, validation structs, and market constants.
- `src/db.rs`: schema creation and compatibility migrations.
- `src/seeds.rs`: Cameroon administrative seed data and operational demo seed data.
- `src/services.rs`: domain logic, matrix calculations, priority scoring, CSV escaping, and database reads.
- `src/handlers.rs`: Actix HTTP handlers that validate input and call service/database operations.

This keeps Rust as the backend spine while leaving space to grow into separate crates later, for example `infrapulse-domain`, `infrapulse-api`, and `infrapulse-ingestion`.
