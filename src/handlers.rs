use std::path::{Path, PathBuf};

use actix_web::{get, patch, post, web, HttpRequest, HttpResponse, Responder};
use base64::Engine;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::models::*;
use crate::services::*;
use crate::workflow::{
    validate_decision_approval, validate_execution_completion, validate_execution_plan_creation,
    validate_ticket_completion, validate_transition, WorkflowKind,
};

fn header_value(request: &HttpRequest, name: &str) -> Option<String> {
    request
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

async fn request_context(
    pool: &PgPool,
    request: &HttpRequest,
) -> Result<UserContext, HttpResponse> {
    auth_context_from_token(pool, header_value(request, "x-kk-session").as_deref())
        .await
        .map_err(|err| {
            eprintln!("Failed to resolve auth context: {}", err);
            HttpResponse::InternalServerError().finish()
        })
}

async fn require_permission(
    pool: &PgPool,
    request: &HttpRequest,
    permission: &str,
) -> Result<UserContext, HttpResponse> {
    let context = request_context(pool, request).await?;
    if context.permissions.iter().any(|value| value == permission) {
        Ok(context)
    } else {
        Err(HttpResponse::Forbidden().json(ApiError {
            message: format!(
                "Role '{}' does not have '{}' permission.",
                context.role, permission
            ),
        }))
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#039;")
}

fn normalize_entity_type(value: &str) -> Option<&'static str> {
    match value {
        "organization" | "organizations" => Some("organization"),
        "project" | "projects" => Some("project"),
        "site" | "site_profile" | "site_profiles" => Some("site_profile"),
        "campaign" | "survey_campaign" | "survey_campaigns" => Some("survey_campaign"),
        "asset" | "infrastructure_asset" | "infrastructure_assets" => Some("infrastructure_asset"),
        "report" | "field_report" | "field_reports" => Some("field_report"),
        "alert" | "alerts" => Some("alert"),
        "ticket" | "maintenance_ticket" | "maintenance_tickets" => Some("maintenance_ticket"),
        "decision" | "decision_snapshot" | "decision_snapshots" => Some("decision_snapshot"),
        "execution" | "execution_plan" | "execution_plans" => Some("execution_plan"),
        "imei" | "operator_imei_event" | "operator_imei_events" => Some("operator_imei_event"),
        "template" | "workspace_template" | "workspace_templates" => Some("workspace_template"),
        _ => None,
    }
}

fn archive_table_for_entity(entity_type: &str) -> Option<&'static str> {
    match entity_type {
        "project" => Some("projects"),
        "site_profile" => Some("site_profiles"),
        "survey_campaign" => Some("survey_campaigns"),
        "infrastructure_asset" => Some("infrastructure_assets"),
        "field_report" => Some("field_reports"),
        "alert" => Some("alerts"),
        "maintenance_ticket" => Some("maintenance_tickets"),
        "decision_snapshot" => Some("decision_snapshots"),
        "execution_plan" => Some("execution_plans"),
        "operator_imei_event" => Some("operator_imei_events"),
        "workspace_template" => Some("workspace_templates"),
        _ => None,
    }
}

fn safe_file_name(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    cleaned
        .trim_matches('_')
        .chars()
        .take(120)
        .collect::<String>()
}

async fn record_exists(
    pool: &PgPool,
    entity_type: &str,
    entity_id: i64,
) -> Result<bool, sqlx::Error> {
    let table = match entity_type {
        "organization" => "organizations",
        "project" => "projects",
        "site_profile" => "site_profiles",
        "survey_campaign" => "survey_campaigns",
        "infrastructure_asset" => "infrastructure_assets",
        "field_report" => "field_reports",
        "alert" => "alerts",
        "maintenance_ticket" => "maintenance_tickets",
        "decision_snapshot" => "decision_snapshots",
        "execution_plan" => "execution_plans",
        "operator_imei_event" => "operator_imei_events",
        "workspace_template" => "workspace_templates",
        _ => return Ok(false),
    };
    if entity_type == "workspace_template" {
        return Ok(false);
    }
    let query = format!(
        "SELECT EXISTS (SELECT 1 FROM {} WHERE id = $1 AND archived_at IS NULL)",
        table
    );
    sqlx::query_as::<_, (bool,)>(&query)
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .map(|row| row.0)
}

async fn record_template_version(
    pool: &PgPool,
    template_id: &str,
    change_type: &str,
    actor: &str,
    note: Option<&str>,
) -> Result<(), sqlx::Error> {
    let snapshot = sqlx::query_as::<_, (serde_json::Value,)>(
        "SELECT to_jsonb(t) FROM workspace_templates t WHERE id = $1",
    )
    .bind(template_id)
    .fetch_one(pool)
    .await?
    .0;
    let version_number = sqlx::query_as::<_, (i32,)>(
        "SELECT COALESCE(MAX(version_number), 0) + 1 FROM workspace_template_versions WHERE template_id = $1",
    )
    .bind(template_id)
    .fetch_one(pool)
    .await?
    .0;
    sqlx::query(
        r#"
        INSERT INTO workspace_template_versions (
            template_id, version_number, change_type, snapshot, actor, note
        ) VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(template_id)
    .bind(version_number)
    .bind(change_type)
    .bind(snapshot)
    .bind(actor)
    .bind(note)
    .execute(pool)
    .await?;
    Ok(())
}

fn clean_template_list(values: &[String], fallback: &[&str]) -> Vec<String> {
    let cleaned = values
        .iter()
        .map(|value| value.trim().to_lowercase().replace(' ', "_"))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if cleaned.is_empty() {
        fallback.iter().map(|value| (*value).into()).collect()
    } else {
        cleaned
    }
}

fn validate_template_id(id: &str) -> Option<String> {
    let normalized = id.trim().to_lowercase();
    let valid = (3..=80).contains(&normalized.len())
        && normalized
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    valid.then_some(normalized)
}

fn clean_template_text(value: &str) -> String {
    value.trim().chars().take(500).collect::<String>()
}

async fn area_from_request(
    pool: &PgPool,
    region: &str,
    department: &str,
    commune: &str,
) -> Result<Option<LocationStat>, sqlx::Error> {
    Ok(fetch_location_stats(pool).await?.into_iter().find(|area| {
        area.region == region && area.department == department && area.commune == commune
    }))
}

async fn default_project_for_area(
    pool: &PgPool,
    region: &str,
    requested_project_id: Option<i64>,
) -> Result<Option<i64>, sqlx::Error> {
    if requested_project_id.is_some() {
        return Ok(requested_project_id);
    }

    sqlx::query_as::<_, (i64,)>(
        r#"
        SELECT id
        FROM projects
        ORDER BY
            CASE WHEN region = $1 THEN 0 ELSE 1 END,
            created_at DESC
        LIMIT 1
        "#,
    )
    .bind(region)
    .fetch_optional(pool)
    .await
    .map(|row| row.map(|value| value.0))
}

async fn ensure_area_action(
    pool: &PgPool,
    action: &str,
    area: &LocationStat,
    project_id: Option<i64>,
) -> Result<Vec<String>, sqlx::Error> {
    let mut created = Vec::new();
    let run_all = action == "full";
    let project_id = default_project_for_area(pool, &area.region, project_id).await?;
    let site_name = format!("{} field site", area.commune);
    let probe_name = format!("{} signal probe", area.commune);
    let campaign_name = format!("{} phone access validation", area.commune);
    let decision_title = format!("{} validation decision", area.commune);
    let ownership_rate = area.phone_rate;
    let confidence_pct = (area.confidence * 100.0).round() as i64;
    let budget = 450_000
        + (((area.population as f64) * 5.5).round() as i64).min(1_900_000)
        + if area.confidence < 0.68 {
            380_000
        } else {
            180_000
        };
    let evidence_score = ((area.confidence * 55.0) + 25.0).clamp(0.0, 100.0);

    if run_all || action == "site" {
        sqlx::query(
            r#"
            INSERT INTO site_profiles (
                project_id, name, site_type, region, department, commune, latitude,
                longitude, beneficiary_estimate, trust_signal, access_notes
            ) VALUES ($1, $2, 'telecom_probe_site', $3, $4, $5, $6, $7, $8, 'gps_photo_verified', $9)
            ON CONFLICT (name, commune)
            DO UPDATE SET
                project_id = COALESCE(EXCLUDED.project_id, site_profiles.project_id),
                beneficiary_estimate = EXCLUDED.beneficiary_estimate,
                trust_signal = EXCLUDED.trust_signal,
                access_notes = EXCLUDED.access_notes
            "#,
        )
        .bind(project_id)
        .bind(&site_name)
        .bind(&area.region)
        .bind(&area.department)
        .bind(&area.commune)
        .bind(area.latitude)
        .bind(area.longitude)
        .bind(area.population)
        .bind(format!(
            "Auto-created operational site for {}. Validate local focal point and GPS/photo proof.",
            area.commune
        ))
        .execute(pool)
        .await?;
        created.push("site profile".into());
    }

    if run_all || action == "campaign" {
        sqlx::query(
            r#"
            INSERT INTO survey_campaigns (
                project_id, name, form_type, target_region, target_department, target_commune,
                status, language_mode, offline_enabled, starts_on, ends_on
            ) VALUES ($1, $2, $3, $4, $5, $6, 'draft', 'bilingual', TRUE, CURRENT_DATE, CURRENT_DATE + INTERVAL '21 days')
            ON CONFLICT (project_id, name)
            DO UPDATE SET
                target_region = EXCLUDED.target_region,
                target_department = EXCLUDED.target_department,
                target_commune = EXCLUDED.target_commune,
                offline_enabled = TRUE,
                ends_on = EXCLUDED.ends_on
            "#,
        )
        .bind(project_id)
        .bind(&campaign_name)
        .bind(if ownership_rate < 65.0 {
            "phone_ownership_baseline"
        } else {
            "gps_photo_survey"
        })
        .bind(&area.region)
        .bind(&area.department)
        .bind(&area.commune)
        .execute(pool)
        .await?;
        created.push("survey campaign".into());
    }

    if run_all || action == "probe" {
        let site_id = sqlx::query_as::<_, (i64,)>(
            "SELECT id FROM site_profiles WHERE name = $1 AND commune = $2",
        )
        .bind(&site_name)
        .bind(&area.commune)
        .fetch_optional(pool)
        .await?
        .map(|row| row.0);
        sqlx::query(
            r#"
            INSERT INTO infrastructure_assets (
                project_id, site_profile_id, asset_type, name, region, department, commune,
                latitude, longitude, status, operator, last_checked_at, notes
            ) VALUES ($1, $2, 'connectivity_probe', $3, $4, $5, $6, $7, $8, $9, 'Operator/API field team', NOW(), $10)
            ON CONFLICT (name, commune)
            DO UPDATE SET
                project_id = COALESCE(EXCLUDED.project_id, infrastructure_assets.project_id),
                site_profile_id = COALESCE(EXCLUDED.site_profile_id, infrastructure_assets.site_profile_id),
                status = EXCLUDED.status,
                last_checked_at = NOW(),
                notes = EXCLUDED.notes
            "#,
        )
        .bind(project_id)
        .bind(site_id)
        .bind(&probe_name)
        .bind(&area.region)
        .bind(&area.department)
        .bind(&area.commune)
        .bind(area.latitude)
        .bind(area.longitude)
        .bind(if area.confidence < 0.68 { "warning" } else { "online" })
        .bind(format!(
            "Auto-created probe from action workflow: {:.1}% phone ownership, {}% confidence.",
            ownership_rate, confidence_pct
        ))
        .execute(pool)
        .await?;
        created.push("signal probe".into());
    }

    let asset_id = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM infrastructure_assets WHERE name = $1 AND commune = $2",
    )
    .bind(&probe_name)
    .bind(&area.commune)
    .fetch_optional(pool)
    .await?
    .map(|row| row.0);

    if run_all || action == "report" {
        sqlx::query(
            r#"
            INSERT INTO field_reports (
                project_id, site_profile_id, campaign_id, asset_id, report_type,
                region, department, commune, latitude, longitude, status,
                evidence_quality, notes, submitted_by
            )
            SELECT $1, sp.id, sc.id, $2, 'auto_validation_task',
                   $3, $4, $5, $6, $7, 'needs_followup',
                   'system_generated', $8, 'Action workflow'
            FROM (SELECT 1) seed
            LEFT JOIN site_profiles sp ON sp.name = $9 AND sp.commune = $5
            LEFT JOIN survey_campaigns sc ON sc.name = $10 AND sc.target_commune = $5
            WHERE NOT EXISTS (
                SELECT 1 FROM field_reports
                WHERE report_type = 'auto_validation_task'
                  AND region = $3 AND department = $4 AND commune = $5
                  AND submitted_by = 'Action workflow'
            )
            "#,
        )
        .bind(project_id)
        .bind(asset_id)
        .bind(&area.region)
        .bind(&area.department)
        .bind(&area.commune)
        .bind(area.latitude)
        .bind(area.longitude)
        .bind(format!(
            "Validate {} with GPS/photo proof. Matrix shows {:.1}% phone ownership and {}% confidence.",
            area.commune, ownership_rate, confidence_pct
        ))
        .bind(&site_name)
        .bind(&campaign_name)
        .execute(pool)
        .await?;
        created.push("validation report task".into());
    }

    if run_all || action == "alert" {
        sqlx::query(
            r#"
            INSERT INTO alerts (project_id, site_profile_id, asset_id, severity, title, message, status)
            SELECT $1, sp.id, $2, $3, $4, $5, 'open'
            FROM (SELECT 1) seed
            LEFT JOIN site_profiles sp ON sp.name = $6 AND sp.commune = $7
            WHERE NOT EXISTS (
                SELECT 1 FROM alerts WHERE title = $4 AND status <> 'resolved'
            )
            "#,
        )
        .bind(project_id)
        .bind(asset_id)
        .bind(if area.confidence < 0.68 { "warning" } else { "watch" })
        .bind(format!("{} validation alert", area.commune))
        .bind(format!(
            "{} needs field proof before higher-budget action. Confidence is {}%.",
            area.commune, confidence_pct
        ))
        .bind(&site_name)
        .bind(&area.commune)
        .execute(pool)
        .await?;
        created.push("coverage alert".into());
    }

    if run_all || action == "ticket" {
        sqlx::query(
            r#"
            INSERT INTO maintenance_tickets (
                project_id, site_profile_id, asset_id, title, priority, status,
                assigned_to, due_date, sla_hours
            )
            SELECT $1, sp.id, $2, $3, $4, 'open',
                   'Field operations team', CURRENT_DATE + INTERVAL '7 days', $5
            FROM (SELECT 1) seed
            LEFT JOIN site_profiles sp ON sp.name = $6 AND sp.commune = $7
            WHERE NOT EXISTS (
                SELECT 1 FROM maintenance_tickets
                WHERE title = $3 AND status NOT IN ('done', 'completed', 'cancelled')
            )
            "#,
        )
        .bind(project_id)
        .bind(asset_id)
        .bind(format!("{} field follow-up", area.commune))
        .bind(if area.confidence < 0.68 {
            "high"
        } else {
            "medium"
        })
        .bind(if area.confidence < 0.68 { 120 } else { 240 })
        .bind(&site_name)
        .bind(&area.commune)
        .execute(pool)
        .await?;
        created.push("maintenance ticket".into());
    }

    if run_all || action == "decision" {
        sqlx::query(
            r#"
            INSERT INTO decision_snapshots (
                project_id, site_profile_id, asset_id, title, decision_stage,
                priority_score, recommended_budget_xaf, owner_name, risk_level,
                evidence_score, execution_status, rationale, next_action
            )
            SELECT $1, sp.id, $2, $3, 'recommended',
                   $4, $5, 'Field operations lead', $6,
                   $7, 'not_started', $8, $9
            FROM (SELECT 1) seed
            LEFT JOIN site_profiles sp ON sp.name = $10 AND sp.commune = $11
            WHERE NOT EXISTS (
                SELECT 1 FROM decision_snapshots
                WHERE title = $3 AND decision_stage <> 'completed'
            )
            "#,
        )
        .bind(project_id)
        .bind(asset_id)
        .bind(&decision_title)
        .bind((100.0 - ownership_rate).max(0.0) + ((1.0 - area.confidence) * 30.0))
        .bind(budget)
        .bind(if area.confidence < 0.68 { "high" } else { "medium" })
        .bind(evidence_score)
        .bind(format!(
            "{} has {} people, {:.1}% estimated phone ownership, and {}% model confidence.",
            area.commune, area.population, ownership_rate, confidence_pct
        ))
        .bind("Complete field validation, review operator/telemetry evidence, then approve execution.")
        .bind(&site_name)
        .bind(&area.commune)
        .execute(pool)
        .await?;
        created.push("decision snapshot".into());
    }

    Ok(created)
}

fn render_ops_status_html(app_summary: &Summary, health: &WorkspaceHealth) -> String {
    format!(
        r#"<div class="alert alert-success py-2 mb-0">{} arrondissements, {} assets, {} open alerts, {} active tickets.</div>"#,
        app_summary.commune_count,
        health.monitored_assets,
        health.open_alerts,
        health.active_tickets
    )
}

fn render_activity_html(activity: &[WorkspaceActivity]) -> String {
    if activity.is_empty() {
        return r#"<div class="empty-state">No workspace activity yet.</div>"#.into();
    }

    activity
        .iter()
        .map(|item| {
            format!(
                r#"<article class="compact-card"><div><strong>{}</strong><span>{} &middot; {} &middot; {}</span></div><span class="status-pill">Live</span><p>{}</p></article>"#,
                escape_html(&item.action),
                escape_html(&item.related_entity),
                escape_html(&item.source),
                escape_html(&item.timestamp),
                escape_html(&item.description)
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

#[get("/api/summary")]
pub(crate) async fn summary(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_summary(pool.get_ref()).await {
        Ok(summary) => HttpResponse::Ok().json(summary),
        Err(err) => {
            eprintln!("Failed to query summary: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/overview")]
pub(crate) async fn overview(pool: web::Data<PgPool>) -> impl Responder {
    match build_overview_intelligence(pool.get_ref()).await {
        Ok(overview) => HttpResponse::Ok().json(overview),
        Err(err) => {
            eprintln!("Failed to build overview intelligence: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/auth/context")]
pub(crate) async fn auth_context(request: HttpRequest, pool: web::Data<PgPool>) -> impl Responder {
    match request_context(pool.get_ref(), &request).await {
        Ok(context) => HttpResponse::Ok().json(context),
        Err(response) => response,
    }
}

#[post("/api/auth/login")]
pub(crate) async fn login(
    pool: web::Data<PgPool>,
    payload: web::Json<LoginRequest>,
) -> impl Responder {
    match login_user(pool.get_ref(), &payload).await {
        Ok(Some(response)) => HttpResponse::Ok().json(response),
        Ok(None) => HttpResponse::Unauthorized().json(ApiError {
            message: "Invalid username, email, or password.".into(),
        }),
        Err(err) => {
            eprintln!("Failed to log in: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/audit-events")]
pub(crate) async fn list_audit_events(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    query: web::Query<AuditEventQuery>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "audit:read").await {
        return response;
    }
    match fetch_audit_events(pool.get_ref(), &query).await {
        Ok(events) => HttpResponse::Ok().json(events),
        Err(err) => {
            eprintln!("Failed to query audit events: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/entities/{entity_type}/{id}")]
pub(crate) async fn entity_detail(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<(String, i64)>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "audit:read").await {
        return response;
    }
    let (raw_entity_type, entity_id) = path.into_inner();
    let Some(entity_type) = normalize_entity_type(&raw_entity_type) else {
        return HttpResponse::BadRequest().json(ApiError {
            message: "Unsupported detail entity type.".into(),
        });
    };

    let record = match entity_type {
        "organization" => fetch_organizations(pool.get_ref())
            .await
            .ok()
            .and_then(|rows| rows.into_iter().find(|row| row.id == entity_id))
            .and_then(|record| serde_json::to_value(record).ok()),
        "project" => fetch_projects(pool.get_ref())
            .await
            .ok()
            .and_then(|rows| rows.into_iter().find(|row| row.id == entity_id))
            .and_then(|record| serde_json::to_value(record).ok()),
        "site_profile" => fetch_site_profiles(pool.get_ref())
            .await
            .ok()
            .and_then(|rows| rows.into_iter().find(|row| row.id == entity_id))
            .and_then(|record| serde_json::to_value(record).ok()),
        "survey_campaign" => fetch_survey_campaigns(pool.get_ref())
            .await
            .ok()
            .and_then(|rows| rows.into_iter().find(|row| row.id == entity_id))
            .and_then(|record| serde_json::to_value(record).ok()),
        "infrastructure_asset" => fetch_assets(pool.get_ref())
            .await
            .ok()
            .and_then(|rows| rows.into_iter().find(|row| row.id == entity_id))
            .and_then(|record| serde_json::to_value(record).ok()),
        "field_report" => fetch_reports(pool.get_ref())
            .await
            .ok()
            .and_then(|rows| rows.into_iter().find(|row| row.id == entity_id))
            .and_then(|record| serde_json::to_value(record).ok()),
        "alert" => fetch_alerts(pool.get_ref())
            .await
            .ok()
            .and_then(|rows| rows.into_iter().find(|row| row.id == entity_id))
            .and_then(|record| serde_json::to_value(record).ok()),
        "maintenance_ticket" => fetch_tickets(pool.get_ref())
            .await
            .ok()
            .and_then(|rows| rows.into_iter().find(|row| row.id == entity_id))
            .and_then(|record| serde_json::to_value(record).ok()),
        "decision_snapshot" => fetch_decision_snapshots(pool.get_ref())
            .await
            .ok()
            .and_then(|rows| rows.into_iter().find(|row| row.id == entity_id))
            .and_then(|record| serde_json::to_value(record).ok()),
        "execution_plan" => fetch_execution_plans(pool.get_ref())
            .await
            .ok()
            .and_then(|rows| rows.into_iter().find(|row| row.id == entity_id))
            .and_then(|record| serde_json::to_value(record).ok()),
        "operator_imei_event" => fetch_imei_events(pool.get_ref())
            .await
            .ok()
            .and_then(|rows| rows.into_iter().find(|row| row.id == entity_id))
            .and_then(|record| serde_json::to_value(record).ok()),
        _ => None,
    };
    let Some(record) = record else {
        return HttpResponse::NotFound().json(ApiError {
            message: "Record not found.".into(),
        });
    };

    let evidence = match fetch_evidence_files(pool.get_ref(), entity_type, entity_id).await {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to fetch evidence files: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };
    let audit_events = match fetch_audit_events(
        pool.get_ref(),
        &AuditEventQuery {
            entity_type: Some(entity_type.into()),
            entity_id: Some(entity_id),
            limit: Some(50),
        },
    )
    .await
    {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to fetch entity audit events: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(EntityDetail {
        entity_type: entity_type.into(),
        entity_id,
        record,
        evidence,
        audit_events,
    })
}

#[get("/api/evidence")]
pub(crate) async fn list_evidence(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    query: web::Query<EvidenceQuery>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "audit:read").await {
        return response;
    }
    let Some(entity_type) = normalize_entity_type(&query.entity_type) else {
        return HttpResponse::BadRequest().json(ApiError {
            message: "Unsupported evidence entity type.".into(),
        });
    };
    match fetch_evidence_files(pool.get_ref(), entity_type, query.entity_id).await {
        Ok(files) => HttpResponse::Ok().json(files),
        Err(err) => {
            eprintln!("Failed to query evidence files: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/evidence")]
pub(crate) async fn upload_evidence(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    payload: web::Json<EvidenceUploadRequest>,
) -> impl Responder {
    let context = match require_permission(pool.get_ref(), &request, "field:submit").await {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Some(entity_type) = normalize_entity_type(&payload.entity_type) else {
        return HttpResponse::BadRequest().json(ApiError {
            message: "Unsupported evidence entity type.".into(),
        });
    };
    match record_exists(pool.get_ref(), entity_type, payload.entity_id).await {
        Ok(true) => {}
        Ok(false) => {
            return HttpResponse::NotFound().json(ApiError {
                message: "Cannot attach evidence to a missing record.".into(),
            })
        }
        Err(err) => {
            eprintln!("Failed to verify evidence entity: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    }
    let bytes =
        match base64::engine::general_purpose::STANDARD.decode(payload.content_base64.trim()) {
            Ok(bytes) if !bytes.is_empty() => bytes,
            Ok(_) => {
                return HttpResponse::BadRequest().json(ApiError {
                    message: "Evidence file content is empty.".into(),
                })
            }
            Err(_) => {
                return HttpResponse::BadRequest().json(ApiError {
                    message: "Evidence file must be valid base64.".into(),
                })
            }
        };
    if bytes.len() > 8 * 1024 * 1024 {
        return HttpResponse::BadRequest().json(ApiError {
            message: "Evidence file cannot exceed 8 MB in this upload path.".into(),
        });
    }
    if let (Some(latitude), Some(longitude)) = (payload.latitude, payload.longitude) {
        if let Err(message) = validate_gps(latitude, longitude) {
            return HttpResponse::BadRequest().json(ApiError { message });
        }
    }

    let digest = Sha256::digest(&bytes);
    let hash = hex::encode(digest);
    let safe_name = safe_file_name(&payload.file_name);
    if safe_name.is_empty() {
        return HttpResponse::BadRequest().json(ApiError {
            message: "Evidence file name is required.".into(),
        });
    }
    let storage_dir: PathBuf = ["evidence_uploads", entity_type].iter().collect();
    if let Err(err) = std::fs::create_dir_all(&storage_dir) {
        eprintln!("Failed to create evidence storage dir: {}", err);
        return HttpResponse::InternalServerError().finish();
    }
    let storage_path = storage_dir.join(format!(
        "{}-{}-{}",
        payload.entity_id,
        &hash[..16],
        safe_name
    ));
    if let Err(err) = std::fs::write(&storage_path, &bytes) {
        eprintln!("Failed to write evidence file: {}", err);
        return HttpResponse::InternalServerError().finish();
    }
    let storage_path_string = Path::new(&storage_path).to_string_lossy().to_string();

    let insert = sqlx::query_as::<_, (i64,)>(
        r#"
        INSERT INTO evidence_files (
            entity_type, entity_id, file_name, content_type, storage_path,
            sha256_hash, file_size, latitude, longitude, captured_at, uploaded_by
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10::TIMESTAMPTZ, $11)
        RETURNING id
        "#,
    )
    .bind(entity_type)
    .bind(payload.entity_id)
    .bind(&payload.file_name)
    .bind(&payload.content_type)
    .bind(&storage_path_string)
    .bind(&hash)
    .bind(bytes.len() as i64)
    .bind(payload.latitude)
    .bind(payload.longitude)
    .bind(&payload.captured_at)
    .bind(&context.actor)
    .fetch_one(pool.get_ref())
    .await;

    let evidence_id = match insert {
        Ok(row) => row.0,
        Err(err) => {
            eprintln!("Failed to store evidence metadata: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };
    if let Err(err) = record_custom_audit_event(
        pool.get_ref(),
        entity_type,
        payload.entity_id,
        "evidence",
        None,
        Some(&payload.file_name),
        &context.actor,
        Some("Evidence uploaded."),
    )
    .await
    {
        eprintln!("Failed to audit evidence upload: {}", err);
        return HttpResponse::InternalServerError().finish();
    }

    match fetch_evidence_files(pool.get_ref(), entity_type, payload.entity_id).await {
        Ok(files) => HttpResponse::Ok().json(json!({ "evidence_id": evidence_id, "files": files })),
        Err(err) => {
            eprintln!("Failed to return evidence files after upload: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[patch("/api/entities/{entity_type}/{id}/archive")]
pub(crate) async fn archive_entity(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<(String, String)>,
    payload: web::Json<ArchiveRequest>,
) -> impl Responder {
    let context = match require_permission(pool.get_ref(), &request, "workspace:manage").await {
        Ok(context) => context,
        Err(response) => return response,
    };
    let (raw_entity_type, raw_id) = path.into_inner();
    let Some(entity_type) = normalize_entity_type(&raw_entity_type) else {
        return HttpResponse::BadRequest().json(ApiError {
            message: "Unsupported archive entity type.".into(),
        });
    };
    let Some(table) = archive_table_for_entity(entity_type) else {
        return HttpResponse::BadRequest().json(ApiError {
            message: "This record type cannot be archived through governance.".into(),
        });
    };
    let reason = payload
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Archived through governance workflow.");

    let rows_affected = if entity_type == "workspace_template" {
        let query = format!(
            r#"
            UPDATE {}
            SET archived_at = COALESCE(archived_at, NOW()),
                archived_by = COALESCE(archived_by, $2),
                archive_reason = COALESCE(archive_reason, $3),
                active = FALSE,
                updated_at = NOW()
            WHERE id = $1 AND archived_at IS NULL
            "#,
            table
        );
        match sqlx::query(&query)
            .bind(&raw_id)
            .bind(&context.actor)
            .bind(reason)
            .execute(pool.get_ref())
            .await
        {
            Ok(result) => result.rows_affected(),
            Err(err) => {
                eprintln!("Failed to archive template: {}", err);
                return HttpResponse::InternalServerError().finish();
            }
        }
    } else {
        let Ok(entity_id) = raw_id.parse::<i64>() else {
            return HttpResponse::BadRequest().json(ApiError {
                message: "Archive entity id must be numeric.".into(),
            });
        };
        let query = format!(
            r#"
            UPDATE {}
            SET archived_at = COALESCE(archived_at, NOW()),
                archived_by = COALESCE(archived_by, $2),
                archive_reason = COALESCE(archive_reason, $3)
            WHERE id = $1 AND archived_at IS NULL
            "#,
            table
        );
        match sqlx::query(&query)
            .bind(entity_id)
            .bind(&context.actor)
            .bind(reason)
            .execute(pool.get_ref())
            .await
        {
            Ok(result) => {
                if result.rows_affected() > 0 {
                    if let Err(err) = record_custom_audit_event(
                        pool.get_ref(),
                        entity_type,
                        entity_id,
                        "archived_at",
                        None,
                        Some("archived"),
                        &context.actor,
                        Some(reason),
                    )
                    .await
                    {
                        eprintln!("Failed to audit archive action: {}", err);
                        return HttpResponse::InternalServerError().finish();
                    }
                }
                result.rows_affected()
            }
            Err(err) => {
                eprintln!("Failed to archive entity: {}", err);
                return HttpResponse::InternalServerError().finish();
            }
        }
    };

    if rows_affected == 0 {
        return HttpResponse::NotFound().json(ApiError {
            message: "Record not found or already archived.".into(),
        });
    }

    if entity_type == "workspace_template" {
        if let Err(err) = record_template_version(
            pool.get_ref(),
            &raw_id,
            "archived",
            &context.actor,
            Some(reason),
        )
        .await
        {
            eprintln!("Failed to version template archive action: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
        if let Err(err) = record_custom_audit_event(
            pool.get_ref(),
            entity_type,
            0,
            "archived_at",
            None,
            Some(&raw_id),
            &context.actor,
            Some(reason),
        )
        .await
        {
            eprintln!("Failed to audit template archive action: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    }

    HttpResponse::Ok().json(ArchiveResponse {
        entity_type: entity_type.into(),
        entity_id: raw_id,
        archived: true,
        message: "Record archived and audit event written.".into(),
    })
}

#[get("/api/stats")]
pub(crate) async fn list_stats(pool: web::Data<PgPool>) -> impl Responder {
    let stats = fetch_location_stats(pool.get_ref()).await;

    match stats {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(err) => {
            eprintln!("Failed to query stats: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/phone-matrix")]
pub(crate) async fn phone_matrix(pool: web::Data<PgPool>) -> impl Responder {
    match build_phone_matrix(pool.get_ref()).await {
        Ok(matrix) => HttpResponse::Ok().json(matrix),
        Err(err) => {
            eprintln!("Failed to build phone matrix: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/phone-matrix/detail")]
pub(crate) async fn phone_matrix_detail(
    pool: web::Data<PgPool>,
    query: web::Query<PhoneMatrixDetailQuery>,
) -> impl Responder {
    match build_phone_matrix_detail(pool.get_ref(), &query).await {
        Ok(Some(detail)) => HttpResponse::Ok().json(detail),
        Ok(None) => HttpResponse::NotFound().json(ApiError {
            message: "Phone Matrix area not found.".into(),
        }),
        Err(err) => {
            eprintln!("Failed to build phone matrix detail: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/phone-matrix/assumptions")]
pub(crate) async fn phone_matrix_assumption_list() -> impl Responder {
    HttpResponse::Ok().json(phone_matrix_assumptions())
}

#[post("/api/phone-matrix/recalculate")]
pub(crate) async fn phone_matrix_recalculate(
    pool: web::Data<PgPool>,
    payload: web::Json<PhoneMatrixRecalculateRequest>,
) -> impl Responder {
    match recalculate_phone_matrix(pool.get_ref(), &payload).await {
        Ok(logs) => HttpResponse::Ok().json(logs),
        Err(err) => {
            eprintln!("Failed to recalculate phone matrix: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/stats/update")]
pub(crate) async fn update_stats(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    payload: web::Json<UpdateLocationRequest>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "data:write").await {
        return response;
    }
    if let Err(message) = payload.validate() {
        return HttpResponse::BadRequest().json(ApiError { message });
    }

    let result = sqlx::query(
        r#"
        INSERT INTO mobile_phone_stats (
            pcode,
            region,
            department,
            commune,
            location,
            latitude,
            longitude,
            area_sqkm,
            phone_owners,
            population,
            data_source
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'Manual entry')
        ON CONFLICT (region, department, commune)
        DO UPDATE SET
            location = EXCLUDED.location,
            latitude = EXCLUDED.latitude,
            longitude = EXCLUDED.longitude,
            area_sqkm = EXCLUDED.area_sqkm,
            phone_owners = EXCLUDED.phone_owners,
            population = EXCLUDED.population,
            data_source = EXCLUDED.data_source,
            updated_at = NOW()
        "#,
    )
    .bind(&payload.pcode)
    .bind(&payload.region)
    .bind(&payload.department)
    .bind(&payload.commune)
    .bind(&payload.location)
    .bind(payload.latitude)
    .bind(payload.longitude)
    .bind(payload.area_sqkm)
    .bind(payload.phone_owners)
    .bind(payload.population)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => {
            let stats = fetch_location_stats(pool.get_ref()).await;

            match stats {
                Ok(list) => HttpResponse::Ok().json(list),
                Err(err) => {
                    eprintln!("Failed to return updated stats: {}", err);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        Err(err) => {
            eprintln!("Failed to update stats: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/workspaces/dashboard")]
pub(crate) async fn workspace_dashboard(pool: web::Data<PgPool>) -> impl Responder {
    match build_workspace_dashboard(pool.get_ref()).await {
        Ok(dashboard) => HttpResponse::Ok().json(dashboard),
        Err(err) => {
            eprintln!("Failed to build workspace dashboard: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/actions/area")]
pub(crate) async fn run_area_action(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    payload: web::Json<AreaActionRequest>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "data:write").await {
        return response;
    }
    let action = payload.action.trim();
    if !matches!(
        action,
        "site" | "campaign" | "probe" | "report" | "alert" | "ticket" | "decision" | "full"
    ) {
        return HttpResponse::BadRequest().json(ApiError {
            message: "Unsupported area action.".into(),
        });
    }

    let area = match area_from_request(
        pool.get_ref(),
        &payload.region,
        &payload.department,
        &payload.commune,
    )
    .await
    {
        Ok(Some(area)) => area,
        Ok(None) => {
            return HttpResponse::NotFound().json(ApiError {
                message: "Area not found in phone matrix.".into(),
            })
        }
        Err(err) => {
            eprintln!("Failed to load area for action: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };

    match ensure_area_action(pool.get_ref(), action, &area, payload.project_id).await {
        Ok(created) => match build_workspace_dashboard(pool.get_ref()).await {
            Ok(dashboard) => HttpResponse::Ok().json(ActionResult {
                message: format!(
                    "{} action completed for {}.",
                    action.replace('_', " "),
                    area.commune
                ),
                created,
                dashboard,
            }),
            Err(err) => {
                eprintln!("Failed to build dashboard after action: {}", err);
                HttpResponse::InternalServerError().finish()
            }
        },
        Err(err) => {
            eprintln!("Failed to run area action: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/workspace-templates")]
pub(crate) async fn list_workspace_templates(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_workspace_templates(pool.get_ref()).await {
        Ok(templates) => HttpResponse::Ok().json(templates),
        Err(err) => {
            eprintln!("Failed to query workspace templates: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/workspace-templates")]
pub(crate) async fn create_workspace_template(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    payload: web::Json<WorkspaceTemplateRequest>,
) -> impl Responder {
    let context = match require_permission(pool.get_ref(), &request, "workspace:manage").await {
        Ok(context) => context,
        Err(response) => return response,
    };
    let Some(template_id) = validate_template_id(&payload.id) else {
        return HttpResponse::BadRequest().json(ApiError {
            message:
                "Template id must be 3-80 lowercase letters, numbers, hyphens, or underscores."
                    .into(),
        });
    };
    if payload.title.trim().is_empty()
        || payload.description.trim().is_empty()
        || payload.org_type.trim().is_empty()
        || payload.sector.trim().is_empty()
    {
        return HttpResponse::BadRequest().json(ApiError {
            message: "Template title, description, org type, and sector are required.".into(),
        });
    }

    let default_actions =
        clean_template_list(&payload.default_actions, &["site", "campaign", "decision"]);
    let required_evidence = clean_template_list(
        &payload.required_evidence,
        &["gps_photo", "local_focal_point"],
    );
    let result = sqlx::query(
        r#"
        INSERT INTO workspace_templates (
            id, title, description, org_type, sector, site_type, form_type,
            trust_signal, default_project_status, language_mode, offline_enabled,
            channel_strategy, target_segment, default_actions, required_evidence,
            creates_asset, creates_report_task, creates_alert, creates_ticket,
            active, sort_order, updated_at, archived_at, archived_by, archive_reason
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
            $16, $17, $18, $19, $20, $21, NOW(), NULL, NULL, NULL
        )
        "#,
    )
    .bind(&template_id)
    .bind(clean_template_text(&payload.title))
    .bind(clean_template_text(&payload.description))
    .bind(clean_template_text(&payload.org_type))
    .bind(clean_template_text(&payload.sector))
    .bind(clean_template_text(&payload.site_type))
    .bind(clean_template_text(&payload.form_type))
    .bind(clean_template_text(&payload.trust_signal))
    .bind(
        payload
            .default_project_status
            .as_deref()
            .map(clean_template_text)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "planning".into()),
    )
    .bind(
        payload
            .language_mode
            .as_deref()
            .map(clean_template_text)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "bilingual".into()),
    )
    .bind(payload.offline_enabled.unwrap_or(true))
    .bind(
        payload
            .channel_strategy
            .as_deref()
            .map(clean_template_text)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "field_team_whatsapp_sms".into()),
    )
    .bind(
        payload
            .target_segment
            .as_deref()
            .map(clean_template_text)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "council_ngo_operator".into()),
    )
    .bind(default_actions)
    .bind(required_evidence)
    .bind(payload.creates_asset.unwrap_or(false))
    .bind(payload.creates_report_task.unwrap_or(false))
    .bind(payload.creates_alert.unwrap_or(false))
    .bind(payload.creates_ticket.unwrap_or(false))
    .bind(payload.active.unwrap_or(true))
    .bind(payload.sort_order.unwrap_or(100))
    .execute(pool.get_ref())
    .await;

    if let Err(err) = result {
        eprintln!("Failed to create workspace template: {}", err);
        return HttpResponse::InternalServerError().finish();
    }
    if let Err(err) = record_template_version(
        pool.get_ref(),
        &template_id,
        "created",
        &context.actor,
        payload.note.as_deref(),
    )
    .await
    {
        eprintln!("Failed to version workspace template create: {}", err);
        return HttpResponse::InternalServerError().finish();
    }
    if let Err(err) = record_custom_audit_event(
        pool.get_ref(),
        "workspace_template",
        0,
        "template_management",
        None,
        Some(&format!("created:{template_id}")),
        &context.actor,
        payload.note.as_deref(),
    )
    .await
    {
        eprintln!("Failed to audit workspace template create: {}", err);
    }
    match fetch_workspace_templates(pool.get_ref()).await {
        Ok(templates) => HttpResponse::Ok().json(templates),
        Err(err) => {
            eprintln!("Failed to query workspace templates after create: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[patch("/api/workspace-templates/{id}")]
pub(crate) async fn update_workspace_template(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<String>,
    payload: web::Json<WorkspaceTemplateRequest>,
) -> impl Responder {
    let context = match require_permission(pool.get_ref(), &request, "workspace:manage").await {
        Ok(context) => context,
        Err(response) => return response,
    };
    let template_id = path.into_inner();
    if fetch_workspace_templates(pool.get_ref())
        .await
        .ok()
        .and_then(|rows| rows.into_iter().find(|template| template.id == template_id))
        .is_none()
    {
        return HttpResponse::NotFound().json(ApiError {
            message: "Workspace template not found.".into(),
        });
    }
    let default_actions =
        clean_template_list(&payload.default_actions, &["site", "campaign", "decision"]);
    let required_evidence = clean_template_list(
        &payload.required_evidence,
        &["gps_photo", "local_focal_point"],
    );
    let result = sqlx::query(
        r#"
        UPDATE workspace_templates
        SET title = $2,
            description = $3,
            org_type = $4,
            sector = $5,
            site_type = $6,
            form_type = $7,
            trust_signal = $8,
            default_project_status = $9,
            language_mode = $10,
            offline_enabled = $11,
            channel_strategy = $12,
            target_segment = $13,
            default_actions = $14,
            required_evidence = $15,
            creates_asset = $16,
            creates_report_task = $17,
            creates_alert = $18,
            creates_ticket = $19,
            active = $20,
            sort_order = $21,
            updated_at = NOW()
        WHERE id = $1 AND archived_at IS NULL
        "#,
    )
    .bind(&template_id)
    .bind(clean_template_text(&payload.title))
    .bind(clean_template_text(&payload.description))
    .bind(clean_template_text(&payload.org_type))
    .bind(clean_template_text(&payload.sector))
    .bind(clean_template_text(&payload.site_type))
    .bind(clean_template_text(&payload.form_type))
    .bind(clean_template_text(&payload.trust_signal))
    .bind(
        payload
            .default_project_status
            .as_deref()
            .map(clean_template_text)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "planning".into()),
    )
    .bind(
        payload
            .language_mode
            .as_deref()
            .map(clean_template_text)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "bilingual".into()),
    )
    .bind(payload.offline_enabled.unwrap_or(true))
    .bind(
        payload
            .channel_strategy
            .as_deref()
            .map(clean_template_text)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "field_team_whatsapp_sms".into()),
    )
    .bind(
        payload
            .target_segment
            .as_deref()
            .map(clean_template_text)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "council_ngo_operator".into()),
    )
    .bind(default_actions)
    .bind(required_evidence)
    .bind(payload.creates_asset.unwrap_or(false))
    .bind(payload.creates_report_task.unwrap_or(false))
    .bind(payload.creates_alert.unwrap_or(false))
    .bind(payload.creates_ticket.unwrap_or(false))
    .bind(payload.active.unwrap_or(true))
    .bind(payload.sort_order.unwrap_or(100))
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(done) if done.rows_affected() > 0 => {}
        Ok(_) => {
            return HttpResponse::NotFound().json(ApiError {
                message: "Workspace template not found.".into(),
            })
        }
        Err(err) => {
            eprintln!("Failed to update workspace template: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    }
    if let Err(err) = record_template_version(
        pool.get_ref(),
        &template_id,
        "updated",
        &context.actor,
        payload.note.as_deref(),
    )
    .await
    {
        eprintln!("Failed to version workspace template update: {}", err);
        return HttpResponse::InternalServerError().finish();
    }
    if let Err(err) = record_custom_audit_event(
        pool.get_ref(),
        "workspace_template",
        0,
        "template_management",
        None,
        Some(&format!("updated:{template_id}")),
        &context.actor,
        payload.note.as_deref(),
    )
    .await
    {
        eprintln!("Failed to audit workspace template update: {}", err);
    }
    match fetch_workspace_templates(pool.get_ref()).await {
        Ok(templates) => HttpResponse::Ok().json(templates),
        Err(err) => {
            eprintln!("Failed to query workspace templates after update: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[patch("/api/workspace-templates/{id}/status")]
pub(crate) async fn update_workspace_template_status(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<String>,
    payload: web::Json<WorkspaceTemplateStatusRequest>,
) -> impl Responder {
    let context = match require_permission(pool.get_ref(), &request, "workspace:manage").await {
        Ok(context) => context,
        Err(response) => return response,
    };
    let template_id = path.into_inner();
    let result = sqlx::query(
        "UPDATE workspace_templates SET active = $2, updated_at = NOW() WHERE id = $1 AND archived_at IS NULL",
    )
    .bind(&template_id)
    .bind(payload.active)
    .execute(pool.get_ref())
    .await;
    match result {
        Ok(done) if done.rows_affected() > 0 => {}
        Ok(_) => {
            return HttpResponse::NotFound().json(ApiError {
                message: "Workspace template not found.".into(),
            })
        }
        Err(err) => {
            eprintln!("Failed to update workspace template status: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    }
    let change_type = if payload.active {
        "activated"
    } else {
        "deactivated"
    };
    if let Err(err) = record_template_version(
        pool.get_ref(),
        &template_id,
        change_type,
        &context.actor,
        payload.note.as_deref(),
    )
    .await
    {
        eprintln!("Failed to version workspace template status: {}", err);
        return HttpResponse::InternalServerError().finish();
    }
    if let Err(err) = record_custom_audit_event(
        pool.get_ref(),
        "workspace_template",
        0,
        "template_management",
        None,
        Some(&format!("{change_type}:{template_id}")),
        &context.actor,
        payload.note.as_deref(),
    )
    .await
    {
        eprintln!("Failed to audit workspace template status: {}", err);
    }
    match fetch_workspace_templates(pool.get_ref()).await {
        Ok(templates) => HttpResponse::Ok().json(templates),
        Err(err) => {
            eprintln!("Failed to query workspace templates after status: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[patch("/api/workspace-templates/{id}/reorder")]
pub(crate) async fn reorder_workspace_template(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<String>,
    payload: web::Json<WorkspaceTemplateReorderRequest>,
) -> impl Responder {
    let context = match require_permission(pool.get_ref(), &request, "workspace:manage").await {
        Ok(context) => context,
        Err(response) => return response,
    };
    let template_id = path.into_inner();
    let result = sqlx::query(
        "UPDATE workspace_templates SET sort_order = $2, updated_at = NOW() WHERE id = $1 AND archived_at IS NULL",
    )
    .bind(&template_id)
    .bind(payload.sort_order)
    .execute(pool.get_ref())
    .await;
    match result {
        Ok(done) if done.rows_affected() > 0 => {}
        Ok(_) => {
            return HttpResponse::NotFound().json(ApiError {
                message: "Workspace template not found.".into(),
            })
        }
        Err(err) => {
            eprintln!("Failed to reorder workspace template: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    }
    if let Err(err) = record_template_version(
        pool.get_ref(),
        &template_id,
        "reordered",
        &context.actor,
        payload.note.as_deref(),
    )
    .await
    {
        eprintln!("Failed to version workspace template reorder: {}", err);
        return HttpResponse::InternalServerError().finish();
    }
    if let Err(err) = record_custom_audit_event(
        pool.get_ref(),
        "workspace_template",
        0,
        "template_management",
        None,
        Some(&format!("reordered:{template_id}")),
        &context.actor,
        payload.note.as_deref(),
    )
    .await
    {
        eprintln!("Failed to audit workspace template reorder: {}", err);
    }
    match fetch_workspace_templates(pool.get_ref()).await {
        Ok(templates) => HttpResponse::Ok().json(templates),
        Err(err) => {
            eprintln!("Failed to query workspace templates after reorder: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/workspace-templates/{id}/versions")]
pub(crate) async fn list_workspace_template_versions(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<String>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "audit:read").await {
        return response;
    }
    match fetch_workspace_template_versions(pool.get_ref(), &path.into_inner()).await {
        Ok(versions) => HttpResponse::Ok().json(versions),
        Err(err) => {
            eprintln!("Failed to query workspace template versions: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/workspace-templates/apply")]
pub(crate) async fn apply_workspace_template(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    payload: web::Json<WorkspaceTemplateApplyRequest>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "workspace:manage").await {
        return response;
    }

    let template = match fetch_workspace_template(pool.get_ref(), &payload.template_id).await {
        Ok(Some(template)) => template,
        Ok(None) => {
            return HttpResponse::BadRequest().json(ApiError {
                message: "Unknown workspace template.".into(),
            })
        }
        Err(err) => {
            eprintln!("Failed to load workspace template: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let area = if let (Some(region), Some(department), Some(commune)) = (
        payload.region.as_deref(),
        payload.department.as_deref(),
        payload.commune.as_deref(),
    ) {
        match area_from_request(pool.get_ref(), region, department, commune).await {
            Ok(value) => value,
            Err(err) => {
                eprintln!("Failed to load template area: {}", err);
                return HttpResponse::InternalServerError().finish();
            }
        }
    } else {
        match fetch_location_stats(pool.get_ref()).await {
            Ok(mut areas) => {
                areas.sort_by(|a, b| b.population.cmp(&a.population));
                areas.into_iter().next()
            }
            Err(err) => {
                eprintln!("Failed to load fallback template area: {}", err);
                return HttpResponse::InternalServerError().finish();
            }
        }
    };

    let Some(area) = area else {
        return HttpResponse::NotFound().json(ApiError {
            message: "No matrix area is available for the template.".into(),
        });
    };

    let org_name = format!("{} client", template.title);
    let project_name = format!("{} - {}", template.title, area.commune);
    let org_id = match sqlx::query_as::<_, (i64,)>(
        r#"
        INSERT INTO organizations (name, org_type, contact_name, contact_email)
        VALUES ($1, $2, 'Field operations lead', NULL)
        ON CONFLICT (name)
        DO UPDATE SET org_type = EXCLUDED.org_type
        RETURNING id
        "#,
    )
    .bind(&org_name)
    .bind(&template.org_type)
    .fetch_one(pool.get_ref())
    .await
    {
        Ok(row) => row.0,
        Err(err) => {
            eprintln!("Failed to apply template organization: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let project_id = match sqlx::query_as::<_, (i64,)>(
        r#"
        INSERT INTO projects (
            organization_id, name, sector, region, status, language_mode,
            channel_strategy, target_segment, start_date
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, CURRENT_DATE)
        ON CONFLICT (organization_id, name)
        DO UPDATE SET
            sector = EXCLUDED.sector,
            region = EXCLUDED.region,
            status = EXCLUDED.status,
            language_mode = EXCLUDED.language_mode,
            channel_strategy = EXCLUDED.channel_strategy,
            target_segment = EXCLUDED.target_segment
        RETURNING id
        "#,
    )
    .bind(org_id)
    .bind(&project_name)
    .bind(&template.sector)
    .bind(&area.region)
    .bind(&template.default_project_status)
    .bind(&template.language_mode)
    .bind(&template.channel_strategy)
    .bind(&template.target_segment)
    .fetch_one(pool.get_ref())
    .await
    {
        Ok(row) => row.0,
        Err(err) => {
            eprintln!("Failed to apply template project: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let mut created = vec![
        format!("organization: {}", org_name),
        format!("project: {}", project_name),
    ];
    let site_name = format!("{} {}", area.commune, template.site_type.replace('_', " "));
    if let Err(err) = sqlx::query(
        r#"
        INSERT INTO site_profiles (
            project_id, name, site_type, region, department, commune, latitude,
            longitude, beneficiary_estimate, trust_signal, access_notes
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        ON CONFLICT (name, commune)
        DO UPDATE SET
            project_id = EXCLUDED.project_id,
            site_type = EXCLUDED.site_type,
            trust_signal = EXCLUDED.trust_signal,
            access_notes = EXCLUDED.access_notes
        "#,
    )
    .bind(project_id)
    .bind(&site_name)
    .bind(&template.site_type)
    .bind(&area.region)
    .bind(&area.department)
    .bind(&area.commune)
    .bind(area.latitude)
    .bind(area.longitude)
    .bind(area.population)
    .bind(&template.trust_signal)
    .bind(format!(
        "{} template created for {}. Required evidence: {}.",
        template.title,
        area.commune,
        template.required_evidence.join(", ")
    ))
    .execute(pool.get_ref())
    .await
    {
        eprintln!("Failed to apply template site: {}", err);
        return HttpResponse::InternalServerError().finish();
    }
    created.push(format!("template site type: {}", template.site_type));

    if let Err(err) = sqlx::query(
        r#"
        INSERT INTO survey_campaigns (
            project_id, name, form_type, target_region, target_department,
            target_commune, status, language_mode, offline_enabled, starts_on, ends_on
        ) VALUES ($1, $2, $3, $4, $5, $6, 'draft', $7, $8, CURRENT_DATE, CURRENT_DATE + INTERVAL '21 days')
        ON CONFLICT (project_id, name)
        DO UPDATE SET
            form_type = EXCLUDED.form_type,
            language_mode = EXCLUDED.language_mode,
            offline_enabled = EXCLUDED.offline_enabled,
            ends_on = EXCLUDED.ends_on
        "#,
    )
    .bind(project_id)
    .bind(format!("{} {}", area.commune, template.form_type.replace('_', " ")))
    .bind(&template.form_type)
    .bind(&area.region)
    .bind(&area.department)
    .bind(&area.commune)
    .bind(&template.language_mode)
    .bind(template.offline_enabled)
    .execute(pool.get_ref())
    .await
    {
        eprintln!("Failed to apply template campaign: {}", err);
        return HttpResponse::InternalServerError().finish();
    }
    created.push(format!("template campaign: {}", template.form_type));

    for action in template.default_actions.iter() {
        if matches!(action.as_str(), "site" | "campaign") {
            continue;
        }
        match ensure_area_action(pool.get_ref(), action, &area, Some(project_id)).await {
            Ok(mut action_created) => created.append(&mut action_created),
            Err(err) => {
                eprintln!("Failed to apply template action '{}': {}", action, err);
                return HttpResponse::InternalServerError().finish();
            }
        }
    }
    created.push(format!(
        "required evidence: {}",
        template.required_evidence.join(", ")
    ));

    match build_workspace_dashboard(pool.get_ref()).await {
        Ok(dashboard) => HttpResponse::Ok().json(ActionResult {
            message: format!("Template '{}' applied to {}.", template.title, area.commune),
            created,
            dashboard,
        }),
        Err(err) => {
            eprintln!("Failed to build dashboard after template: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/fragments/ops-status")]
pub(crate) async fn ops_status_fragment(pool: web::Data<PgPool>) -> impl Responder {
    let app_summary = match fetch_summary(pool.get_ref()).await {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to build ops fragment summary: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };
    let health = match build_workspace_health(pool.get_ref()).await {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to build ops fragment health: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(render_ops_status_html(&app_summary, &health))
}

#[get("/fragments/workspace-activity")]
pub(crate) async fn workspace_activity_fragment(pool: web::Data<PgPool>) -> impl Responder {
    match build_workspace_dashboard(pool.get_ref()).await {
        Ok(dashboard) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(render_activity_html(&dashboard.activity)),
        Err(err) => {
            eprintln!("Failed to render workspace activity fragment: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/site-profiles")]
pub(crate) async fn list_site_profiles(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_site_profiles(pool.get_ref()).await {
        Ok(sites) => HttpResponse::Ok().json(sites),
        Err(err) => {
            eprintln!("Failed to query site profiles: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/site-profiles")]
pub(crate) async fn create_site_profile(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    payload: web::Json<SiteProfileRequest>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "data:write").await {
        return response;
    }
    for (value, label) in [
        (&payload.name, "Site name"),
        (&payload.site_type, "Site type"),
        (&payload.region, "Region"),
        (&payload.department, "Department"),
        (&payload.commune, "Arrondissement"),
    ] {
        if let Err(message) = validate_required(value, label) {
            return HttpResponse::BadRequest().json(ApiError { message });
        }
    }
    if let Err(message) = validate_gps(payload.latitude, payload.longitude) {
        return HttpResponse::BadRequest().json(ApiError { message });
    }
    if matches!(payload.beneficiary_estimate, Some(value) if value < 0) {
        return HttpResponse::BadRequest().json(ApiError {
            message: "Beneficiary estimate cannot be negative.".into(),
        });
    }

    let trust_signal = payload
        .trust_signal
        .clone()
        .unwrap_or_else(|| "field_verified".into());
    let result = sqlx::query(
        r#"
        INSERT INTO site_profiles (
            project_id, name, site_type, region, department, commune, latitude,
            longitude, beneficiary_estimate, trust_signal, access_notes
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        ON CONFLICT (name, commune)
        DO UPDATE SET
            project_id = EXCLUDED.project_id,
            site_type = EXCLUDED.site_type,
            region = EXCLUDED.region,
            department = EXCLUDED.department,
            latitude = EXCLUDED.latitude,
            longitude = EXCLUDED.longitude,
            beneficiary_estimate = EXCLUDED.beneficiary_estimate,
            trust_signal = EXCLUDED.trust_signal,
            access_notes = EXCLUDED.access_notes
        "#,
    )
    .bind(payload.project_id)
    .bind(&payload.name)
    .bind(&payload.site_type)
    .bind(&payload.region)
    .bind(&payload.department)
    .bind(&payload.commune)
    .bind(payload.latitude)
    .bind(payload.longitude)
    .bind(payload.beneficiary_estimate)
    .bind(trust_signal)
    .bind(&payload.access_notes)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => match fetch_site_profiles(pool.get_ref()).await {
            Ok(sites) => HttpResponse::Ok().json(sites),
            Err(err) => {
                eprintln!("Failed to return site profiles: {}", err);
                HttpResponse::InternalServerError().finish()
            }
        },
        Err(err) => {
            eprintln!("Failed to create site profile: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[patch("/api/site-profiles/{id}")]
pub(crate) async fn update_site_profile(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
    payload: web::Json<SiteProfileRequest>,
) -> impl Responder {
    let context = match require_permission(pool.get_ref(), &request, "data:write").await {
        Ok(context) => context,
        Err(response) => return response,
    };
    for (value, label) in [
        (&payload.name, "Site name"),
        (&payload.site_type, "Site type"),
        (&payload.region, "Region"),
        (&payload.department, "Department"),
        (&payload.commune, "Arrondissement"),
    ] {
        if let Err(message) = validate_required(value, label) {
            return HttpResponse::BadRequest().json(ApiError { message });
        }
    }
    if let Err(message) = validate_gps(payload.latitude, payload.longitude) {
        return HttpResponse::BadRequest().json(ApiError { message });
    }
    let id = *path;
    let result = sqlx::query(
        r#"
        UPDATE site_profiles
        SET project_id = $1,
            name = $2,
            site_type = $3,
            region = $4,
            department = $5,
            commune = $6,
            latitude = $7,
            longitude = $8,
            beneficiary_estimate = $9,
            trust_signal = COALESCE($10, trust_signal),
            access_notes = $11
        WHERE id = $12
        "#,
    )
    .bind(payload.project_id)
    .bind(&payload.name)
    .bind(&payload.site_type)
    .bind(&payload.region)
    .bind(&payload.department)
    .bind(&payload.commune)
    .bind(payload.latitude)
    .bind(payload.longitude)
    .bind(payload.beneficiary_estimate)
    .bind(&payload.trust_signal)
    .bind(&payload.access_notes)
    .bind(id)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(done) if done.rows_affected() == 0 => HttpResponse::NotFound().json(ApiError {
            message: "Site profile not found.".into(),
        }),
        Ok(_) => {
            if let Err(err) = record_custom_audit_event(
                pool.get_ref(),
                "site_profile",
                id,
                "metadata",
                None,
                Some("updated"),
                &context.actor,
                Some("Site profile edited."),
            )
            .await
            {
                eprintln!("Failed to audit site edit: {}", err);
                return HttpResponse::InternalServerError().finish();
            }
            match fetch_site_profiles(pool.get_ref()).await {
                Ok(sites) => HttpResponse::Ok().json(sites),
                Err(err) => {
                    eprintln!("Failed to return site profiles after update: {}", err);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        Err(err) => {
            eprintln!("Failed to update site profile: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/survey-campaigns")]
pub(crate) async fn list_survey_campaigns(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_survey_campaigns(pool.get_ref()).await {
        Ok(campaigns) => HttpResponse::Ok().json(campaigns),
        Err(err) => {
            eprintln!("Failed to query survey campaigns: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/survey-campaigns")]
pub(crate) async fn create_survey_campaign(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    payload: web::Json<SurveyCampaignRequest>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "data:write").await {
        return response;
    }
    for (value, label) in [
        (&payload.name, "Campaign name"),
        (&payload.form_type, "Form type"),
    ] {
        if let Err(message) = validate_required(value, label) {
            return HttpResponse::BadRequest().json(ApiError { message });
        }
    }
    let status = payload.status.clone().unwrap_or_else(|| "draft".into());
    let language_mode = payload
        .language_mode
        .clone()
        .unwrap_or_else(|| "bilingual".into());
    let offline_enabled = payload.offline_enabled.unwrap_or(true);

    let result = sqlx::query(
        r#"
        INSERT INTO survey_campaigns (
            project_id, name, form_type, target_region, target_department,
            target_commune, status, language_mode, offline_enabled, starts_on, ends_on
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10::DATE, $11::DATE)
        ON CONFLICT (project_id, name)
        DO UPDATE SET
            form_type = EXCLUDED.form_type,
            target_region = EXCLUDED.target_region,
            target_department = EXCLUDED.target_department,
            target_commune = EXCLUDED.target_commune,
            status = EXCLUDED.status,
            language_mode = EXCLUDED.language_mode,
            offline_enabled = EXCLUDED.offline_enabled,
            starts_on = EXCLUDED.starts_on,
            ends_on = EXCLUDED.ends_on
        "#,
    )
    .bind(payload.project_id)
    .bind(&payload.name)
    .bind(&payload.form_type)
    .bind(&payload.target_region)
    .bind(&payload.target_department)
    .bind(&payload.target_commune)
    .bind(status)
    .bind(language_mode)
    .bind(offline_enabled)
    .bind(&payload.starts_on)
    .bind(&payload.ends_on)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => match fetch_survey_campaigns(pool.get_ref()).await {
            Ok(campaigns) => HttpResponse::Ok().json(campaigns),
            Err(err) => {
                eprintln!("Failed to return survey campaigns: {}", err);
                HttpResponse::InternalServerError().finish()
            }
        },
        Err(err) => {
            eprintln!("Failed to create survey campaign: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[patch("/api/survey-campaigns/{id}/status")]
pub(crate) async fn update_survey_campaign_status(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
    payload: web::Json<SurveyCampaignStatusRequest>,
) -> impl Responder {
    let context = match require_permission(pool.get_ref(), &request, "workflow:transition").await {
        Ok(context) => context,
        Err(response) => return response,
    };
    let id = *path;
    let status = payload.status.trim();
    let current = match fetch_workflow_value(pool.get_ref(), WorkflowKind::SurveyCampaign, id).await
    {
        Ok(Some(value)) => value,
        Ok(None) => {
            return HttpResponse::NotFound().json(ApiError {
                message: "Survey campaign not found.".into(),
            })
        }
        Err(err) => {
            eprintln!("Failed to load survey campaign status: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };
    if let Err(message) = validate_transition(WorkflowKind::SurveyCampaign, &current, status) {
        return HttpResponse::BadRequest().json(ApiError { message });
    }

    let result = sqlx::query("UPDATE survey_campaigns SET status = $1 WHERE id = $2")
        .bind(status)
        .bind(id)
        .execute(pool.get_ref())
        .await;

    match result {
        Ok(_) => {
            if let Err(err) = record_audit_event(
                pool.get_ref(),
                WorkflowKind::SurveyCampaign,
                id,
                &current,
                status,
                &context.actor,
                None,
            )
            .await
            {
                eprintln!("Failed to audit survey campaign status change: {}", err);
                return HttpResponse::InternalServerError().finish();
            }
            match fetch_survey_campaigns(pool.get_ref()).await {
                Ok(campaigns) => HttpResponse::Ok().json(campaigns),
                Err(err) => {
                    eprintln!("Failed to return survey campaigns: {}", err);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        Err(err) => {
            eprintln!("Failed to update survey campaign status: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/decision-snapshots")]
pub(crate) async fn list_decision_snapshots(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_decision_snapshots(pool.get_ref()).await {
        Ok(decisions) => HttpResponse::Ok().json(decisions),
        Err(err) => {
            eprintln!("Failed to query decision snapshots: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/decision-board")]
pub(crate) async fn decision_board(pool: web::Data<PgPool>) -> impl Responder {
    match build_decision_board(pool.get_ref()).await {
        Ok(board) => HttpResponse::Ok().json(board),
        Err(err) => {
            eprintln!("Failed to build decision board: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/decision-snapshots")]
pub(crate) async fn create_decision_snapshot(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    payload: web::Json<DecisionSnapshotRequest>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "data:write").await {
        return response;
    }
    if let Err(message) = validate_required(&payload.title, "Decision title") {
        return HttpResponse::BadRequest().json(ApiError { message });
    }
    let priority_score = payload.priority_score.unwrap_or(0.0).clamp(0.0, 100.0);
    let decision_stage = payload
        .decision_stage
        .clone()
        .unwrap_or_else(|| "draft".into());
    let evidence_score = payload.evidence_score.unwrap_or_else(|| {
        decision_evidence_score(
            priority_score,
            payload.project_id.is_some(),
            payload.site_profile_id.is_some(),
            payload.asset_id.is_some(),
            payload.recommended_budget_xaf.is_some(),
        )
    });
    let risk_level = payload.risk_level.clone().unwrap_or_else(|| {
        decision_risk_level(
            priority_score,
            evidence_score,
            payload.recommended_budget_xaf,
        )
    });
    let execution_status = payload
        .execution_status
        .clone()
        .unwrap_or_else(|| "not_started".into());
    let rationale = payload.rationale.clone().unwrap_or_else(|| {
        "Decision created from KK Evo workspace data; enrich with field evidence before final approval.".into()
    });
    let next_action = payload.next_action.clone().unwrap_or_else(|| {
        "Review field evidence, confirm budget, then schedule execution.".into()
    });

    let result = sqlx::query(
        r#"
        INSERT INTO decision_snapshots (
            project_id, site_profile_id, asset_id, title, decision_stage, priority_score,
            recommended_budget_xaf, owner_name, risk_level, evidence_score,
            approval_notes, execution_status, rationale, next_action
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        "#,
    )
    .bind(payload.project_id)
    .bind(payload.site_profile_id)
    .bind(payload.asset_id)
    .bind(&payload.title)
    .bind(decision_stage)
    .bind(priority_score)
    .bind(payload.recommended_budget_xaf)
    .bind(&payload.owner_name)
    .bind(risk_level)
    .bind(evidence_score)
    .bind(&payload.approval_notes)
    .bind(execution_status)
    .bind(rationale)
    .bind(next_action)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => match fetch_decision_snapshots(pool.get_ref()).await {
            Ok(decisions) => HttpResponse::Ok().json(decisions),
            Err(err) => {
                eprintln!("Failed to return decision snapshots: {}", err);
                HttpResponse::InternalServerError().finish()
            }
        },
        Err(err) => {
            eprintln!("Failed to create decision snapshot: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[patch("/api/decision-snapshots/{id}/status")]
pub(crate) async fn update_decision_status(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
    payload: web::Json<DecisionStatusRequest>,
) -> impl Responder {
    let id = *path;
    let stage = payload.decision_stage.trim();
    if !matches!(
        stage,
        "draft" | "recommended" | "approved" | "blocked" | "executing" | "completed"
    ) {
        return HttpResponse::BadRequest().json(ApiError {
            message: "Decision stage is not supported.".into(),
        });
    }
    let decisions = match fetch_decision_snapshots(pool.get_ref()).await {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to load decision status: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };
    let Some(decision) = decisions.into_iter().find(|item| item.id == id) else {
        return HttpResponse::NotFound().json(ApiError {
            message: "Decision not found.".into(),
        });
    };
    if let Err(message) =
        validate_transition(WorkflowKind::Decision, &decision.decision_stage, stage)
    {
        return HttpResponse::BadRequest().json(ApiError { message });
    }
    let context = if stage == "approved" {
        match require_permission(pool.get_ref(), &request, "decision:approve").await {
            Ok(context) => context,
            Err(response) => return response,
        }
    } else {
        match require_permission(pool.get_ref(), &request, "workflow:transition").await {
            Ok(context) => context,
            Err(response) => return response,
        }
    };
    if let Err(message) = validate_decision_approval(
        stage,
        decision.evidence_score,
        decision.recommended_budget_xaf,
        payload.approval_notes.as_deref(),
    ) {
        return HttpResponse::BadRequest().json(ApiError { message });
    }
    let execution_status = payload.execution_status.clone().unwrap_or_else(|| {
        match stage {
            "approved" => "ready",
            "executing" => "in_progress",
            "completed" => "completed",
            "blocked" => "blocked",
            _ => "not_started",
        }
        .into()
    });

    let result = sqlx::query(
        r#"
        UPDATE decision_snapshots
        SET decision_stage = $1,
            execution_status = $2,
            approval_notes = COALESCE($3, approval_notes)
        WHERE id = $4
        "#,
    )
    .bind(stage)
    .bind(execution_status)
    .bind(&payload.approval_notes)
    .bind(id)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => {
            if let Err(err) = record_audit_event(
                pool.get_ref(),
                WorkflowKind::Decision,
                id,
                &decision.decision_stage,
                stage,
                &context.actor,
                payload.approval_notes.as_deref(),
            )
            .await
            {
                eprintln!("Failed to audit decision status change: {}", err);
                return HttpResponse::InternalServerError().finish();
            }
            match build_decision_board(pool.get_ref()).await {
                Ok(board) => HttpResponse::Ok().json(board),
                Err(err) => {
                    eprintln!("Failed to return decision board: {}", err);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        Err(err) => {
            eprintln!("Failed to update decision status: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/execution-plans")]
pub(crate) async fn list_execution_plans(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_execution_plans(pool.get_ref()).await {
        Ok(plans) => HttpResponse::Ok().json(plans),
        Err(err) => {
            eprintln!("Failed to query execution plans: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/execution-board")]
pub(crate) async fn execution_board(pool: web::Data<PgPool>) -> impl Responder {
    match build_execution_board(pool.get_ref()).await {
        Ok(board) => HttpResponse::Ok().json(board),
        Err(err) => {
            eprintln!("Failed to build execution board: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/decision-snapshots/{id}/execution-plan")]
pub(crate) async fn create_execution_plan_from_decision(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "decision:approve").await {
        return response;
    }
    let decision_id = *path;
    let decisions = match fetch_decision_snapshots(pool.get_ref()).await {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to load decision for execution plan: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };
    let Some(decision) = decisions.into_iter().find(|item| item.id == decision_id) else {
        return HttpResponse::NotFound().json(ApiError {
            message: "Decision not found.".into(),
        });
    };
    if let Err(message) =
        validate_execution_plan_creation(&decision.decision_stage, decision.evidence_score)
    {
        return HttpResponse::BadRequest().json(ApiError { message });
    }

    let result = sqlx::query(
        r#"
        INSERT INTO execution_plans (
            decision_id, project_id, site_profile_id, asset_id, title, owner_name,
            status, budget_xaf, planned_start, planned_end, transport_access_notes,
            xaf_budget_approved, blocker
        ) VALUES (
            $1, $2, $3, $4, $5, $6,
            CASE WHEN $7 >= 60 THEN 'ready' ELSE 'planned' END,
            $8, CURRENT_DATE + INTERVAL '7 days', CURRENT_DATE + INTERVAL '21 days',
            $9, $10, $11
        )
        "#,
    )
    .bind(decision.id)
    .bind(decision.project_id)
    .bind(decision.site_profile_id)
    .bind(decision.asset_id)
    .bind(format!("Execute: {}", decision.title))
    .bind(&decision.owner_name)
    .bind(decision.evidence_score)
    .bind(decision.recommended_budget_xaf)
    .bind("Confirm field transport, local access, and partner availability before launch.")
    .bind(decision.decision_stage == "approved")
    .bind(if decision.evidence_score < 60.0 {
        Some("Evidence score below execution threshold.")
    } else {
        None
    })
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => {
            let _ = sqlx::query(
                "UPDATE decision_snapshots SET execution_status = 'ready' WHERE id = $1",
            )
            .bind(decision_id)
            .execute(pool.get_ref())
            .await;
            match build_execution_board(pool.get_ref()).await {
                Ok(board) => HttpResponse::Ok().json(board),
                Err(err) => {
                    eprintln!("Failed to return execution board: {}", err);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        Err(err) => {
            eprintln!("Failed to create execution plan: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[patch("/api/execution-plans/{id}/status")]
pub(crate) async fn update_execution_plan_status(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
    payload: web::Json<ExecutionPlanStatusRequest>,
) -> impl Responder {
    let id = *path;
    let status = payload.status.trim();
    if !matches!(
        status,
        "planned" | "ready" | "in_progress" | "blocked" | "completed"
    ) {
        return HttpResponse::BadRequest().json(ApiError {
            message: "Execution status is not supported.".into(),
        });
    }
    let current = match fetch_workflow_value(pool.get_ref(), WorkflowKind::ExecutionPlan, id).await
    {
        Ok(Some(value)) => value,
        Ok(None) => {
            return HttpResponse::NotFound().json(ApiError {
                message: "Execution plan not found.".into(),
            })
        }
        Err(err) => {
            eprintln!("Failed to load execution plan status: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };
    if let Err(message) = validate_transition(WorkflowKind::ExecutionPlan, &current, status) {
        return HttpResponse::BadRequest().json(ApiError { message });
    }
    if let Err(message) = validate_execution_completion(status, payload.outcome_notes.as_deref()) {
        return HttpResponse::BadRequest().json(ApiError { message });
    }
    let context = if status == "completed" {
        match require_permission(pool.get_ref(), &request, "execution:complete").await {
            Ok(context) => context,
            Err(response) => return response,
        }
    } else {
        match require_permission(pool.get_ref(), &request, "workflow:transition").await {
            Ok(context) => context,
            Err(response) => return response,
        }
    };

    let result = sqlx::query(
        r#"
        UPDATE execution_plans
        SET status = $1,
            local_focal_point_confirmed = COALESCE($2, local_focal_point_confirmed),
            gps_photo_proof_required = COALESCE($3, gps_photo_proof_required),
            offline_survey_ready = COALESCE($4, offline_survey_ready),
            bilingual_script_ready = COALESCE($5, bilingual_script_ready),
            xaf_budget_approved = COALESCE($6, xaf_budget_approved),
            blocker = COALESCE($7, blocker),
            outcome_notes = COALESCE($8, outcome_notes),
            updated_at = NOW()
        WHERE id = $9
        "#,
    )
    .bind(status)
    .bind(payload.local_focal_point_confirmed)
    .bind(payload.gps_photo_proof_required)
    .bind(payload.offline_survey_ready)
    .bind(payload.bilingual_script_ready)
    .bind(payload.xaf_budget_approved)
    .bind(&payload.blocker)
    .bind(&payload.outcome_notes)
    .bind(id)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => {
            if let Err(err) = record_audit_event(
                pool.get_ref(),
                WorkflowKind::ExecutionPlan,
                id,
                &current,
                status,
                &context.actor,
                payload
                    .outcome_notes
                    .as_deref()
                    .or(payload.blocker.as_deref()),
            )
            .await
            {
                eprintln!("Failed to audit execution plan status change: {}", err);
                return HttpResponse::InternalServerError().finish();
            }
            match build_execution_board(pool.get_ref()).await {
                Ok(board) => HttpResponse::Ok().json(board),
                Err(err) => {
                    eprintln!("Failed to return execution board: {}", err);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        Err(err) => {
            eprintln!("Failed to update execution plan: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/organizations")]
pub(crate) async fn list_organizations(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_organizations(pool.get_ref()).await {
        Ok(organizations) => HttpResponse::Ok().json(organizations),
        Err(err) => {
            eprintln!("Failed to query organizations: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/organizations")]
pub(crate) async fn create_organization(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    payload: web::Json<OrganizationRequest>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "workspace:manage").await {
        return response;
    }
    let result = sqlx::query(
        r#"
        INSERT INTO organizations (name, org_type, contact_name, contact_email)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (name)
        DO UPDATE SET
            org_type = EXCLUDED.org_type,
            contact_name = EXCLUDED.contact_name,
            contact_email = EXCLUDED.contact_email
        "#,
    )
    .bind(&payload.name)
    .bind(&payload.org_type)
    .bind(&payload.contact_name)
    .bind(&payload.contact_email)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => match fetch_organizations(pool.get_ref()).await {
            Ok(organizations) => HttpResponse::Ok().json(organizations),
            Err(err) => {
                eprintln!("Failed to return organizations: {}", err);
                HttpResponse::InternalServerError().finish()
            }
        },
        Err(err) => {
            eprintln!("Failed to create organization: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/projects")]
pub(crate) async fn list_projects(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_projects(pool.get_ref()).await {
        Ok(projects) => HttpResponse::Ok().json(projects),
        Err(err) => {
            eprintln!("Failed to query projects: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/projects")]
pub(crate) async fn create_project(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    payload: web::Json<ProjectRequest>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "workspace:manage").await {
        return response;
    }
    let result = sqlx::query(
        r#"
        INSERT INTO projects (organization_id, name, sector, region, status, start_date)
        VALUES ($1, $2, $3, $4, $5, $6::DATE)
        ON CONFLICT (organization_id, name)
        DO UPDATE SET
            sector = EXCLUDED.sector,
            region = EXCLUDED.region,
            status = EXCLUDED.status,
            start_date = EXCLUDED.start_date
        "#,
    )
    .bind(payload.organization_id)
    .bind(&payload.name)
    .bind(&payload.sector)
    .bind(&payload.region)
    .bind(&payload.status)
    .bind(&payload.start_date)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => match fetch_projects(pool.get_ref()).await {
            Ok(projects) => HttpResponse::Ok().json(projects),
            Err(err) => {
                eprintln!("Failed to return projects: {}", err);
                HttpResponse::InternalServerError().finish()
            }
        },
        Err(err) => {
            eprintln!("Failed to create project: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/assets")]
pub(crate) async fn list_assets(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_assets(pool.get_ref()).await {
        Ok(assets) => HttpResponse::Ok().json(assets),
        Err(err) => {
            eprintln!("Failed to query assets: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/signal-probes/dashboard")]
pub(crate) async fn signal_probe_dashboard(pool: web::Data<PgPool>) -> impl Responder {
    match build_signal_probe_dashboard(pool.get_ref()).await {
        Ok(dashboard) => HttpResponse::Ok().json(dashboard),
        Err(err) => {
            eprintln!("Failed to build signal probe dashboard: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/area-dossier")]
pub(crate) async fn area_dossier(
    pool: web::Data<PgPool>,
    query: web::Query<AreaDossierQuery>,
) -> impl Responder {
    match build_area_dossier(
        pool.get_ref(),
        &query.region,
        &query.department,
        &query.commune,
    )
    .await
    {
        Ok(Some(dossier)) => HttpResponse::Ok().json(dossier),
        Ok(None) => HttpResponse::NotFound().json(ApiError {
            message: "Area dossier not found.".into(),
        }),
        Err(err) => {
            eprintln!("Failed to build area dossier: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/assets")]
pub(crate) async fn create_asset(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    payload: web::Json<AssetRequest>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "data:write").await {
        return response;
    }
    if let Err(message) = validate_gps(payload.latitude, payload.longitude) {
        return HttpResponse::BadRequest().json(ApiError { message });
    }

    let result = sqlx::query(
        r#"
        INSERT INTO infrastructure_assets (
            project_id, site_profile_id, asset_type, name, region, department, commune,
            latitude, longitude, status, operator, installed_at, last_checked_at, notes
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::DATE, NOW(), $13)
        ON CONFLICT (name, commune)
        DO UPDATE SET
            project_id = COALESCE(EXCLUDED.project_id, infrastructure_assets.project_id),
            site_profile_id = COALESCE(EXCLUDED.site_profile_id, infrastructure_assets.site_profile_id),
            asset_type = EXCLUDED.asset_type,
            region = EXCLUDED.region,
            department = EXCLUDED.department,
            latitude = EXCLUDED.latitude,
            longitude = EXCLUDED.longitude,
            status = EXCLUDED.status,
            operator = EXCLUDED.operator,
            installed_at = EXCLUDED.installed_at,
            last_checked_at = NOW(),
            notes = EXCLUDED.notes
        "#,
    )
    .bind(payload.project_id)
    .bind(payload.site_profile_id)
    .bind(&payload.asset_type)
    .bind(&payload.name)
    .bind(&payload.region)
    .bind(&payload.department)
    .bind(&payload.commune)
    .bind(payload.latitude)
    .bind(payload.longitude)
    .bind(&payload.status)
    .bind(&payload.operator)
    .bind(&payload.installed_at)
    .bind(&payload.notes)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => match fetch_assets(pool.get_ref()).await {
            Ok(assets) => HttpResponse::Ok().json(assets),
            Err(err) => {
                eprintln!("Failed to return assets: {}", err);
                HttpResponse::InternalServerError().finish()
            }
        },
        Err(err) => {
            eprintln!("Failed to create asset: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[patch("/api/assets/{id}")]
pub(crate) async fn update_asset(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
    payload: web::Json<AssetRequest>,
) -> impl Responder {
    let context = match require_permission(pool.get_ref(), &request, "data:write").await {
        Ok(context) => context,
        Err(response) => return response,
    };
    if let Err(message) = validate_gps(payload.latitude, payload.longitude) {
        return HttpResponse::BadRequest().json(ApiError { message });
    }
    let id = *path;
    let result = sqlx::query(
        r#"
        UPDATE infrastructure_assets
        SET project_id = $1,
            site_profile_id = $2,
            asset_type = $3,
            name = $4,
            region = $5,
            department = $6,
            commune = $7,
            latitude = $8,
            longitude = $9,
            status = $10,
            operator = $11,
            installed_at = $12::DATE,
            notes = $13,
            last_checked_at = NOW()
        WHERE id = $14
        "#,
    )
    .bind(payload.project_id)
    .bind(payload.site_profile_id)
    .bind(&payload.asset_type)
    .bind(&payload.name)
    .bind(&payload.region)
    .bind(&payload.department)
    .bind(&payload.commune)
    .bind(payload.latitude)
    .bind(payload.longitude)
    .bind(&payload.status)
    .bind(&payload.operator)
    .bind(&payload.installed_at)
    .bind(&payload.notes)
    .bind(id)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(done) if done.rows_affected() == 0 => HttpResponse::NotFound().json(ApiError {
            message: "Asset not found.".into(),
        }),
        Ok(_) => {
            if let Err(err) = record_custom_audit_event(
                pool.get_ref(),
                "infrastructure_asset",
                id,
                "metadata",
                None,
                Some("updated"),
                &context.actor,
                Some("Asset edited."),
            )
            .await
            {
                eprintln!("Failed to audit asset edit: {}", err);
                return HttpResponse::InternalServerError().finish();
            }
            match fetch_assets(pool.get_ref()).await {
                Ok(assets) => HttpResponse::Ok().json(assets),
                Err(err) => {
                    eprintln!("Failed to return assets after update: {}", err);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        Err(err) => {
            eprintln!("Failed to update asset: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[patch("/api/assets/{id}/status")]
pub(crate) async fn update_asset_status(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
    payload: web::Json<AssetStatusRequest>,
) -> impl Responder {
    let context = match require_permission(pool.get_ref(), &request, "workflow:transition").await {
        Ok(context) => context,
        Err(response) => return response,
    };
    let id = *path;
    let status = payload.status.trim();
    if !matches!(status, "online" | "warning" | "critical" | "offline") {
        return HttpResponse::BadRequest().json(ApiError {
            message: "Asset status must be online, warning, critical, or offline.".into(),
        });
    }
    let current = match fetch_workflow_value(pool.get_ref(), WorkflowKind::Asset, id).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return HttpResponse::NotFound().json(ApiError {
                message: "Asset not found.".into(),
            })
        }
        Err(err) => {
            eprintln!("Failed to load asset status: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };
    if let Err(message) = validate_transition(WorkflowKind::Asset, &current, status) {
        return HttpResponse::BadRequest().json(ApiError { message });
    }

    let result = sqlx::query(
        r#"
        UPDATE infrastructure_assets
        SET status = $1,
            notes = COALESCE($2, notes),
            last_checked_at = NOW()
        WHERE id = $3
        "#,
    )
    .bind(status)
    .bind(&payload.notes)
    .bind(id)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => {
            if let Err(err) = record_audit_event(
                pool.get_ref(),
                WorkflowKind::Asset,
                id,
                &current,
                status,
                &context.actor,
                payload.notes.as_deref(),
            )
            .await
            {
                eprintln!("Failed to audit asset status change: {}", err);
                return HttpResponse::InternalServerError().finish();
            }
            match fetch_assets(pool.get_ref()).await {
                Ok(assets) => HttpResponse::Ok().json(assets),
                Err(err) => {
                    eprintln!("Failed to return assets after status update: {}", err);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        Err(err) => {
            eprintln!("Failed to update asset status: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/reports")]
pub(crate) async fn list_reports(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_reports(pool.get_ref()).await {
        Ok(reports) => HttpResponse::Ok().json(reports),
        Err(err) => {
            eprintln!("Failed to query reports: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/reports")]
pub(crate) async fn create_report(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    payload: web::Json<FieldReportRequest>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "field:submit").await {
        return response;
    }
    if let Err(message) = validate_gps(payload.latitude, payload.longitude) {
        return HttpResponse::BadRequest().json(ApiError { message });
    }

    let result = sqlx::query(
        r#"
        INSERT INTO field_reports (
            project_id, site_profile_id, campaign_id, asset_id, report_type,
            region, department, commune, latitude, longitude, status,
            evidence_quality, notes, submitted_by
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        "#,
    )
    .bind(payload.project_id)
    .bind(payload.site_profile_id)
    .bind(payload.campaign_id)
    .bind(payload.asset_id)
    .bind(&payload.report_type)
    .bind(&payload.region)
    .bind(&payload.department)
    .bind(&payload.commune)
    .bind(payload.latitude)
    .bind(payload.longitude)
    .bind(&payload.status)
    .bind(
        payload
            .evidence_quality
            .clone()
            .unwrap_or_else(|| "unverified".into()),
    )
    .bind(&payload.notes)
    .bind(&payload.submitted_by)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => match fetch_reports(pool.get_ref()).await {
            Ok(reports) => HttpResponse::Ok().json(reports),
            Err(err) => {
                eprintln!("Failed to return reports: {}", err);
                HttpResponse::InternalServerError().finish()
            }
        },
        Err(err) => {
            eprintln!("Failed to create report: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[patch("/api/reports/{id}")]
pub(crate) async fn update_report(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
    payload: web::Json<FieldReportRequest>,
) -> impl Responder {
    let context = match require_permission(pool.get_ref(), &request, "field:submit").await {
        Ok(context) => context,
        Err(response) => return response,
    };
    if let Err(message) = validate_gps(payload.latitude, payload.longitude) {
        return HttpResponse::BadRequest().json(ApiError { message });
    }
    let id = *path;
    let result = sqlx::query(
        r#"
        UPDATE field_reports
        SET project_id = $1,
            site_profile_id = $2,
            campaign_id = $3,
            asset_id = $4,
            report_type = $5,
            region = $6,
            department = $7,
            commune = $8,
            latitude = $9,
            longitude = $10,
            status = $11,
            evidence_quality = COALESCE($12, evidence_quality),
            notes = $13,
            submitted_by = $14
        WHERE id = $15
        "#,
    )
    .bind(payload.project_id)
    .bind(payload.site_profile_id)
    .bind(payload.campaign_id)
    .bind(payload.asset_id)
    .bind(&payload.report_type)
    .bind(&payload.region)
    .bind(&payload.department)
    .bind(&payload.commune)
    .bind(payload.latitude)
    .bind(payload.longitude)
    .bind(&payload.status)
    .bind(&payload.evidence_quality)
    .bind(&payload.notes)
    .bind(&payload.submitted_by)
    .bind(id)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(done) if done.rows_affected() == 0 => HttpResponse::NotFound().json(ApiError {
            message: "Report not found.".into(),
        }),
        Ok(_) => {
            if let Err(err) = record_custom_audit_event(
                pool.get_ref(),
                "field_report",
                id,
                "metadata",
                None,
                Some("updated"),
                &context.actor,
                Some("Field report edited."),
            )
            .await
            {
                eprintln!("Failed to audit report edit: {}", err);
                return HttpResponse::InternalServerError().finish();
            }
            match fetch_reports(pool.get_ref()).await {
                Ok(reports) => HttpResponse::Ok().json(reports),
                Err(err) => {
                    eprintln!("Failed to return reports after update: {}", err);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        Err(err) => {
            eprintln!("Failed to update report: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/alerts")]
pub(crate) async fn list_alerts(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_alerts(pool.get_ref()).await {
        Ok(alerts) => HttpResponse::Ok().json(alerts),
        Err(err) => {
            eprintln!("Failed to query alerts: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/alerts")]
pub(crate) async fn create_alert(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    payload: web::Json<AlertRequest>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "workflow:transition").await
    {
        return response;
    }
    let result = sqlx::query(
        r#"
        INSERT INTO alerts (project_id, site_profile_id, asset_id, severity, title, message, status)
        VALUES ($1, $2, $3, $4, $5, $6, 'open')
        "#,
    )
    .bind(payload.project_id)
    .bind(payload.site_profile_id)
    .bind(payload.asset_id)
    .bind(&payload.severity)
    .bind(&payload.title)
    .bind(&payload.message)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => match fetch_alerts(pool.get_ref()).await {
            Ok(alerts) => HttpResponse::Ok().json(alerts),
            Err(err) => {
                eprintln!("Failed to return alerts: {}", err);
                HttpResponse::InternalServerError().finish()
            }
        },
        Err(err) => {
            eprintln!("Failed to create alert: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[patch("/api/alerts/{id}")]
pub(crate) async fn update_alert_status(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
    payload: web::Json<AlertStatusRequest>,
) -> impl Responder {
    let context = match require_permission(pool.get_ref(), &request, "workflow:transition").await {
        Ok(context) => context,
        Err(response) => return response,
    };
    let id = *path;
    let status = payload.status.trim();
    let current = match fetch_workflow_value(pool.get_ref(), WorkflowKind::Alert, id).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return HttpResponse::NotFound().json(ApiError {
                message: "Alert not found.".into(),
            })
        }
        Err(err) => {
            eprintln!("Failed to load alert status: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };
    if let Err(message) = validate_transition(WorkflowKind::Alert, &current, status) {
        return HttpResponse::BadRequest().json(ApiError { message });
    }

    let result = sqlx::query(
        r#"
        UPDATE alerts
        SET status = $1,
            resolved_at = CASE WHEN $1 = 'resolved' THEN NOW() ELSE NULL END
        WHERE id = $2
        "#,
    )
    .bind(status)
    .bind(id)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => {
            if let Err(err) = record_audit_event(
                pool.get_ref(),
                WorkflowKind::Alert,
                id,
                &current,
                status,
                &context.actor,
                None,
            )
            .await
            {
                eprintln!("Failed to audit alert status change: {}", err);
                return HttpResponse::InternalServerError().finish();
            }
            match fetch_alerts(pool.get_ref()).await {
                Ok(alerts) => HttpResponse::Ok().json(alerts),
                Err(err) => {
                    eprintln!("Failed to return alerts: {}", err);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        Err(err) => {
            eprintln!("Failed to update alert: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/tickets")]
pub(crate) async fn list_tickets(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_tickets(pool.get_ref()).await {
        Ok(tickets) => HttpResponse::Ok().json(tickets),
        Err(err) => {
            eprintln!("Failed to query tickets: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/tickets")]
pub(crate) async fn create_ticket(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    payload: web::Json<MaintenanceTicketRequest>,
) -> impl Responder {
    let context = match require_permission(pool.get_ref(), &request, "workflow:transition").await {
        Ok(context) => context,
        Err(response) => return response,
    };
    let previous_alert_status = if let Some(alert_id) = payload.alert_id {
        match fetch_workflow_value(pool.get_ref(), WorkflowKind::Alert, alert_id).await {
            Ok(value) => value,
            Err(err) => {
                eprintln!("Failed to load linked alert for ticket creation: {}", err);
                return HttpResponse::InternalServerError().finish();
            }
        }
    } else {
        None
    };
    let result = sqlx::query(
        r#"
        INSERT INTO maintenance_tickets (
            project_id, site_profile_id, asset_id, alert_id, title, priority,
            status, assigned_to, due_date, sla_hours
        ) VALUES ($1, $2, $3, $4, $5, $6, 'open', $7, $8::DATE, $9)
        "#,
    )
    .bind(payload.project_id)
    .bind(payload.site_profile_id)
    .bind(payload.asset_id)
    .bind(payload.alert_id)
    .bind(&payload.title)
    .bind(&payload.priority)
    .bind(&payload.assigned_to)
    .bind(&payload.due_date)
    .bind(payload.sla_hours)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => {
            if let (Some(alert_id), Some(previous_status)) =
                (payload.alert_id, previous_alert_status.as_deref())
            {
                if matches!(previous_status, "open" | "acknowledged") {
                    if let Err(message) =
                        validate_transition(WorkflowKind::Alert, previous_status, "ticketed")
                    {
                        return HttpResponse::BadRequest().json(ApiError { message });
                    }
                    let alert_update =
                        sqlx::query("UPDATE alerts SET status = 'ticketed' WHERE id = $1")
                            .bind(alert_id)
                            .execute(pool.get_ref())
                            .await;
                    if let Err(err) = alert_update {
                        eprintln!("Failed to mark alert ticketed: {}", err);
                        return HttpResponse::InternalServerError().finish();
                    }
                    if let Err(err) = record_audit_event(
                        pool.get_ref(),
                        WorkflowKind::Alert,
                        alert_id,
                        previous_status,
                        "ticketed",
                        &context.actor,
                        Some("Ticket created from alert."),
                    )
                    .await
                    {
                        eprintln!("Failed to audit alert ticketing: {}", err);
                        return HttpResponse::InternalServerError().finish();
                    }
                }
            }
            match fetch_tickets(pool.get_ref()).await {
                Ok(tickets) => HttpResponse::Ok().json(tickets),
                Err(err) => {
                    eprintln!("Failed to return tickets: {}", err);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        Err(err) => {
            eprintln!("Failed to create ticket: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[patch("/api/tickets/{id}")]
pub(crate) async fn update_ticket_status(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
    payload: web::Json<MaintenanceTicketStatusRequest>,
) -> impl Responder {
    let context = match require_permission(pool.get_ref(), &request, "workflow:transition").await {
        Ok(context) => context,
        Err(response) => return response,
    };
    let id = *path;
    let status = payload.status.trim();
    let current =
        match fetch_workflow_value(pool.get_ref(), WorkflowKind::MaintenanceTicket, id).await {
            Ok(Some(value)) => value,
            Ok(None) => {
                return HttpResponse::NotFound().json(ApiError {
                    message: "Maintenance ticket not found.".into(),
                })
            }
            Err(err) => {
                eprintln!("Failed to load ticket status: {}", err);
                return HttpResponse::InternalServerError().finish();
            }
        };
    if let Err(message) = validate_transition(WorkflowKind::MaintenanceTicket, &current, status) {
        return HttpResponse::BadRequest().json(ApiError { message });
    }
    if let Err(message) = validate_ticket_completion(status, payload.resolution_notes.as_deref()) {
        return HttpResponse::BadRequest().json(ApiError { message });
    }
    let linked_alert = match sqlx::query_as::<_, (Option<i64>,)>(
        "SELECT alert_id FROM maintenance_tickets WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool.get_ref())
    .await
    {
        Ok(Some(row)) => row.0,
        Ok(None) => None,
        Err(err) => {
            eprintln!("Failed to load ticket alert link: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };
    let previous_alert_status = if let Some(alert_id) = linked_alert {
        match fetch_workflow_value(pool.get_ref(), WorkflowKind::Alert, alert_id).await {
            Ok(value) => value,
            Err(err) => {
                eprintln!("Failed to load linked alert status: {}", err);
                return HttpResponse::InternalServerError().finish();
            }
        }
    } else {
        None
    };

    let result = sqlx::query(
        r#"
        UPDATE maintenance_tickets
        SET status = $1,
            resolution_notes = COALESCE($2, resolution_notes),
            updated_at = NOW()
        WHERE id = $3
        "#,
    )
    .bind(status)
    .bind(&payload.resolution_notes)
    .bind(id)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => {
            if let Err(err) = record_audit_event(
                pool.get_ref(),
                WorkflowKind::MaintenanceTicket,
                id,
                &current,
                status,
                &context.actor,
                payload.resolution_notes.as_deref(),
            )
            .await
            {
                eprintln!("Failed to audit ticket status change: {}", err);
                return HttpResponse::InternalServerError().finish();
            }
            if matches!(status, "done" | "completed") {
                if let (Some(alert_id), Some(previous_status)) =
                    (linked_alert, previous_alert_status.as_deref())
                {
                    if previous_status != "resolved" {
                        if let Err(message) =
                            validate_transition(WorkflowKind::Alert, previous_status, "resolved")
                        {
                            return HttpResponse::BadRequest().json(ApiError { message });
                        }
                        let alert_update = sqlx::query(
                            r#"
                            UPDATE alerts
                            SET status = 'resolved',
                                resolved_at = NOW()
                            WHERE id = $1
                            "#,
                        )
                        .bind(alert_id)
                        .execute(pool.get_ref())
                        .await;
                        if let Err(err) = alert_update {
                            eprintln!("Failed to resolve linked alert: {}", err);
                            return HttpResponse::InternalServerError().finish();
                        }
                        if let Err(err) = record_audit_event(
                            pool.get_ref(),
                            WorkflowKind::Alert,
                            alert_id,
                            previous_status,
                            "resolved",
                            &context.actor,
                            Some("Linked ticket completed."),
                        )
                        .await
                        {
                            eprintln!("Failed to audit linked alert resolution: {}", err);
                            return HttpResponse::InternalServerError().finish();
                        }
                    }
                }
            }
            match fetch_tickets(pool.get_ref()).await {
                Ok(tickets) => HttpResponse::Ok().json(tickets),
                Err(err) => {
                    eprintln!("Failed to return tickets: {}", err);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        Err(err) => {
            eprintln!("Failed to update ticket: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/iot/readings")]
pub(crate) async fn list_iot_readings(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_iot_readings(pool.get_ref()).await {
        Ok(readings) => HttpResponse::Ok().json(readings),
        Err(err) => {
            eprintln!("Failed to query IoT readings: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/iot/readings")]
pub(crate) async fn create_iot_reading(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    payload: web::Json<IotReadingRequest>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "telemetry:write").await {
        return response;
    }
    let result = sqlx::query(
        r#"
        INSERT INTO iot_readings (
            project_id, site_profile_id, asset_id, reading_type, value, unit, latitude, longitude
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(payload.project_id)
    .bind(payload.site_profile_id)
    .bind(payload.asset_id)
    .bind(&payload.reading_type)
    .bind(payload.value)
    .bind(&payload.unit)
    .bind(payload.latitude)
    .bind(payload.longitude)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => match fetch_iot_readings(pool.get_ref()).await {
            Ok(readings) => HttpResponse::Ok().json(readings),
            Err(err) => {
                eprintln!("Failed to return IoT readings: {}", err);
                HttpResponse::InternalServerError().finish()
            }
        },
        Err(err) => {
            eprintln!("Failed to create IoT reading: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/operator-imei-events")]
pub(crate) async fn imei_compliance_summary(
    request: HttpRequest,
    pool: web::Data<PgPool>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "audit:read").await {
        return response;
    }
    match build_imei_compliance_summary(pool.get_ref()).await {
        Ok(compliance_summary) => HttpResponse::Ok().json(compliance_summary),
        Err(err) => {
            eprintln!("Failed to build IMEI compliance summary: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/operator-imei-events")]
pub(crate) async fn ingest_imei_event(
    request: HttpRequest,
    pool: web::Data<PgPool>,
    payload: web::Json<OperatorImeiEventRequest>,
) -> impl Responder {
    if let Err(response) = require_permission(pool.get_ref(), &request, "telemetry:write").await {
        return response;
    }
    if let Err(message) = validate_imei_event(&payload) {
        return HttpResponse::BadRequest().json(ApiError { message });
    }

    let imei_hash = payload
        .imei_hash
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| payload.imei.as_deref().map(imei_fingerprint))
        .unwrap_or_default();
    let last4 = payload.imei.as_deref().and_then(imei_last4);
    let source_system = payload
        .source_system
        .clone()
        .unwrap_or_else(|| "operator_api".into());

    let result = sqlx::query(
        r#"
        INSERT INTO operator_imei_events (
            operator_name, imei_hash, imei_last4, device_type, event_type,
            compliance_status, region, department, commune, source_system,
            raw_reference, network_first_seen_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::TIMESTAMPTZ)
        "#,
    )
    .bind(&payload.operator_name)
    .bind(imei_hash)
    .bind(last4)
    .bind(&payload.device_type)
    .bind(&payload.event_type)
    .bind(&payload.compliance_status)
    .bind(&payload.region)
    .bind(&payload.department)
    .bind(&payload.commune)
    .bind(source_system)
    .bind(&payload.raw_reference)
    .bind(&payload.network_first_seen_at)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => match build_imei_compliance_summary(pool.get_ref()).await {
            Ok(compliance_summary) => HttpResponse::Ok().json(compliance_summary),
            Err(err) => {
                eprintln!("Failed to return IMEI compliance summary: {}", err);
                HttpResponse::InternalServerError().finish()
            }
        },
        Err(err) => {
            eprintln!("Failed to ingest IMEI event: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/priority-zones")]
pub(crate) async fn priority_zones(pool: web::Data<PgPool>) -> impl Responder {
    match build_priority_zones(pool.get_ref()).await {
        Ok(zones) => HttpResponse::Ok().json(zones),
        Err(err) => {
            eprintln!("Failed to build priority zones: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/decision-report")]
pub(crate) async fn decision_report(pool: web::Data<PgPool>) -> impl Responder {
    let report_summary = match fetch_summary(pool.get_ref()).await {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to build report summary: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };
    let report_priority_zones = match build_priority_zones(pool.get_ref()).await {
        Ok(zones) => zones.into_iter().take(8).collect::<Vec<_>>(),
        Err(err) => {
            eprintln!("Failed to build report priority zones: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };
    let assets = fetch_assets(pool.get_ref()).await.unwrap_or_default();
    let alerts = fetch_alerts(pool.get_ref()).await.unwrap_or_default();
    let reports = fetch_reports(pool.get_ref()).await.unwrap_or_default();
    let tickets = fetch_tickets(pool.get_ref()).await.unwrap_or_default();
    let open_alerts = alerts
        .iter()
        .filter(|alert| alert.status != "resolved")
        .count() as i64;
    let active_tickets = tickets
        .iter()
        .filter(|ticket| ticket.status != "done" && ticket.status != "cancelled")
        .count() as i64;
    let overdue_tickets = sqlx::query_as::<_, (i64,)>(
        r#"
        SELECT COUNT(*)
        FROM maintenance_tickets
        WHERE status NOT IN ('done', 'cancelled')
          AND due_date < CURRENT_DATE
        "#,
    )
    .fetch_one(pool.get_ref())
    .await
    .map(|row| row.0)
    .unwrap_or(0);
    let workspace_health = match build_workspace_health(pool.get_ref()).await {
        Ok(value) => value,
        Err(err) => {
            eprintln!(
                "Failed to build workspace health for decision report: {}",
                err
            );
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(DecisionReport {
        generated_for: "KK Evo Cameroon intelligence platform".into(),
        summary: report_summary,
        open_alerts,
        monitored_assets: assets.len() as i64,
        field_reports: reports.len() as i64,
        active_tickets,
        overdue_tickets,
        top_priority_zones: report_priority_zones,
        recommendations: vec![
            "Start with monitored water and solar assets in high-priority arrondissements.".into(),
            "Use field reports to validate matrix estimates before hardware deployment.".into(),
            "Convert repeated alerts into maintenance tickets with technician assignments.".into(),
            "Package monthly council/NGO reports around uptime, response time, and beneficiary reach.".into(),
        ],
        market_realities: market_realities(),
        workspace_health,
    })
}

#[get("/api/export/assets.csv")]
pub(crate) async fn export_assets(pool: web::Data<PgPool>) -> impl Responder {
    let assets = match fetch_assets(pool.get_ref()).await {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to export assets: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };
    let mut csv = String::from("id,type,name,region,department,commune,latitude,longitude,status,operator,installed_at,last_checked_at,notes\n");
    for asset in assets {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            asset.id,
            csv_escape(&asset.asset_type),
            csv_escape(&asset.name),
            csv_escape(&asset.region),
            csv_escape(&asset.department),
            csv_escape(&asset.commune),
            asset.latitude,
            asset.longitude,
            csv_escape(&asset.status),
            csv_escape(asset.operator.as_deref().unwrap_or("")),
            csv_escape(asset.installed_at.as_deref().unwrap_or("")),
            csv_escape(asset.last_checked_at.as_deref().unwrap_or("")),
            csv_escape(asset.notes.as_deref().unwrap_or(""))
        ));
    }
    HttpResponse::Ok()
        .append_header(("Content-Type", "text/csv; charset=utf-8"))
        .append_header((
            "Content-Disposition",
            "attachment; filename=\"kk-evo-assets.csv\"",
        ))
        .body(csv)
}

#[get("/api/export/tickets.csv")]
pub(crate) async fn export_tickets(pool: web::Data<PgPool>) -> impl Responder {
    let tickets = match fetch_tickets(pool.get_ref()).await {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to export tickets: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };
    let mut csv = String::from("id,asset_id,alert_id,title,priority,status,assigned_to,due_date,resolution_notes,created_at,updated_at\n");
    for ticket in tickets {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            ticket.id,
            ticket
                .asset_id
                .map(|value| value.to_string())
                .unwrap_or_default(),
            ticket
                .alert_id
                .map(|value| value.to_string())
                .unwrap_or_default(),
            csv_escape(&ticket.title),
            csv_escape(&ticket.priority),
            csv_escape(&ticket.status),
            csv_escape(ticket.assigned_to.as_deref().unwrap_or("")),
            csv_escape(ticket.due_date.as_deref().unwrap_or("")),
            csv_escape(ticket.resolution_notes.as_deref().unwrap_or("")),
            csv_escape(&ticket.created_at),
            csv_escape(&ticket.updated_at)
        ));
    }
    HttpResponse::Ok()
        .append_header(("Content-Type", "text/csv; charset=utf-8"))
        .append_header((
            "Content-Disposition",
            "attachment; filename=\"kk-evo-tickets.csv\"",
        ))
        .body(csv)
}

#[get("/api/export/priority-zones.csv")]
pub(crate) async fn export_priority_zones(pool: web::Data<PgPool>) -> impl Responder {
    let zones = match build_priority_zones(pool.get_ref()).await {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to export priority zones: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };
    let mut csv = String::from("pcode,region,department,commune,latitude,longitude,population,phone_rate,confidence,asset_count,open_alert_count,report_count,priority_score,priority_label\n");
    for zone in zones {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_escape(zone.pcode.as_deref().unwrap_or("")),
            csv_escape(&zone.region),
            csv_escape(&zone.department),
            csv_escape(&zone.commune),
            zone.latitude,
            zone.longitude,
            zone.population,
            zone.phone_rate,
            zone.confidence,
            zone.asset_count,
            zone.open_alert_count,
            zone.report_count,
            zone.priority_score,
            csv_escape(&zone.priority_label)
        ));
    }
    HttpResponse::Ok()
        .append_header(("Content-Type", "text/csv; charset=utf-8"))
        .append_header((
            "Content-Disposition",
            "attachment; filename=\"kk-evo-priority-zones.csv\"",
        ))
        .body(csv)
}

#[get("/api/export/phone-matrix.csv")]
pub(crate) async fn export_phone_matrix(pool: web::Data<PgPool>) -> impl Responder {
    let matrix = match build_phone_matrix(pool.get_ref()).await {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to export phone matrix: {}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };
    let mut csv = String::from("region,department,arrondissement,pcode,population,estimated_phone_owners,estimated_mobile_subscriptions,ownership_rate,confidence_level,opportunity_score,priority_score,recommended_action,needs_validation,data_source,last_updated\n");
    for row in matrix.rows {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{:.2},{},{:.2},{:.2},{},{},{},{}\n",
            csv_escape(&row.region),
            csv_escape(&row.department),
            csv_escape(&row.commune),
            csv_escape(row.pcode.as_deref().unwrap_or("")),
            row.population,
            row.estimated_phone_owners,
            row.estimated_mobile_subscriptions,
            row.ownership_rate,
            csv_escape(&row.confidence_level),
            row.opportunity_score,
            row.priority_score,
            csv_escape(&row.recommended_action),
            row.needs_validation,
            csv_escape(&row.data_source),
            csv_escape(&row.last_updated)
        ));
    }
    HttpResponse::Ok()
        .append_header(("Content-Type", "text/csv; charset=utf-8"))
        .append_header((
            "Content-Disposition",
            "attachment; filename=\"kk-evo-phone-matrix.csv\"",
        ))
        .body(csv)
}
