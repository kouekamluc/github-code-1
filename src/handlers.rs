use actix_web::{get, patch, post, web, HttpResponse, Responder};
use sqlx::PgPool;

use crate::models::*;
use crate::services::*;

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
    pool: web::Data<PgPool>,
    payload: web::Json<UpdateLocationRequest>,
) -> impl Responder {
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
    pool: web::Data<PgPool>,
    payload: web::Json<SiteProfileRequest>,
) -> impl Responder {
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
    pool: web::Data<PgPool>,
    payload: web::Json<SurveyCampaignRequest>,
) -> impl Responder {
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
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
    payload: web::Json<SurveyCampaignStatusRequest>,
) -> impl Responder {
    let allowed = [
        "draft",
        "ready",
        "in_field",
        "reviewing",
        "completed",
        "paused",
        "cancelled",
        "active",
    ];
    if !allowed.contains(&payload.status.as_str()) {
        return HttpResponse::BadRequest().json(ApiError {
            message: "Unsupported campaign status.".into(),
        });
    }

    let result = sqlx::query("UPDATE survey_campaigns SET status = $1 WHERE id = $2")
        .bind(&payload.status)
        .bind(*path)
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
    pool: web::Data<PgPool>,
    payload: web::Json<DecisionSnapshotRequest>,
) -> impl Responder {
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
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
    payload: web::Json<DecisionStatusRequest>,
) -> impl Responder {
    let stage = payload.decision_stage.trim();
    if !matches!(
        stage,
        "draft" | "recommended" | "approved" | "blocked" | "executing" | "completed"
    ) {
        return HttpResponse::BadRequest().json(ApiError {
            message: "Decision stage is not supported.".into(),
        });
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
    .bind(*path)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => match build_decision_board(pool.get_ref()).await {
            Ok(board) => HttpResponse::Ok().json(board),
            Err(err) => {
                eprintln!("Failed to return decision board: {}", err);
                HttpResponse::InternalServerError().finish()
            }
        },
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
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
) -> impl Responder {
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
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
    payload: web::Json<ExecutionPlanStatusRequest>,
) -> impl Responder {
    let status = payload.status.trim();
    if !matches!(
        status,
        "planned" | "ready" | "in_progress" | "blocked" | "completed"
    ) {
        return HttpResponse::BadRequest().json(ApiError {
            message: "Execution status is not supported.".into(),
        });
    }

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
    .bind(*path)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => match build_execution_board(pool.get_ref()).await {
            Ok(board) => HttpResponse::Ok().json(board),
            Err(err) => {
                eprintln!("Failed to return execution board: {}", err);
                HttpResponse::InternalServerError().finish()
            }
        },
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
    pool: web::Data<PgPool>,
    payload: web::Json<OrganizationRequest>,
) -> impl Responder {
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
    pool: web::Data<PgPool>,
    payload: web::Json<ProjectRequest>,
) -> impl Responder {
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
    pool: web::Data<PgPool>,
    payload: web::Json<AssetRequest>,
) -> impl Responder {
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

#[patch("/api/assets/{id}/status")]
pub(crate) async fn update_asset_status(
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
    payload: web::Json<AssetStatusRequest>,
) -> impl Responder {
    let status = payload.status.trim();
    if !matches!(status, "online" | "warning" | "critical" | "offline") {
        return HttpResponse::BadRequest().json(ApiError {
            message: "Asset status must be online, warning, critical, or offline.".into(),
        });
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
    .bind(*path)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => match fetch_assets(pool.get_ref()).await {
            Ok(assets) => HttpResponse::Ok().json(assets),
            Err(err) => {
                eprintln!("Failed to return assets after status update: {}", err);
                HttpResponse::InternalServerError().finish()
            }
        },
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
    pool: web::Data<PgPool>,
    payload: web::Json<FieldReportRequest>,
) -> impl Responder {
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
    pool: web::Data<PgPool>,
    payload: web::Json<AlertRequest>,
) -> impl Responder {
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
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
    payload: web::Json<AlertStatusRequest>,
) -> impl Responder {
    let result = sqlx::query(
        r#"
        UPDATE alerts
        SET status = $1,
            resolved_at = CASE WHEN $1 = 'resolved' THEN NOW() ELSE NULL END
        WHERE id = $2
        "#,
    )
    .bind(&payload.status)
    .bind(*path)
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
    pool: web::Data<PgPool>,
    payload: web::Json<MaintenanceTicketRequest>,
) -> impl Responder {
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
        Ok(_) => match fetch_tickets(pool.get_ref()).await {
            Ok(tickets) => HttpResponse::Ok().json(tickets),
            Err(err) => {
                eprintln!("Failed to return tickets: {}", err);
                HttpResponse::InternalServerError().finish()
            }
        },
        Err(err) => {
            eprintln!("Failed to create ticket: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[patch("/api/tickets/{id}")]
pub(crate) async fn update_ticket_status(
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
    payload: web::Json<MaintenanceTicketStatusRequest>,
) -> impl Responder {
    let result = sqlx::query(
        r#"
        UPDATE maintenance_tickets
        SET status = $1,
            resolution_notes = COALESCE($2, resolution_notes),
            updated_at = NOW()
        WHERE id = $3
        "#,
    )
    .bind(&payload.status)
    .bind(&payload.resolution_notes)
    .bind(*path)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => match fetch_tickets(pool.get_ref()).await {
            Ok(tickets) => HttpResponse::Ok().json(tickets),
            Err(err) => {
                eprintln!("Failed to return tickets: {}", err);
                HttpResponse::InternalServerError().finish()
            }
        },
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
    pool: web::Data<PgPool>,
    payload: web::Json<IotReadingRequest>,
) -> impl Responder {
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
