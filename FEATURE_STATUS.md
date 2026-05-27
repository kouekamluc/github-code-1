# Feature Status And Implementation Plan

Last checked: 2026-05-27

## End-To-End Status

The current local build passes the core backend workflow from public read surfaces through authenticated operations, evidence capture, workflow transitions, IMEI intake, decision approval, execution planning, audit logs, HTMX fragments, and CSV exports.

Run the same check with:

```powershell
.\scripts\feature_status_check.ps1
```

## Feature Readiness Matrix

| Feature | Current status | Verified path | Remaining production work |
| --- | --- | --- | --- |
| Public cockpit | Working | Summary, overview KPIs, risks, next actions | Add configurable tenant/client views |
| Phone matrix | Working | Matrix, assumptions, area detail, recalculation endpoint | Add source versioning and admin review for manual overrides |
| Workspace management | Working | Organization, project, dashboard, readiness cards | Add tenant isolation and workspace-level permissions |
| Site profiles | Working | Site creation linked to project and area dossier | Add duplicate review and map-assisted site placement |
| Survey campaigns | Working core | Campaign create and status transition | Implement form builder, offline submissions, photos, and field-agent assignments |
| Signal probes/assets | Working core | Asset create, linked site/project, status workflow | Add device registry keys, heartbeat rules, and asset detail pages |
| Field reports | Working core | Report create linked to campaign/site/asset | Add file/photo upload and evidence review workflow |
| Alerts | Working core | Alert create, acknowledge, ticket link | Add automatic rules from telemetry thresholds and missed heartbeats |
| Maintenance tickets | Working core | Ticket create, start, complete, linked alert resolution | Add technician calendar, SLA escalation, completion evidence |
| Telemetry | Working core | Reading create linked to asset/site/project | Add ingestion tokens, batch ingest, and anomaly detection |
| Operator IMEI API | Working intake | Event ingest, hash/last-four storage, compliance summary | Add per-ISP connectors, webhook signatures, batch upload, rate limits, and legal retention rules |
| Decision board | Working core | Linked decision create, approval validation | Add richer approval notes, attachments, and decision history view |
| Execution board | Working core | Plan creation from approved decision, status transition | Add execution checklist editing and outcome evidence |
| Area dossier | Working | Area dossier joins sites, assets, campaigns, reports, alerts, tickets, readings | Add printable dossier and related-record deep links |
| HTMX fragments | Working | Ops status and workspace activity auto-refresh | Expand fragments to ticket queue, alert queue, and execution board |
| Auth/RBAC | Working core | Login, session context, permission checks | Add user management UI, password reset, org membership, session revocation |
| Audit logs | Working core | Workflow audit events readable by admin | Add audit viewer UI and export filters |
| CSV exports | Working | Assets, tickets, priority zones, phone matrix | Add reports, campaigns, sites, decisions, IMEI compliance exports |

## Implementation Plan

### Phase 1: Make Existing Core Workflows Product-Complete

1. Add detail pages/drawers for project, site, campaign, asset, report, alert, ticket, decision, execution plan, and IMEI event records.
2. Add edit/update endpoints for non-status fields so records are not create-only after initial submission.
3. Add delete/archive semantics for mistakes, using soft-delete or archived status where operational history matters.
4. Add the audit-log UI and show record history inside detail pages.

### Phase 2: Evidence And Field Operations

1. Implement file/photo upload for reports and ticket completion.
2. Add survey form templates with field definitions, offline payload storage, GPS/photo requirements, and submission review.
3. Add technician/team assignment and SLA escalation rules.
4. Add automatic alert creation from telemetry thresholds, missing heartbeat windows, repeated failed reports, and unresolved SLA breaches.

### Phase 3: IMEI/ISP Production Intake

1. Create operator connector records for MTN Cameroon, Orange Cameroun, Camtel, and future ISP/customs feeds.
2. Add authenticated webhook and batch ingest endpoints with per-operator API keys, HMAC signatures, replay protection, and rate limits.
3. Normalize raw ISP payloads into `operator_imei_events` while hashing IMEI values before storage.
4. Add compliance reconciliation jobs for pending, cleared, blocked, customs-cleared, and unknown device states.
5. Add retention, redaction, export, and audit policies for legal compliance.

### Phase 4: Tenantization And Production Hardening

1. Add organization membership and workspace-scoped permissions.
2. Move startup DDL into versioned migrations.
3. Add health checks, structured logs, metrics, backups, and restore tests.
4. Add automated test suites for backend services, API workflows, and browser-critical frontend paths.
5. Add deployment scripts, HTTPS/reverse-proxy config, secret rotation, and environment-specific configuration.

## Current Priority Recommendation

The app is past demo-only backend wiring for the core flow. The next normal implementation step should be detail/edit pages plus evidence uploads, because those two unlock real operational use: people can inspect a record, correct it, attach proof, and follow the history from intake to decision to execution.
