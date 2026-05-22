use sqlx::PgPool;

use crate::models::SeedLocation;

pub(crate) async fn seed_operational_demo(pool: &PgPool) -> Result<(), sqlx::Error> {
    let seeded_orgs: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM organizations")
        .fetch_one(pool)
        .await?;
    if seeded_orgs.0 == 0 {
        let organizations = vec![
            (
                "Littoral Water Council Pilot",
                "municipal_council",
                "Council operations desk",
                "ops@littoral-water.local",
            ),
            (
                "Solar Clinics Cameroon",
                "solar_operator",
                "Rural energy coordinator",
                "field@solarclinics.local",
            ),
        ];

        for organization in organizations {
            sqlx::query(
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
            .bind(organization.0)
            .bind(organization.1)
            .bind(organization.2)
            .bind(organization.3)
            .execute(pool)
            .await?;
        }
    }

    let seeded_projects: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM projects")
        .fetch_one(pool)
        .await?;
    if seeded_projects.0 == 0 {
        sqlx::query(
            r#"
            INSERT INTO projects (organization_id, name, sector, region, status, start_date)
            SELECT id, 'Water point reliability pilot', 'water', 'Littoral', 'active', '2026-05-01'::DATE
            FROM organizations
            WHERE name = 'Littoral Water Council Pilot'
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO projects (organization_id, name, sector, region, status, start_date)
            SELECT id, 'Clinic solar uptime monitoring', 'solar', 'Sud-Ouest', 'planning', '2026-06-01'::DATE
            FROM organizations
            WHERE name = 'Solar Clinics Cameroon'
            "#,
        )
        .execute(pool)
        .await?;
    }

    let seeded_sites: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM site_profiles")
        .fetch_one(pool)
        .await?;
    if seeded_sites.0 == 0 {
        sqlx::query(
            r#"
            INSERT INTO site_profiles (
                project_id, name, site_type, region, department, commune, latitude,
                longitude, beneficiary_estimate, trust_signal, access_notes
            )
            SELECT id, 'Bare-Bakem water cluster', 'water_cluster', 'Littoral', 'Moungo',
                   'Bare-Bakem', 4.9827, 10.0167, 8200, 'council_agent_verified',
                   'Road access depends on rain; field team should call local focal point first.'
            FROM projects
            WHERE name = 'Water point reliability pilot'
            ON CONFLICT (name, commune)
            DO UPDATE SET
                project_id = EXCLUDED.project_id,
                beneficiary_estimate = EXCLUDED.beneficiary_estimate,
                trust_signal = EXCLUDED.trust_signal,
                access_notes = EXCLUDED.access_notes
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO site_profiles (
                project_id, name, site_type, region, department, commune, latitude,
                longitude, beneficiary_estimate, trust_signal, access_notes
            )
            SELECT id, 'Buea clinic energy site', 'clinic', 'Sud-Ouest', 'Fako',
                   'Buea', 4.1575, 9.2407, 4300, 'clinic_staff_verified',
                   'Use bilingual field form; clinic staff may submit photos offline.'
            FROM projects
            WHERE name = 'Clinic solar uptime monitoring'
            ON CONFLICT (name, commune)
            DO UPDATE SET
                project_id = EXCLUDED.project_id,
                beneficiary_estimate = EXCLUDED.beneficiary_estimate,
                trust_signal = EXCLUDED.trust_signal,
                access_notes = EXCLUDED.access_notes
            "#,
        )
        .execute(pool)
        .await?;
    }

    let seeded_campaigns: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM survey_campaigns")
        .fetch_one(pool)
        .await?;
    if seeded_campaigns.0 == 0 {
        sqlx::query(
            r#"
            INSERT INTO survey_campaigns (
                project_id, name, form_type, target_region, target_department,
                target_commune, status, language_mode, offline_enabled, starts_on, ends_on
            )
            SELECT id, 'Littoral water trust baseline', 'gps_photo_survey', 'Littoral',
                   'Moungo', 'Bare-Bakem', 'active', 'bilingual', TRUE,
                   CURRENT_DATE, CURRENT_DATE + INTERVAL '21 days'
            FROM projects
            WHERE name = 'Water point reliability pilot'
            ON CONFLICT (project_id, name)
            DO UPDATE SET
                status = EXCLUDED.status,
                offline_enabled = EXCLUDED.offline_enabled,
                ends_on = EXCLUDED.ends_on
            "#,
        )
        .execute(pool)
        .await?;
    }

    let assets = vec![
        (
            "water_point",
            "Moungo borehole cluster",
            "Littoral",
            "Moungo",
            "Bare-Bakem",
            4.9827,
            10.0167,
            "warning",
            "Council water unit",
            "2024-06-12",
            "Flow drops after evening demand peaks.",
        ),
        (
            "solar_system",
            "Buea clinic solar backup",
            "Sud-Ouest",
            "Fako",
            "Buea",
            4.1575,
            9.2407,
            "online",
            "Solar partner",
            "2024-03-20",
            "Clinic backup system with battery telemetry.",
        ),
        (
            "connectivity_probe",
            "Garoua market signal probe",
            "Nord",
            "Bénoué",
            "Garoua I",
            9.3014,
            13.3977,
            "online",
            "InfraPulse field team",
            "2025-01-10",
            "Measures evening mobile signal quality.",
        ),
        (
            "water_point",
            "Ngaoundéré pump station",
            "Adamaoua",
            "Vina",
            "Ngaoundéré I",
            7.3277,
            13.5847,
            "critical",
            "Community operator",
            "2023-11-05",
            "Pump requires maintenance follow-up.",
        ),
    ];

    for asset in assets {
        sqlx::query(
            r#"
            INSERT INTO infrastructure_assets (
                asset_type, name, region, department, commune, latitude, longitude,
                status, operator, installed_at, last_checked_at, notes
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10::DATE, NOW(), $11)
            ON CONFLICT (name, commune)
            DO UPDATE SET
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
        .bind(asset.0)
        .bind(asset.1)
        .bind(asset.2)
        .bind(asset.3)
        .bind(asset.4)
        .bind(asset.5)
        .bind(asset.6)
        .bind(asset.7)
        .bind(asset.8)
        .bind(asset.9)
        .bind(asset.10)
        .execute(pool)
        .await?;
    }

    sqlx::query(
        r#"
        UPDATE infrastructure_assets a
        SET site_profile_id = s.id,
            project_id = s.project_id
        FROM site_profiles s
        WHERE a.site_profile_id IS NULL
          AND a.region = s.region
          AND a.department = s.department
          AND a.commune = s.commune
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        UPDATE field_reports r
        SET project_id = COALESCE(r.project_id, a.project_id),
            site_profile_id = COALESCE(r.site_profile_id, a.site_profile_id)
        FROM infrastructure_assets a
        WHERE r.asset_id = a.id
          AND (r.project_id IS NULL OR r.site_profile_id IS NULL)
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        UPDATE alerts al
        SET project_id = COALESCE(al.project_id, a.project_id),
            site_profile_id = COALESCE(al.site_profile_id, a.site_profile_id)
        FROM infrastructure_assets a
        WHERE al.asset_id = a.id
          AND (al.project_id IS NULL OR al.site_profile_id IS NULL)
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        UPDATE maintenance_tickets t
        SET project_id = COALESCE(t.project_id, a.project_id),
            site_profile_id = COALESCE(t.site_profile_id, a.site_profile_id),
            sla_hours = COALESCE(
                t.sla_hours,
                CASE t.priority
                    WHEN 'urgent' THEN 48
                    WHEN 'high' THEN 120
                    WHEN 'medium' THEN 240
                    ELSE 360
                END
            )
        FROM infrastructure_assets a
        WHERE t.asset_id = a.id
          AND (t.project_id IS NULL OR t.site_profile_id IS NULL OR t.sla_hours IS NULL)
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        UPDATE maintenance_tickets t
        SET project_id = COALESCE(t.project_id, al.project_id),
            site_profile_id = COALESCE(t.site_profile_id, al.site_profile_id)
        FROM alerts al
        WHERE t.alert_id = al.id
          AND (t.project_id IS NULL OR t.site_profile_id IS NULL)
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        UPDATE iot_readings r
        SET project_id = COALESCE(r.project_id, a.project_id),
            site_profile_id = COALESCE(r.site_profile_id, a.site_profile_id)
        FROM infrastructure_assets a
        WHERE r.asset_id = a.id
          AND (r.project_id IS NULL OR r.site_profile_id IS NULL)
        "#,
    )
    .execute(pool)
    .await?;

    let seeded_reports: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM field_reports")
        .fetch_one(pool)
        .await?;
    if seeded_reports.0 == 0 {
        sqlx::query(
            r#"
            INSERT INTO field_reports (
                project_id, site_profile_id, campaign_id, asset_id, report_type,
                region, department, commune, latitude, longitude, status,
                evidence_quality, notes, submitted_by
            )
            SELECT a.project_id, a.site_profile_id, c.id, a.id, 'inspection',
                   a.region, a.department, a.commune, a.latitude, a.longitude,
                   'needs_followup', 'agent_verified',
                   'Technician observed irregular flow and community queueing.',
                   'Demo field agent'
            FROM infrastructure_assets a
            LEFT JOIN survey_campaigns c ON c.target_commune = a.commune
            WHERE a.name = 'Moungo borehole cluster'
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO field_reports (
                project_id, site_profile_id, asset_id, report_type, region,
                department, commune, latitude, longitude, status, evidence_quality,
                notes, submitted_by
            )
            SELECT project_id, site_profile_id, id, 'signal_check', region,
                   department, commune, latitude, longitude, 'verified',
                   'gps_verified', 'Evening signal quality remains acceptable around the market.',
                   'Demo field agent'
            FROM infrastructure_assets
            WHERE name = 'Garoua market signal probe'
            "#,
        )
        .execute(pool)
        .await?;
    }

    let seeded_alerts: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM alerts")
        .fetch_one(pool)
        .await?;
    if seeded_alerts.0 == 0 {
        sqlx::query(
            r#"
            INSERT INTO alerts (project_id, site_profile_id, asset_id, severity, title, message, status)
            SELECT project_id, site_profile_id, id, 'critical', 'Pump telemetry offline',
                   'Ngaoundéré pump station missed recent IoT heartbeats.', 'open'
            FROM infrastructure_assets
            WHERE name = 'Ngaoundéré pump station'
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO alerts (project_id, site_profile_id, asset_id, severity, title, message, status)
            SELECT project_id, site_profile_id, id, 'warning', 'Water flow below baseline',
                   'Moungo borehole flow dropped below expected evening demand.', 'open'
            FROM infrastructure_assets
            WHERE name = 'Moungo borehole cluster'
            "#,
        )
        .execute(pool)
        .await?;
    }

    let seeded_tickets: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM maintenance_tickets")
        .fetch_one(pool)
        .await?;
    if seeded_tickets.0 == 0 {
        sqlx::query(
            r#"
            INSERT INTO maintenance_tickets (
                project_id, site_profile_id, asset_id, alert_id, title, priority,
                status, assigned_to, due_date, sla_hours
            )
            SELECT a.project_id, a.site_profile_id, a.asset_id, a.id,
                   'Dispatch technician to verify pump telemetry',
                   'urgent', 'open', 'North field team', CURRENT_DATE + INTERVAL '2 days', 48
            FROM alerts a
            WHERE a.title = 'Pump telemetry offline'
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO maintenance_tickets (
                project_id, site_profile_id, asset_id, alert_id, title, priority,
                status, assigned_to, due_date, sla_hours
            )
            SELECT a.project_id, a.site_profile_id, a.asset_id, a.id,
                   'Inspect borehole flow and evening demand pattern',
                   'high', 'scheduled', 'Littoral water unit', CURRENT_DATE + INTERVAL '5 days', 120
            FROM alerts a
            WHERE a.title = 'Water flow below baseline'
            "#,
        )
        .execute(pool)
        .await?;
    }

    let seeded_readings: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM iot_readings")
        .fetch_one(pool)
        .await?;
    if seeded_readings.0 == 0 {
        let readings = vec![
            ("Moungo borehole cluster", "flow_rate", 11.8, "L/min"),
            ("Buea clinic solar backup", "battery_level", 82.0, "%"),
            (
                "Garoua market signal probe",
                "signal_strength",
                -78.0,
                "dBm",
            ),
            ("Ngaoundéré pump station", "heartbeat_age", 18.0, "hours"),
        ];

        for reading in readings {
            sqlx::query(
                r#"
                INSERT INTO iot_readings (
                    project_id, site_profile_id, asset_id, reading_type, value, unit, latitude, longitude
                )
                SELECT project_id, site_profile_id, id, $2, $3, $4, latitude, longitude
                FROM infrastructure_assets
                WHERE name = $1
                "#,
            )
            .bind(reading.0)
            .bind(reading.1)
            .bind(reading.2)
            .bind(reading.3)
            .execute(pool)
            .await?;
        }
    }

    let seeded_decisions: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM decision_snapshots")
        .fetch_one(pool)
        .await?;
    if seeded_decisions.0 == 0 {
        sqlx::query(
            r#"
            INSERT INTO decision_snapshots (
                project_id, title, decision_stage, priority_score,
                recommended_budget_xaf, rationale, next_action
            )
            SELECT id, 'Approve Bare-Bakem validation sprint', 'recommended', 74.0,
                   1850000,
                   'Open water-flow alert, existing council pilot, and field-verifiable beneficiary cluster make this a low-friction trust-building deployment.',
                   'Run GPS/photo survey, validate evening queue pattern, then dispatch maintenance team.'
            FROM projects
            WHERE name = 'Water point reliability pilot'
            "#,
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub(crate) async fn seed_sample_data(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM mobile_phone_stats
        WHERE pcode IS NULL
          AND commune IN (
            'Yaoundé I',
            'Douala I',
            'Bafoussam',
            'Bamenda',
            'Buea',
            'Maroua',
            'Ngaoundéré I',
            'Bertoua',
            'Garoua I',
            'Ebolowa I'
          )
        "#,
    )
    .execute(pool)
    .await?;

    for row in seed_locations() {
        sqlx::query(
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
                data_source
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (pcode)
            DO UPDATE SET
                region = EXCLUDED.region,
                department = EXCLUDED.department,
                commune = EXCLUDED.commune,
                latitude = EXCLUDED.latitude,
                longitude = EXCLUDED.longitude,
                area_sqkm = EXCLUDED.area_sqkm,
                data_source = EXCLUDED.data_source,
                updated_at = NOW()
            "#,
        )
        .bind(&row.pcode)
        .bind(&row.region)
        .bind(&row.department)
        .bind(&row.commune)
        .bind(&row.commune)
        .bind(row.latitude)
        .bind(row.longitude)
        .bind(row.area_sqkm)
        .bind(&row.data_source)
        .execute(pool)
        .await?;
    }

    Ok(())
}

fn seed_locations() -> Vec<SeedLocation> {
    include_str!("../data/cameroon_admin3_seed.tsv")
        .lines()
        .skip(1)
        .filter_map(|line| {
            let columns: Vec<&str> = line.split('\t').collect();
            if columns.len() < 10 {
                return None;
            }

            Some(SeedLocation {
                region: columns[0].to_string(),
                department: columns[1].to_string(),
                commune: columns[2].to_string(),
                pcode: columns[3].to_string(),
                latitude: columns[4].parse().ok()?,
                longitude: columns[5].parse().ok()?,
                area_sqkm: columns[6].parse().ok(),
                data_source: columns[9].to_string(),
            })
        })
        .collect()
}
