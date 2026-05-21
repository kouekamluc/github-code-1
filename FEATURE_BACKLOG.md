# InfraPulse Cameroon Feature Backlog

## Implemented in this batch

- CSV exports for monitored assets, maintenance tickets, and priority zones.
- Decision report KPIs for active and overdue maintenance tickets.
- Maintenance ticket workflow actions for start, block, and done.
- UI export controls in the Decision Report workspace.

## Still missing before a serious pilot

- User accounts, organizations, roles, and audit logs.
- Project/workspace separation for different councils, NGOs, and companies.
- Site profiles that group assets, reports, tickets, alerts, and IoT readings under one physical place.
- Survey campaign builder with reusable forms, offline field collection, photos, and GPS proof.
- File/photo uploads for field reports and maintenance completion evidence.
- Notification channels for SMS, email, WhatsApp, and Slack-style operations alerts.
- IoT device registry with device keys, heartbeat checks, ingestion tokens, and bad-signal quarantine.
- Automatic alert rules from telemetry thresholds, missed heartbeats, and repeated field reports.
- SLA policies by priority, overdue escalation, and technician assignment calendars.
- Public transparency portal for councils and donor-funded projects.
- PDF report generation for monthly donor/client packs.
- Billing/subscription plans and customer onboarding flows.
- Production hardening: authentication, rate limits, backups, migrations, observability, and deployment scripts.

## Recommended next implementation order

1. Organizations and project workspaces.
2. Site profiles and asset grouping.
3. Survey campaigns with GPS/photo evidence.
4. IoT device registry and telemetry ingestion keys.
5. Automatic alert rules and SLA escalation.
6. PDF report export.
7. Authentication and role-based access.
