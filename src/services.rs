use std::collections::{HashMap, HashSet};

use sqlx::PgPool;

use crate::models::*;

fn region_weight(region: &str) -> f64 {
    match region {
        "Centre" | "Littoral" => 1.24,
        "Ouest" | "Sud-Ouest" => 1.08,
        "Nord-Ouest" => 1.02,
        "Adamaoua" | "Nord" => 0.92,
        "Extrême-Nord" => 0.88,
        "Est" | "Sud" => 0.82,
        _ => 1.0,
    }
}

fn haversine_km(a_lat: f64, a_lon: f64, b_lat: f64, b_lon: f64) -> f64 {
    let radius_km = 6_371.0;
    let d_lat = (b_lat - a_lat).to_radians();
    let d_lon = (b_lon - a_lon).to_radians();
    let a = (d_lat / 2.0).sin().powi(2)
        + a_lat.to_radians().cos() * b_lat.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
    2.0 * radius_km * a.sqrt().asin()
}

fn urban_signal(row: &DbLocation) -> f64 {
    let signal = URBAN_ANCHORS
        .iter()
        .map(|anchor| {
            let distance = haversine_km(
                row.latitude,
                row.longitude,
                anchor.latitude,
                anchor.longitude,
            );
            anchor.influence / (1.0 + distance / 42.0).powi(2)
        })
        .sum::<f64>();

    signal.clamp(0.0, 1.0)
}

fn population_weight(row: &DbLocation, urban_signal: f64) -> f64 {
    let area = row.area_sqkm.unwrap_or(900.0).max(1.0);
    let density_signal = 1.0 / area.sqrt();
    let location_signal = 0.25 + (urban_signal * 2.9);
    density_signal * location_signal * region_weight(&row.region)
}

fn ownership_rate(urban_signal: f64, region: &str) -> f64 {
    let national_proxy = (CAMEROON_2024_MOBILE_SUBSCRIPTIONS_PER_100 / 100.0).min(0.96);
    let rural_floor = 0.56;
    let urban_ceiling = national_proxy;
    let region_adjustment = (region_weight(region) - 1.0) * 0.09;
    (rural_floor + ((urban_ceiling - rural_floor) * urban_signal) + region_adjustment)
        .clamp(0.48, 0.96)
}

fn confidence(row: &DbLocation, urban_signal: f64) -> f64 {
    let has_geometry = row.area_sqkm.is_some() && row.pcode.is_some();
    let base = if has_geometry { 0.58 } else { 0.35 };
    let signal_bonus = urban_signal * 0.22;
    (base + signal_bonus).clamp(0.35, 0.86)
}

fn apply_matrix(rows: Vec<DbLocation>) -> Vec<LocationStat> {
    let signals = rows
        .iter()
        .map(|row| {
            let urban = urban_signal(row);
            let weight = population_weight(row, urban);
            (urban, weight)
        })
        .collect::<Vec<_>>();

    let measured_population = rows.iter().filter_map(|row| row.population).sum::<i64>();
    let remaining_population = (CAMEROON_2025_POPULATION - measured_population).max(0);
    let unknown_weight_sum = rows
        .iter()
        .zip(signals.iter())
        .filter(|(row, _)| row.population.is_none())
        .map(|(_, (_, weight))| weight)
        .sum::<f64>()
        .max(1.0);
    let mut allocated_populations = vec![None; rows.len()];
    let mut fractional_allocations = Vec::new();
    let mut allocated_total = 0_i64;

    for (index, (row, (_, weight))) in rows.iter().zip(signals.iter()).enumerate() {
        if let Some(population) = row.population {
            allocated_populations[index] = Some(population);
            continue;
        }

        let raw = (weight / unknown_weight_sum) * remaining_population as f64;
        let floor = raw.floor() as i64;
        allocated_populations[index] = Some(floor);
        allocated_total += floor;
        fractional_allocations.push((index, raw - floor as f64));
    }

    fractional_allocations
        .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    for (index, _) in fractional_allocations
        .into_iter()
        .take((remaining_population - allocated_total).max(0) as usize)
    {
        if let Some(population) = allocated_populations[index].as_mut() {
            *population += 1;
        }
    }

    rows.into_iter()
        .zip(signals)
        .zip(allocated_populations)
        .map(|((row, (urban, _weight)), allocated_population)| {
            let population = allocated_population.unwrap_or(0);
            let rate = match (row.phone_owners, row.population) {
                (Some(phone_owners), Some(population)) if population > 0 => {
                    phone_owners as f64 / population as f64
                }
                _ => ownership_rate(urban, &row.region),
            };
            let phone_owners = row
                .phone_owners
                .unwrap_or_else(|| (population as f64 * rate).round() as i64);
            let metric_source = if row.phone_owners.is_some() && row.population.is_some() {
                "Measured local update".to_string()
            } else {
                MODEL_SOURCE.to_string()
            };
            let confidence = if metric_source == "Measured local update" {
                0.95
            } else {
                confidence(&row, urban)
            };

            LocationStat {
                pcode: row.pcode,
                region: row.region,
                department: row.department,
                commune: row.commune,
                location: row.location,
                latitude: row.latitude,
                longitude: row.longitude,
                area_sqkm: row.area_sqkm,
                phone_owners,
                population,
                phone_rate: if population > 0 {
                    (phone_owners as f64 / population as f64) * 100.0
                } else {
                    0.0
                },
                metric_source,
                confidence,
                urban_signal: urban,
                data_source: row.data_source,
            }
        })
        .collect()
}

pub(crate) fn validate_gps(latitude: f64, longitude: f64) -> Result<(), String> {
    if !latitude.is_finite() || !longitude.is_finite() {
        return Err("Latitude and longitude must be valid GPS coordinates.".into());
    }

    if !(CAMEROON_MIN_LATITUDE..=CAMEROON_MAX_LATITUDE).contains(&latitude)
        || !(CAMEROON_MIN_LONGITUDE..=CAMEROON_MAX_LONGITUDE).contains(&longitude)
    {
        return Err("GPS coordinates must be inside Cameroon.".into());
    }

    Ok(())
}

pub(crate) fn priority_label(score: f64) -> String {
    if score >= 52.0 {
        "High".into()
    } else if score >= 38.0 {
        "Medium".into()
    } else {
        "Watch".into()
    }
}

pub(crate) fn csv_escape(value: &str) -> String {
    let escaped = value.replace('"', "\"\"");
    format!("\"{}\"", escaped)
}

pub(crate) async fn fetch_location_stats(pool: &PgPool) -> Result<Vec<LocationStat>, sqlx::Error> {
    let rows = sqlx::query_as::<_, DbLocation>(
        r#"
        SELECT
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
        FROM mobile_phone_stats
        ORDER BY region, department, commune
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(apply_matrix(rows))
}

pub(crate) async fn fetch_summary(pool: &PgPool) -> Result<Summary, sqlx::Error> {
    let stats = fetch_location_stats(pool).await?;
    let total_phone_owners = stats.iter().map(|row| row.phone_owners).sum::<i64>();
    let total_population = stats.iter().map(|row| row.population).sum::<i64>();
    let percent_with_phone = if total_population > 0 {
        (total_phone_owners as f64 / total_population as f64) * 100.0
    } else {
        0.0
    };
    let region_count = stats
        .iter()
        .map(|row| &row.region)
        .collect::<HashSet<_>>()
        .len() as i64;
    let department_count = stats
        .iter()
        .map(|row| &row.department)
        .collect::<HashSet<_>>()
        .len() as i64;
    let commune_count = stats
        .iter()
        .map(|row| &row.commune)
        .collect::<HashSet<_>>()
        .len() as i64;
    let measured_location_count = stats
        .iter()
        .filter(|row| row.metric_source == "Measured local update")
        .count() as i64;

    Ok(Summary {
        total_phone_owners,
        total_population,
        percent_with_phone,
        region_count,
        department_count,
        commune_count,
        measured_location_count,
        estimated_location_count: commune_count - measured_location_count,
    })
}

pub(crate) async fn fetch_organizations(pool: &PgPool) -> Result<Vec<Organization>, sqlx::Error> {
    sqlx::query_as::<_, Organization>(
        r#"
        SELECT
            id,
            name,
            org_type,
            contact_name,
            contact_email,
            created_at::TEXT AS created_at
        FROM organizations
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
}

pub(crate) async fn fetch_projects(pool: &PgPool) -> Result<Vec<Project>, sqlx::Error> {
    sqlx::query_as::<_, Project>(
        r#"
        SELECT
            p.id,
            p.organization_id,
            o.name AS organization_name,
            p.name,
            p.sector,
            p.region,
            p.status,
            p.start_date::TEXT AS start_date,
            p.created_at::TEXT AS created_at
        FROM projects p
        LEFT JOIN organizations o ON o.id = p.organization_id
        ORDER BY p.status, p.region NULLS LAST, p.name
        "#,
    )
    .fetch_all(pool)
    .await
}

pub(crate) async fn fetch_site_profiles(pool: &PgPool) -> Result<Vec<SiteProfile>, sqlx::Error> {
    sqlx::query_as::<_, SiteProfile>(
        r#"
        SELECT
            s.id,
            s.project_id,
            p.name AS project_name,
            s.name,
            s.site_type,
            s.region,
            s.department,
            s.commune,
            s.latitude,
            s.longitude,
            s.beneficiary_estimate,
            s.trust_signal,
            s.access_notes,
            s.created_at::TEXT AS created_at
        FROM site_profiles s
        LEFT JOIN projects p ON p.id = s.project_id
        ORDER BY s.region, s.department, s.commune, s.name
        "#,
    )
    .fetch_all(pool)
    .await
}

pub(crate) async fn fetch_survey_campaigns(
    pool: &PgPool,
) -> Result<Vec<SurveyCampaign>, sqlx::Error> {
    sqlx::query_as::<_, SurveyCampaign>(
        r#"
        SELECT
            c.id,
            c.project_id,
            p.name AS project_name,
            c.name,
            c.form_type,
            c.target_region,
            c.target_department,
            c.target_commune,
            c.status,
            c.language_mode,
            c.offline_enabled,
            c.starts_on::TEXT AS starts_on,
            c.ends_on::TEXT AS ends_on,
            c.created_at::TEXT AS created_at
        FROM survey_campaigns c
        LEFT JOIN projects p ON p.id = c.project_id
        ORDER BY
            CASE c.status
                WHEN 'active' THEN 1
                WHEN 'draft' THEN 2
                WHEN 'paused' THEN 3
                ELSE 4
            END,
            c.created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub(crate) async fn fetch_decision_snapshots(
    pool: &PgPool,
) -> Result<Vec<DecisionSnapshot>, sqlx::Error> {
    sqlx::query_as::<_, DecisionSnapshot>(
        r#"
        SELECT
            d.id,
            d.project_id,
            d.site_profile_id,
            d.asset_id,
            p.name AS project_name,
            s.name AS site_name,
            a.name AS asset_name,
            d.title,
            d.decision_stage,
            d.priority_score,
            d.recommended_budget_xaf,
            d.owner_name,
            d.risk_level,
            d.evidence_score,
            d.approval_notes,
            d.execution_status,
            d.rationale,
            d.next_action,
            d.created_at::TEXT AS created_at
        FROM decision_snapshots d
        LEFT JOIN projects p ON p.id = d.project_id
        LEFT JOIN site_profiles s ON s.id = d.site_profile_id
        LEFT JOIN infrastructure_assets a ON a.id = d.asset_id
        ORDER BY d.created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub(crate) fn decision_evidence_score(
    priority_score: f64,
    has_project: bool,
    has_site: bool,
    has_asset: bool,
    budget_set: bool,
) -> f64 {
    let mut score = priority_score.clamp(0.0, 100.0) * 0.35;
    if has_project {
        score += 15.0;
    }
    if has_site {
        score += 18.0;
    }
    if has_asset {
        score += 14.0;
    }
    if budget_set {
        score += 12.0;
    }
    score.min(100.0)
}

pub(crate) fn decision_risk_level(
    priority_score: f64,
    evidence_score: f64,
    budget: Option<i64>,
) -> String {
    if evidence_score < 45.0 || budget.unwrap_or(0) > 3_000_000 {
        "high".into()
    } else if priority_score >= 55.0 || evidence_score < 70.0 {
        "medium".into()
    } else {
        "low".into()
    }
}

pub(crate) async fn build_decision_board(pool: &PgPool) -> Result<DecisionBoard, sqlx::Error> {
    let decisions = fetch_decision_snapshots(pool).await?;
    let stage_names = [
        "draft",
        "recommended",
        "approved",
        "blocked",
        "executing",
        "completed",
    ];
    let stages = stage_names
        .iter()
        .map(|stage| {
            let stage_decisions = decisions
                .iter()
                .filter(|decision| decision.decision_stage == *stage)
                .collect::<Vec<_>>();
            let count = stage_decisions.len() as i64;
            let total_budget_xaf = stage_decisions
                .iter()
                .filter_map(|decision| decision.recommended_budget_xaf)
                .sum::<i64>();
            let average_evidence_score = if count > 0 {
                stage_decisions
                    .iter()
                    .map(|decision| decision.evidence_score)
                    .sum::<f64>()
                    / count as f64
            } else {
                0.0
            };
            DecisionStageSummary {
                stage: (*stage).into(),
                count,
                total_budget_xaf,
                average_evidence_score,
            }
        })
        .collect::<Vec<_>>();

    let needs_evidence = decisions
        .iter()
        .filter(|decision| decision.evidence_score < 55.0)
        .count();
    let approved_not_executing = decisions
        .iter()
        .filter(|decision| {
            decision.decision_stage == "approved" && decision.execution_status == "not_started"
        })
        .count();

    Ok(DecisionBoard {
        stages,
        decisions,
        recommendations: vec![
            format!(
                "{} decisions need stronger field evidence before approval.",
                needs_evidence
            ),
            format!(
                "{} approved decisions are waiting for execution kickoff.",
                approved_not_executing
            ),
            "Keep decisions small, budgeted in XAF, and linked to a site/probe whenever possible."
                .into(),
        ],
    })
}

pub(crate) async fn fetch_execution_plans(
    pool: &PgPool,
) -> Result<Vec<ExecutionPlan>, sqlx::Error> {
    sqlx::query_as::<_, ExecutionPlan>(
        r#"
        SELECT
            e.id,
            e.decision_id,
            d.title AS decision_title,
            e.project_id,
            e.site_profile_id,
            e.asset_id,
            p.name AS project_name,
            s.name AS site_name,
            a.name AS asset_name,
            e.title,
            e.owner_name,
            e.status,
            e.budget_xaf,
            e.planned_start::TEXT AS planned_start,
            e.planned_end::TEXT AS planned_end,
            e.local_focal_point_confirmed,
            e.gps_photo_proof_required,
            e.offline_survey_ready,
            e.bilingual_script_ready,
            e.transport_access_notes,
            e.xaf_budget_approved,
            e.blocker,
            e.outcome_notes,
            e.created_at::TEXT AS created_at,
            e.updated_at::TEXT AS updated_at
        FROM execution_plans e
        LEFT JOIN decision_snapshots d ON d.id = e.decision_id
        LEFT JOIN projects p ON p.id = e.project_id
        LEFT JOIN site_profiles s ON s.id = e.site_profile_id
        LEFT JOIN infrastructure_assets a ON a.id = e.asset_id
        ORDER BY
            CASE e.status
                WHEN 'planned' THEN 1
                WHEN 'ready' THEN 2
                WHEN 'in_progress' THEN 3
                WHEN 'blocked' THEN 4
                ELSE 5
            END,
            e.updated_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

fn plan_checklist_completion(plan: &ExecutionPlan) -> f64 {
    let checks = [
        plan.local_focal_point_confirmed,
        plan.gps_photo_proof_required,
        plan.offline_survey_ready,
        plan.bilingual_script_ready,
        plan.xaf_budget_approved,
    ];
    checks.iter().filter(|value| **value).count() as f64 / checks.len() as f64 * 100.0
}

pub(crate) async fn build_execution_board(pool: &PgPool) -> Result<ExecutionBoard, sqlx::Error> {
    let plans = fetch_execution_plans(pool).await?;
    let statuses = ["planned", "ready", "in_progress", "blocked", "completed"];
    let stages = statuses
        .iter()
        .map(|status| {
            let stage_plans = plans
                .iter()
                .filter(|plan| plan.status == *status)
                .collect::<Vec<_>>();
            let count = stage_plans.len() as i64;
            let total_budget_xaf = stage_plans
                .iter()
                .filter_map(|plan| plan.budget_xaf)
                .sum::<i64>();
            let checklist_completion = if count > 0 {
                stage_plans
                    .iter()
                    .map(|plan| plan_checklist_completion(plan))
                    .sum::<f64>()
                    / count as f64
            } else {
                0.0
            };
            ExecutionStageSummary {
                status: (*status).into(),
                count,
                total_budget_xaf,
                checklist_completion,
            }
        })
        .collect::<Vec<_>>();
    let blocked = plans.iter().filter(|plan| plan.status == "blocked").count();
    let ready = plans.iter().filter(|plan| plan.status == "ready").count();

    Ok(ExecutionBoard {
        stages,
        plans,
        recommendations: vec![
            format!("{} execution plans are ready for field kickoff.", ready),
            format!("{} execution plans are blocked and need owner attention.", blocked),
            "Before field launch, confirm focal point, offline survey readiness, bilingual script, and XAF budget.".into(),
        ],
    })
}

pub(crate) async fn fetch_assets(pool: &PgPool) -> Result<Vec<InfrastructureAsset>, sqlx::Error> {
    sqlx::query_as::<_, InfrastructureAsset>(
        r#"
        SELECT
            a.id,
            a.project_id,
            a.site_profile_id,
            p.name AS project_name,
            s.name AS site_name,
            a.asset_type,
            a.name,
            a.region,
            a.department,
            a.commune,
            a.latitude,
            a.longitude,
            a.status,
            a.operator,
            a.installed_at::TEXT AS installed_at,
            a.last_checked_at::TEXT AS last_checked_at,
            a.notes
        FROM infrastructure_assets a
        LEFT JOIN projects p ON p.id = a.project_id
        LEFT JOIN site_profiles s ON s.id = a.site_profile_id
        ORDER BY a.status DESC, a.region, a.department, a.commune, a.name
        "#,
    )
    .fetch_all(pool)
    .await
}

pub(crate) async fn fetch_reports(pool: &PgPool) -> Result<Vec<FieldReport>, sqlx::Error> {
    sqlx::query_as::<_, FieldReport>(
        r#"
        SELECT
            r.id,
            r.project_id,
            r.site_profile_id,
            r.campaign_id,
            r.asset_id,
            p.name AS project_name,
            s.name AS site_name,
            c.name AS campaign_name,
            r.report_type,
            r.region,
            r.department,
            r.commune,
            r.latitude,
            r.longitude,
            r.status,
            r.evidence_quality,
            r.notes,
            r.submitted_by,
            r.created_at::TEXT AS created_at
        FROM field_reports r
        LEFT JOIN projects p ON p.id = r.project_id
        LEFT JOIN site_profiles s ON s.id = r.site_profile_id
        LEFT JOIN survey_campaigns c ON c.id = r.campaign_id
        ORDER BY r.created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub(crate) async fn fetch_alerts(pool: &PgPool) -> Result<Vec<Alert>, sqlx::Error> {
    sqlx::query_as::<_, Alert>(
        r#"
        SELECT
            a.id,
            a.project_id,
            a.site_profile_id,
            a.asset_id,
            p.name AS project_name,
            s.name AS site_name,
            a.severity,
            a.title,
            a.message,
            a.status,
            a.created_at::TEXT AS created_at,
            a.resolved_at::TEXT AS resolved_at
        FROM alerts a
        LEFT JOIN projects p ON p.id = a.project_id
        LEFT JOIN site_profiles s ON s.id = a.site_profile_id
        ORDER BY
            CASE severity
                WHEN 'critical' THEN 1
                WHEN 'warning' THEN 2
                ELSE 3
            END,
            created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub(crate) async fn fetch_tickets(pool: &PgPool) -> Result<Vec<MaintenanceTicket>, sqlx::Error> {
    sqlx::query_as::<_, MaintenanceTicket>(
        r#"
        SELECT
            t.id,
            t.project_id,
            t.site_profile_id,
            t.asset_id,
            t.alert_id,
            p.name AS project_name,
            s.name AS site_name,
            t.title,
            t.priority,
            t.status,
            t.assigned_to,
            t.due_date::TEXT AS due_date,
            t.sla_hours,
            t.resolution_notes,
            t.created_at::TEXT AS created_at,
            t.updated_at::TEXT AS updated_at
        FROM maintenance_tickets t
        LEFT JOIN projects p ON p.id = t.project_id
        LEFT JOIN site_profiles s ON s.id = t.site_profile_id
        ORDER BY
            CASE t.status
                WHEN 'open' THEN 1
                WHEN 'scheduled' THEN 2
                WHEN 'in_progress' THEN 3
                WHEN 'blocked' THEN 4
                ELSE 5
            END,
            CASE t.priority
                WHEN 'urgent' THEN 1
                WHEN 'high' THEN 2
                WHEN 'medium' THEN 3
                ELSE 4
            END,
            t.due_date ASC NULLS LAST,
            t.created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub(crate) async fn fetch_iot_readings(pool: &PgPool) -> Result<Vec<IotReading>, sqlx::Error> {
    sqlx::query_as::<_, IotReading>(
        r#"
        SELECT
            r.id,
            r.project_id,
            r.site_profile_id,
            r.asset_id,
            p.name AS project_name,
            s.name AS site_name,
            r.reading_type,
            r.value,
            r.unit,
            r.latitude,
            r.longitude,
            r.created_at::TEXT AS created_at
        FROM iot_readings r
        LEFT JOIN projects p ON p.id = r.project_id
        LEFT JOIN site_profiles s ON s.id = r.site_profile_id
        ORDER BY r.created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

fn asset_health_label(score: f64) -> String {
    if score >= 82.0 {
        "Healthy".into()
    } else if score >= 62.0 {
        "Watch".into()
    } else if score >= 42.0 {
        "Needs action".into()
    } else {
        "Critical".into()
    }
}

fn asset_recommended_action(
    asset: &InfrastructureAsset,
    open_alerts: i64,
    active_tickets: i64,
    reading_count: i64,
    report_count: i64,
) -> String {
    if asset.status == "critical" || asset.status == "offline" {
        return "Create urgent maintenance ticket and collect field confirmation before closing the incident.".into();
    }
    if open_alerts > 0 {
        return "Resolve open validation alerts and attach a field report to rebuild trust evidence.".into();
    }
    if active_tickets > 0 {
        return "Follow the active ticket to completion and update the probe status after technician feedback.".into();
    }
    if reading_count == 0 {
        return "Submit first telemetry reading so the probe can be monitored over time.".into();
    }
    if report_count == 0 {
        return "Add GPS/photo survey evidence for local accountability and buyer confidence."
            .into();
    }
    "Keep monitoring; this probe has usable telemetry and field evidence.".into()
}

pub(crate) async fn build_signal_probe_dashboard(
    pool: &PgPool,
) -> Result<SignalProbeDashboard, sqlx::Error> {
    let assets = fetch_assets(pool).await?;
    let alerts = fetch_alerts(pool).await?;
    let tickets = fetch_tickets(pool).await?;
    let reports = fetch_reports(pool).await?;
    let readings = fetch_iot_readings(pool).await?;

    let mut alert_counts: HashMap<i64, i64> = HashMap::new();
    for alert in alerts.iter().filter(|alert| alert.status != "resolved") {
        if let Some(asset_id) = alert.asset_id {
            *alert_counts.entry(asset_id).or_default() += 1;
        }
    }

    let mut ticket_counts: HashMap<i64, i64> = HashMap::new();
    for ticket in tickets
        .iter()
        .filter(|ticket| ticket.status != "done" && ticket.status != "cancelled")
    {
        if let Some(asset_id) = ticket.asset_id {
            *ticket_counts.entry(asset_id).or_default() += 1;
        }
    }

    let mut report_counts: HashMap<i64, i64> = HashMap::new();
    for report in reports.iter().filter_map(|report| report.asset_id) {
        *report_counts.entry(report).or_default() += 1;
    }

    let mut reading_counts: HashMap<i64, i64> = HashMap::new();
    let mut latest_readings: HashMap<i64, String> = HashMap::new();
    for reading in readings {
        *reading_counts.entry(reading.asset_id).or_default() += 1;
        latest_readings.entry(reading.asset_id).or_insert_with(|| {
            format!(
                "{} {} {}",
                reading.reading_type, reading.value, reading.unit
            )
        });
    }

    let mut health = assets
        .iter()
        .map(|asset| {
            let open_alerts = alert_counts.get(&asset.id).copied().unwrap_or(0);
            let active_tickets = ticket_counts.get(&asset.id).copied().unwrap_or(0);
            let report_count = report_counts.get(&asset.id).copied().unwrap_or(0);
            let reading_count = reading_counts.get(&asset.id).copied().unwrap_or(0);
            let status_penalty = match asset.status.as_str() {
                "online" => 0.0,
                "warning" => 18.0,
                "critical" => 38.0,
                "offline" => 48.0,
                _ => 10.0,
            };
            let evidence_bonus = ((report_count.min(3) + reading_count.min(3)) as f64) * 4.0;
            let health_score = (88.0 + evidence_bonus
                - status_penalty
                - (open_alerts as f64 * 14.0)
                - (active_tickets as f64 * 8.0))
                .clamp(0.0, 100.0);

            SignalProbeHealth {
                asset_id: asset.id,
                health_score,
                health_label: asset_health_label(health_score),
                open_alerts,
                active_tickets,
                report_count,
                reading_count,
                latest_reading: latest_readings.get(&asset.id).cloned(),
                recommended_action: asset_recommended_action(
                    asset,
                    open_alerts,
                    active_tickets,
                    reading_count,
                    report_count,
                ),
            }
        })
        .collect::<Vec<_>>();

    health.sort_by(|a, b| {
        a.health_score
            .partial_cmp(&b.health_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(SignalProbeDashboard {
        total_probes: assets.len() as i64,
        online_probes: assets
            .iter()
            .filter(|asset| asset.status == "online")
            .count() as i64,
        warning_probes: assets
            .iter()
            .filter(|asset| asset.status == "warning")
            .count() as i64,
        critical_probes: assets
            .iter()
            .filter(|asset| asset.status == "critical")
            .count() as i64,
        offline_probes: assets
            .iter()
            .filter(|asset| asset.status == "offline")
            .count() as i64,
        open_alerts: alerts
            .iter()
            .filter(|alert| alert.status != "resolved")
            .count() as i64,
        active_tickets: tickets
            .iter()
            .filter(|ticket| ticket.status != "done" && ticket.status != "cancelled")
            .count() as i64,
        health,
    })
}

fn same_area(region: &str, department: &str, commune: &str, item: (&str, &str, &str)) -> bool {
    region == item.0 && department == item.1 && commune == item.2
}

fn area_budget_xaf(area: &LocationStat, open_alerts: usize) -> i64 {
    let population_component = ((area.population as f64) * 5.5).round() as i64;
    let validation_component = if area.confidence < 0.68 {
        380_000
    } else {
        180_000
    };
    let travel_component = if area.phone_rate < 65.0 {
        260_000
    } else {
        120_000
    };
    450_000
        + population_component.min(1_900_000)
        + validation_component
        + (open_alerts as i64 * 300_000)
        + travel_component
}

fn area_reach(area: &LocationStat) -> i64 {
    let reach_factor = ((100.0 - area.phone_rate) / 180.0).clamp(0.08, 0.32);
    ((area.population as f64) * reach_factor).round() as i64
}

fn area_channel(area: &LocationStat) -> String {
    if area.phone_rate < 65.0 {
        "Offline forms, SMS follow-up, and local focal-point validation".into()
    } else if area.confidence < 0.70 {
        "GPS/photo survey with WhatsApp supervisor coordination".into()
    } else {
        "WhatsApp coordination with targeted GPS proof checks".into()
    }
}

fn area_execution_risk(
    area: &LocationStat,
    asset_count: usize,
    site_count: usize,
    alert_count: usize,
) -> String {
    let risk = (if area.confidence < 0.68 { 2 } else { 0 })
        + (if area.phone_rate < 65.0 { 2 } else { 0 })
        + (if alert_count > 0 { 2 } else { 0 })
        + (if asset_count == 0 { 1 } else { 0 })
        + (if site_count == 0 { 1 } else { 0 });
    if risk >= 5 {
        "High risk".into()
    } else if risk >= 3 {
        "Medium risk".into()
    } else {
        "Controlled risk".into()
    }
}

fn area_next_action(
    area: &LocationStat,
    asset_count: usize,
    site_count: usize,
    campaign_count: usize,
    alert_count: usize,
    priority: Option<&PriorityZone>,
) -> String {
    if alert_count > 0 {
        "Resolve open alerts, attach field evidence, and update probe status before new spend."
            .into()
    } else if site_count == 0 {
        "Create a trusted site profile with GPS/photo proof and a named local focal point.".into()
    } else if asset_count == 0 {
        "Install or register a signal probe so the area has operational telemetry.".into()
    } else if campaign_count == 0 || area.confidence < 0.70 {
        "Launch a bilingual offline survey campaign to strengthen confidence before approval."
            .into()
    } else if priority
        .map(|zone| zone.priority_score >= 52.0)
        .unwrap_or(false)
    {
        "Prepare a decision snapshot with budget, reach, and execution owner.".into()
    } else {
        "Keep monitoring and refresh the dossier when new survey or telemetry evidence arrives."
            .into()
    }
}

pub(crate) async fn build_area_dossier(
    pool: &PgPool,
    region: &str,
    department: &str,
    commune: &str,
) -> Result<Option<AreaDossier>, sqlx::Error> {
    let stats = fetch_location_stats(pool).await?;
    let Some(area) = stats.into_iter().find(|item| {
        same_area(
            region,
            department,
            commune,
            (&item.region, &item.department, &item.commune),
        )
    }) else {
        return Ok(None);
    };

    let priority = build_priority_zones(pool).await?.into_iter().find(|zone| {
        same_area(
            region,
            department,
            commune,
            (&zone.region, &zone.department, &zone.commune),
        )
    });
    let assets = fetch_assets(pool)
        .await?
        .into_iter()
        .filter(|asset| {
            same_area(
                region,
                department,
                commune,
                (&asset.region, &asset.department, &asset.commune),
            )
        })
        .collect::<Vec<_>>();
    let sites = fetch_site_profiles(pool)
        .await?
        .into_iter()
        .filter(|site| {
            same_area(
                region,
                department,
                commune,
                (&site.region, &site.department, &site.commune),
            )
        })
        .collect::<Vec<_>>();
    let campaigns = fetch_survey_campaigns(pool)
        .await?
        .into_iter()
        .filter(|campaign| {
            campaign
                .target_region
                .as_deref()
                .is_none_or(|value| value == region)
                && campaign
                    .target_department
                    .as_deref()
                    .is_none_or(|value| value == department)
                && campaign
                    .target_commune
                    .as_deref()
                    .is_none_or(|value| value == commune)
        })
        .collect::<Vec<_>>();
    let reports = fetch_reports(pool)
        .await?
        .into_iter()
        .filter(|report| {
            same_area(
                region,
                department,
                commune,
                (&report.region, &report.department, &report.commune),
            )
        })
        .collect::<Vec<_>>();

    let asset_ids = assets.iter().map(|asset| asset.id).collect::<HashSet<_>>();
    let site_ids = sites.iter().map(|site| site.id).collect::<HashSet<_>>();
    let alerts = fetch_alerts(pool)
        .await?
        .into_iter()
        .filter(|alert| {
            alert.status != "resolved"
                && (alert
                    .asset_id
                    .map(|id| asset_ids.contains(&id))
                    .unwrap_or(false)
                    || alert
                        .site_profile_id
                        .map(|id| site_ids.contains(&id))
                        .unwrap_or(false))
        })
        .collect::<Vec<_>>();
    let tickets = fetch_tickets(pool)
        .await?
        .into_iter()
        .filter(|ticket| {
            ticket.status != "done"
                && ticket.status != "cancelled"
                && (ticket
                    .asset_id
                    .map(|id| asset_ids.contains(&id))
                    .unwrap_or(false)
                    || ticket
                        .site_profile_id
                        .map(|id| site_ids.contains(&id))
                        .unwrap_or(false))
        })
        .collect::<Vec<_>>();
    let readings = fetch_iot_readings(pool)
        .await?
        .into_iter()
        .filter(|reading| asset_ids.contains(&reading.asset_id))
        .collect::<Vec<_>>();

    let economics = AreaEconomics {
        estimated_budget_xaf: area_budget_xaf(&area, alerts.len()),
        likely_reach: area_reach(&area),
        channel_strategy: area_channel(&area),
        execution_risk: area_execution_risk(&area, assets.len(), sites.len(), alerts.len()),
        next_action: area_next_action(
            &area,
            assets.len(),
            sites.len(),
            campaigns.len(),
            alerts.len(),
            priority.as_ref(),
        ),
        trust_gap: if area.confidence < 0.70 {
            "Confidence is below the approval threshold; collect GPS/photo evidence before major budget release.".into()
        } else if sites.is_empty() {
            "No trusted site profile exists; buyers may struggle to map the recommendation to a named place.".into()
        } else {
            "Trust evidence is usable; keep telemetry and field reports current.".into()
        },
    };

    Ok(Some(AreaDossier {
        area,
        priority,
        economics,
        assets,
        sites,
        campaigns,
        reports,
        alerts,
        tickets,
        readings,
        market_notes: vec![
            "Use local administrative names in every field task so teams and partners recognize the place immediately.".into(),
            "Pair phone-access estimates with visible proof; confidence, GPS evidence, and local focal points reduce buyer hesitation.".into(),
            "Keep the first action small enough to approve: probe, survey, ticket, or decision snapshot with XAF budget.".into(),
        ],
    }))
}

pub(crate) async fn build_workspace_health(pool: &PgPool) -> Result<WorkspaceHealth, sqlx::Error> {
    let organizations = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM organizations")
        .fetch_one(pool)
        .await?
        .0;
    let projects = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM projects")
        .fetch_one(pool)
        .await?
        .0;
    let sites = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM site_profiles")
        .fetch_one(pool)
        .await?
        .0;
    let campaigns = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM survey_campaigns")
        .fetch_one(pool)
        .await?
        .0;
    let monitored_assets =
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM infrastructure_assets")
            .fetch_one(pool)
            .await?
            .0;
    let open_alerts =
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM alerts WHERE status <> 'resolved'")
            .fetch_one(pool)
            .await?
            .0;
    let active_tickets = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM maintenance_tickets WHERE status NOT IN ('done', 'cancelled')",
    )
    .fetch_one(pool)
    .await?
    .0;
    let decision_snapshots = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM decision_snapshots")
        .fetch_one(pool)
        .await?
        .0;

    Ok(WorkspaceHealth {
        organizations,
        projects,
        sites,
        campaigns,
        monitored_assets,
        open_alerts,
        active_tickets,
        decision_snapshots,
    })
}

pub(crate) fn market_realities() -> Vec<String> {
    vec![
        "Mobile-first workflows: field teams may rely on phones, prepaid data, WhatsApp, SMS, and offline capture.".into(),
        "Trust is operational: GPS proof, visible confidence, named local contacts, and evidence quality matter before spend decisions.".into(),
        "Bilingual execution: French/English labels and local administrative names reduce friction across regions.".into(),
        "Connectivity is uneven: surveys and telemetry must tolerate low bandwidth, delayed sync, and rural access constraints.".into(),
    ]
}

fn estimate_overview_budget_xaf(zone: &PriorityZone) -> i64 {
    let population_component = ((zone.population as f64) * 5.5).round() as i64;
    let validation_component = if zone.confidence < 0.68 {
        380_000
    } else {
        180_000
    };
    let alert_component = zone.open_alert_count * 300_000;
    let travel_component = if zone.phone_rate < 65.0 {
        260_000
    } else {
        120_000
    };

    450_000
        + population_component.min(1_900_000)
        + validation_component
        + alert_component
        + travel_component
}

fn estimate_overview_reach(zone: &PriorityZone) -> i64 {
    let reach_factor = ((100.0 - zone.phone_rate) / 180.0).clamp(0.08, 0.32);
    ((zone.population as f64) * reach_factor).round() as i64
}

fn overview_channel(zone: &PriorityZone) -> String {
    if zone.phone_rate < 65.0 {
        "Offline forms, SMS follow-up, and a named local focal point".into()
    } else if zone.confidence < 0.70 {
        "GPS/photo validation with WhatsApp coordination for supervisors".into()
    } else {
        "WhatsApp coordination with targeted GPS spot checks".into()
    }
}

fn overview_next_action(zone: &PriorityZone) -> String {
    if zone.open_alert_count > 0 {
        "Resolve open alerts before approving a new deployment sprint.".into()
    } else if zone.asset_count == 0 {
        "Create a site profile, assign local contact, and collect GPS/photo proof.".into()
    } else if zone.report_count == 0 {
        "Launch a bilingual offline survey campaign for ownership and signal proof.".into()
    } else {
        "Prepare decision snapshot with budget, reach, and field evidence.".into()
    }
}

fn compact_xaf(value: i64) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M XAF", value as f64 / 1_000_000.0)
    } else {
        format!("{}K XAF", (value as f64 / 1_000.0).round() as i64)
    }
}

pub(crate) async fn build_overview_intelligence(
    pool: &PgPool,
) -> Result<OverviewIntelligence, sqlx::Error> {
    let summary = fetch_summary(pool).await?;
    let zones = build_priority_zones(pool).await?;
    let health = build_workspace_health(pool).await?;
    let sites = fetch_site_profiles(pool).await?;
    let campaigns = fetch_survey_campaigns(pool).await?;
    let decisions = fetch_decision_snapshots(pool).await?;
    let tickets = fetch_tickets(pool).await?;

    let top_opportunities = zones
        .iter()
        .take(5)
        .map(|zone| {
            let estimated_budget_xaf = estimate_overview_budget_xaf(zone);
            let likely_reach = estimate_overview_reach(zone);
            let business_case = format!(
                "{} people, {:.1}% phone ownership, {:.0}% confidence, and {} monitored assets create a {} opportunity.",
                zone.population,
                zone.phone_rate,
                zone.confidence * 100.0,
                zone.asset_count,
                zone.priority_label.to_lowercase()
            );

            OverviewOpportunity {
                region: zone.region.clone(),
                department: zone.department.clone(),
                commune: zone.commune.clone(),
                priority_score: zone.priority_score,
                priority_label: zone.priority_label.clone(),
                population: zone.population,
                phone_rate: zone.phone_rate,
                confidence: zone.confidence,
                estimated_budget_xaf,
                likely_reach,
                recommended_channel: overview_channel(zone),
                business_case,
                next_action: overview_next_action(zone),
            }
        })
        .collect::<Vec<_>>();

    let total_pipeline_budget = top_opportunities
        .iter()
        .map(|opportunity| opportunity.estimated_budget_xaf)
        .sum::<i64>();
    let total_pipeline_reach = top_opportunities
        .iter()
        .map(|opportunity| opportunity.likely_reach)
        .sum::<i64>();
    let low_confidence_count = zones.iter().filter(|zone| zone.confidence < 0.68).count() as i64;
    let weak_access_count = zones.iter().filter(|zone| zone.phone_rate < 65.0).count() as i64;
    let evidence_ready = sites.len() as i64 + campaigns.len() as i64 + decisions.len() as i64;

    let mut action_queue = Vec::new();
    if let Some(zone) = zones
        .iter()
        .find(|zone| zone.open_alert_count > 0 || zone.priority_score >= 52.0)
    {
        action_queue.push(OverviewAction {
            title: format!("Turn {} into a decision-ready sprint", zone.commune),
            area: Some(format!("{}, {}", zone.department, zone.region)),
            action_type: "decision".into(),
            urgency: if zone.open_alert_count > 0 {
                "urgent".into()
            } else {
                "high".into()
            },
            reason: overview_next_action(zone),
        });
    }
    if health.sites < health.projects {
        action_queue.push(OverviewAction {
            title: "Attach proof sites to every active workspace".into(),
            area: None,
            action_type: "site".into(),
            urgency: "high".into(),
            reason: "Cameroon buyers trust named places, GPS proof, and visible local contacts before budget release.".into(),
        });
    }
    if health.campaigns < health.projects {
        action_queue.push(OverviewAction {
            title: "Create offline survey campaigns for thin-evidence projects".into(),
            area: None,
            action_type: "campaign".into(),
            urgency: "medium".into(),
            reason: "Uneven connectivity means offline capture and delayed sync should be planned before field launch.".into(),
        });
    }
    if action_queue.len() < 4 {
        action_queue.extend(tickets.iter().take(2).map(|ticket| {
            OverviewAction {
                title: ticket.title.clone(),
                area: ticket
                    .site_name
                    .clone()
                    .or_else(|| ticket.project_name.clone()),
                action_type: "maintenance".into(),
                urgency: ticket.priority.clone(),
                reason: format!(
                    "{} ticket assigned to {}",
                    ticket.status,
                    ticket.assigned_to.as_deref().unwrap_or("unassigned team")
                ),
            }
        }));
    }

    let trust_risks = vec![
        OverviewRisk {
            label: "Low-confidence model rows".into(),
            value: low_confidence_count.to_string(),
            severity: if low_confidence_count > 120 {
                "high".into()
            } else {
                "medium".into()
            },
            mitigation: "Prioritize GPS/photo surveys before large deployment decisions.".into(),
        },
        OverviewRisk {
            label: "Weak phone ownership areas".into(),
            value: weak_access_count.to_string(),
            severity: if weak_access_count > 80 {
                "high".into()
            } else {
                "medium".into()
            },
            mitigation: "Use offline-first forms, SMS follow-up, and local focal points.".into(),
        },
        OverviewRisk {
            label: "Open alerts".into(),
            value: health.open_alerts.to_string(),
            severity: if health.open_alerts > 3 {
                "high".into()
            } else {
                "watch".into()
            },
            mitigation:
                "Convert repeated validation signals into tickets with owners and due dates.".into(),
        },
    ];

    Ok(OverviewIntelligence {
        generated_for: "InfraPulse Cameroon operating overview".into(),
        kpis: vec![
            OverviewKpi {
                label: "Opportunity pipeline".into(),
                value: compact_xaf(total_pipeline_budget),
                detail: format!(
                    "Top {} areas can directly reach about {} people.",
                    top_opportunities.len(),
                    total_pipeline_reach
                ),
                tone: "blue".into(),
            },
            OverviewKpi {
                label: "Trust proof assets".into(),
                value: evidence_ready.to_string(),
                detail: format!(
                    "{} sites, {} campaigns, {} decision records.",
                    sites.len(),
                    campaigns.len(),
                    decisions.len()
                ),
                tone: "green".into(),
            },
            OverviewKpi {
                label: "Execution load".into(),
                value: health.active_tickets.to_string(),
                detail: format!("{} open alerts need field attention.", health.open_alerts),
                tone: "gold".into(),
            },
            OverviewKpi {
                label: "National access model".into(),
                value: format!("{:.1}%", summary.percent_with_phone),
                detail: format!(
                    "{} arrondissements across {} regions.",
                    summary.commune_count, summary.region_count
                ),
                tone: "red".into(),
            },
        ],
        top_opportunities,
        action_queue,
        trust_risks,
        market_readout: vec![
            "Design for fast scanning first; many users decide from a phone in the field, not a quiet office.".into(),
            "Make confidence and proof visible because procurement trust often depends on named places, local contacts, and evidence quality.".into(),
            "Bundle recommendations as small pilots with clear XAF budgets, beneficiary reach, and next field action.".into(),
        ],
    })
}

pub(crate) fn validate_required(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{} is required.", label))
    } else {
        Ok(())
    }
}

pub(crate) async fn build_priority_zones(pool: &PgPool) -> Result<Vec<PriorityZone>, sqlx::Error> {
    let stats = fetch_location_stats(pool).await?;
    let assets = fetch_assets(pool).await?;
    let reports = fetch_reports(pool).await?;
    let alerts = fetch_alerts(pool).await?;

    let mut asset_counts: HashMap<(String, String, String), i64> = HashMap::new();
    for asset in assets {
        *asset_counts
            .entry((asset.region, asset.department, asset.commune))
            .or_default() += 1;
    }

    let mut report_counts: HashMap<(String, String, String), i64> = HashMap::new();
    for report in reports {
        *report_counts
            .entry((report.region, report.department, report.commune))
            .or_default() += 1;
    }

    let mut alert_counts: HashMap<i64, i64> = HashMap::new();
    for alert in alerts {
        if alert.status != "resolved" {
            if let Some(asset_id) = alert.asset_id {
                *alert_counts.entry(asset_id).or_default() += 1;
            }
        }
    }

    let assets_for_alerts = fetch_assets(pool).await?;
    let mut open_alert_counts: HashMap<(String, String, String), i64> = HashMap::new();
    for asset in assets_for_alerts {
        let count = alert_counts.get(&asset.id).copied().unwrap_or(0);
        if count > 0 {
            *open_alert_counts
                .entry((asset.region, asset.department, asset.commune))
                .or_default() += count;
        }
    }

    let mut zones = stats
        .into_iter()
        .map(|row| {
            let key = (
                row.region.clone(),
                row.department.clone(),
                row.commune.clone(),
            );
            let asset_count = asset_counts.get(&key).copied().unwrap_or(0);
            let report_count = report_counts.get(&key).copied().unwrap_or(0);
            let open_alert_count = open_alert_counts.get(&key).copied().unwrap_or(0);
            let population_component = ((row.population as f64 / 150_000.0).min(1.0)) * 28.0;
            let connectivity_gap = ((100.0 - row.phone_rate).max(0.0) / 100.0) * 28.0;
            let alert_component = (open_alert_count as f64 * 16.0).min(28.0);
            let report_component = (report_count as f64 * 7.0).min(14.0);
            let confidence_component = ((1.0 - row.confidence).max(0.0)) * 12.0;
            let infrastructure_component = if asset_count == 0 { 10.0 } else { 0.0 };
            let priority_score = (population_component
                + connectivity_gap
                + alert_component
                + report_component
                + confidence_component
                + infrastructure_component)
                .min(100.0);

            PriorityZone {
                pcode: row.pcode,
                region: row.region,
                department: row.department,
                commune: row.commune,
                latitude: row.latitude,
                longitude: row.longitude,
                population: row.population,
                phone_rate: row.phone_rate,
                confidence: row.confidence,
                asset_count,
                open_alert_count,
                report_count,
                priority_score,
                priority_label: priority_label(priority_score),
            }
        })
        .collect::<Vec<_>>();

    zones.sort_by(|a, b| {
        b.priority_score
            .partial_cmp(&a.priority_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(zones)
}
