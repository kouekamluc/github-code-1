use actix_files::Files;
use actix_web::{get, patch, post, web, App, HttpResponse, HttpServer, Responder};
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::collections::{HashMap, HashSet};
use std::env;

#[derive(FromRow)]
struct DbLocation {
    pcode: Option<String>,
    region: String,
    department: String,
    commune: String,
    location: String,
    latitude: f64,
    longitude: f64,
    area_sqkm: Option<f64>,
    phone_owners: Option<i64>,
    population: Option<i64>,
    data_source: String,
}

#[derive(Serialize)]
struct LocationStat {
    pcode: Option<String>,
    region: String,
    department: String,
    commune: String,
    location: String,
    latitude: f64,
    longitude: f64,
    area_sqkm: Option<f64>,
    phone_owners: i64,
    population: i64,
    phone_rate: f64,
    metric_source: String,
    confidence: f64,
    urban_signal: f64,
    data_source: String,
}

#[derive(Serialize)]
struct ApiError {
    message: String,
}

#[derive(Serialize)]
struct Summary {
    total_phone_owners: i64,
    total_population: i64,
    percent_with_phone: f64,
    region_count: i64,
    department_count: i64,
    commune_count: i64,
    measured_location_count: i64,
    estimated_location_count: i64,
}

#[derive(Serialize, FromRow)]
struct InfrastructureAsset {
    id: i64,
    asset_type: String,
    name: String,
    region: String,
    department: String,
    commune: String,
    latitude: f64,
    longitude: f64,
    status: String,
    operator: Option<String>,
    installed_at: Option<String>,
    last_checked_at: Option<String>,
    notes: Option<String>,
}

#[derive(Deserialize)]
struct AssetRequest {
    asset_type: String,
    name: String,
    region: String,
    department: String,
    commune: String,
    latitude: f64,
    longitude: f64,
    status: String,
    operator: Option<String>,
    installed_at: Option<String>,
    notes: Option<String>,
}

#[derive(Serialize, FromRow)]
struct FieldReport {
    id: i64,
    asset_id: Option<i64>,
    report_type: String,
    region: String,
    department: String,
    commune: String,
    latitude: f64,
    longitude: f64,
    status: String,
    notes: String,
    submitted_by: String,
    created_at: String,
}

#[derive(Deserialize)]
struct FieldReportRequest {
    asset_id: Option<i64>,
    report_type: String,
    region: String,
    department: String,
    commune: String,
    latitude: f64,
    longitude: f64,
    status: String,
    notes: String,
    submitted_by: String,
}

#[derive(Serialize, FromRow)]
struct Alert {
    id: i64,
    asset_id: Option<i64>,
    severity: String,
    title: String,
    message: String,
    status: String,
    created_at: String,
    resolved_at: Option<String>,
}

#[derive(Deserialize)]
struct AlertRequest {
    asset_id: Option<i64>,
    severity: String,
    title: String,
    message: String,
}

#[derive(Deserialize)]
struct AlertStatusRequest {
    status: String,
}

#[derive(Serialize, FromRow)]
struct MaintenanceTicket {
    id: i64,
    asset_id: Option<i64>,
    alert_id: Option<i64>,
    title: String,
    priority: String,
    status: String,
    assigned_to: Option<String>,
    due_date: Option<String>,
    resolution_notes: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Deserialize)]
struct MaintenanceTicketRequest {
    asset_id: Option<i64>,
    alert_id: Option<i64>,
    title: String,
    priority: String,
    assigned_to: Option<String>,
    due_date: Option<String>,
}

#[derive(Deserialize)]
struct MaintenanceTicketStatusRequest {
    status: String,
    resolution_notes: Option<String>,
}

#[derive(Serialize, FromRow)]
struct IotReading {
    id: i64,
    asset_id: i64,
    reading_type: String,
    value: f64,
    unit: String,
    latitude: Option<f64>,
    longitude: Option<f64>,
    created_at: String,
}

#[derive(Deserialize)]
struct IotReadingRequest {
    asset_id: i64,
    reading_type: String,
    value: f64,
    unit: String,
    latitude: Option<f64>,
    longitude: Option<f64>,
}

#[derive(Serialize)]
struct PriorityZone {
    pcode: Option<String>,
    region: String,
    department: String,
    commune: String,
    latitude: f64,
    longitude: f64,
    population: i64,
    phone_rate: f64,
    confidence: f64,
    asset_count: i64,
    open_alert_count: i64,
    report_count: i64,
    priority_score: f64,
    priority_label: String,
}

#[derive(Serialize)]
struct DecisionReport {
    generated_for: String,
    summary: Summary,
    open_alerts: i64,
    monitored_assets: i64,
    field_reports: i64,
    top_priority_zones: Vec<PriorityZone>,
    recommendations: Vec<String>,
}

#[derive(Deserialize)]
struct UpdateLocationRequest {
    pcode: Option<String>,
    region: String,
    department: String,
    commune: String,
    location: String,
    latitude: f64,
    longitude: f64,
    area_sqkm: Option<f64>,
    phone_owners: Option<i64>,
    population: Option<i64>,
}

struct SeedLocation {
    region: String,
    department: String,
    commune: String,
    pcode: String,
    latitude: f64,
    longitude: f64,
    area_sqkm: Option<f64>,
    data_source: String,
}

const CAMEROON_MIN_LATITUDE: f64 = 1.5;
const CAMEROON_MAX_LATITUDE: f64 = 13.5;
const CAMEROON_MIN_LONGITUDE: f64 = 8.0;
const CAMEROON_MAX_LONGITUDE: f64 = 16.5;
const CAMEROON_2025_POPULATION: i64 = 29_879_337;
const CAMEROON_2024_MOBILE_SUBSCRIPTIONS_PER_100: f64 = 108.21313;
const MODEL_SOURCE: &str = "Matrix estimate: OCHA COD-AB GPS/area + UN 2025 population + World Bank 2024 mobile subscriptions";

struct UrbanAnchor {
    latitude: f64,
    longitude: f64,
    influence: f64,
}

const URBAN_ANCHORS: &[UrbanAnchor] = &[
    UrbanAnchor {
        latitude: 4.0511,
        longitude: 9.7679,
        influence: 1.35,
    },
    UrbanAnchor {
        latitude: 3.8480,
        longitude: 11.5021,
        influence: 1.30,
    },
    UrbanAnchor {
        latitude: 9.3014,
        longitude: 13.3977,
        influence: 0.82,
    },
    UrbanAnchor {
        latitude: 5.9631,
        longitude: 10.1594,
        influence: 0.80,
    },
    UrbanAnchor {
        latitude: 5.4839,
        longitude: 10.4170,
        influence: 0.78,
    },
    UrbanAnchor {
        latitude: 10.5950,
        longitude: 14.3247,
        influence: 0.74,
    },
    UrbanAnchor {
        latitude: 7.3277,
        longitude: 13.5847,
        influence: 0.70,
    },
    UrbanAnchor {
        latitude: 4.5759,
        longitude: 13.6846,
        influence: 0.62,
    },
    UrbanAnchor {
        latitude: 4.1575,
        longitude: 9.2407,
        influence: 0.66,
    },
    UrbanAnchor {
        latitude: 2.9167,
        longitude: 11.1500,
        influence: 0.55,
    },
];

impl UpdateLocationRequest {
    fn validate(&self) -> Result<(), String> {
        if self.region.trim().is_empty()
            || self.department.trim().is_empty()
            || self.commune.trim().is_empty()
            || self.location.trim().is_empty()
        {
            return Err("Region, department, commune, and location are required.".into());
        }

        if !self.latitude.is_finite() || !self.longitude.is_finite() {
            return Err("Latitude and longitude must be valid GPS coordinates.".into());
        }

        if !(CAMEROON_MIN_LATITUDE..=CAMEROON_MAX_LATITUDE).contains(&self.latitude)
            || !(CAMEROON_MIN_LONGITUDE..=CAMEROON_MAX_LONGITUDE).contains(&self.longitude)
        {
            return Err("GPS coordinates must be inside Cameroon.".into());
        }

        if matches!(self.area_sqkm, Some(area) if !area.is_finite() || area < 0.0) {
            return Err("Area must be a valid non-negative number.".into());
        }

        if matches!(self.phone_owners, Some(phone_owners) if phone_owners < 0)
            || matches!(self.population, Some(population) if population < 0)
        {
            return Err("Phone owners and population cannot be negative.".into());
        }

        match (self.phone_owners, self.population) {
            (Some(phone_owners), Some(population)) if phone_owners > population => {
                return Err("Phone owners cannot be greater than population.".into());
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err("Phone owners and population must be provided together.".into());
            }
            _ => {}
        }

        Ok(())
    }
}

async fn ensure_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS mobile_phone_stats (
            id SERIAL PRIMARY KEY,
            pcode TEXT,
            region TEXT NOT NULL,
            department TEXT NOT NULL,
            commune TEXT NOT NULL,
            location TEXT NOT NULL,
            latitude DOUBLE PRECISION NOT NULL,
            longitude DOUBLE PRECISION NOT NULL,
            area_sqkm DOUBLE PRECISION,
            phone_owners BIGINT,
            population BIGINT,
            data_source TEXT NOT NULL DEFAULT 'Manual entry',
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE(region, department, commune),
            UNIQUE(pcode)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        ALTER TABLE mobile_phone_stats
        ADD COLUMN IF NOT EXISTS pcode TEXT,
        ADD COLUMN IF NOT EXISTS area_sqkm DOUBLE PRECISION,
        ADD COLUMN IF NOT EXISTS data_source TEXT NOT NULL DEFAULT 'Manual entry',
        ALTER COLUMN phone_owners TYPE BIGINT,
        ALTER COLUMN population TYPE BIGINT,
        ALTER COLUMN phone_owners DROP NOT NULL,
        ALTER COLUMN population DROP NOT NULL
        "#,
    )
    .execute(pool)
    .await?;

    let operational_schema = [
        r#"
        CREATE TABLE IF NOT EXISTS infrastructure_assets (
            id BIGSERIAL PRIMARY KEY,
            asset_type TEXT NOT NULL,
            name TEXT NOT NULL,
            region TEXT NOT NULL,
            department TEXT NOT NULL,
            commune TEXT NOT NULL,
            latitude DOUBLE PRECISION NOT NULL,
            longitude DOUBLE PRECISION NOT NULL,
            status TEXT NOT NULL,
            operator TEXT,
            installed_at DATE,
            last_checked_at TIMESTAMPTZ,
            notes TEXT,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE(name, commune)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS field_reports (
            id BIGSERIAL PRIMARY KEY,
            asset_id BIGINT REFERENCES infrastructure_assets(id) ON DELETE SET NULL,
            report_type TEXT NOT NULL,
            region TEXT NOT NULL,
            department TEXT NOT NULL,
            commune TEXT NOT NULL,
            latitude DOUBLE PRECISION NOT NULL,
            longitude DOUBLE PRECISION NOT NULL,
            status TEXT NOT NULL,
            notes TEXT NOT NULL,
            submitted_by TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS alerts (
            id BIGSERIAL PRIMARY KEY,
            asset_id BIGINT REFERENCES infrastructure_assets(id) ON DELETE SET NULL,
            severity TEXT NOT NULL,
            title TEXT NOT NULL,
            message TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'open',
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            resolved_at TIMESTAMPTZ
        )
        "#,
        r#"
        DO $$
        BEGIN
            IF to_regclass('maintenance_tickets') IS NULL
               AND to_regclass('maintenance_tickets_id_seq') IS NOT NULL THEN
                DROP SEQUENCE maintenance_tickets_id_seq;
            END IF;
        END $$;
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS maintenance_tickets (
            id BIGSERIAL PRIMARY KEY,
            asset_id BIGINT REFERENCES infrastructure_assets(id) ON DELETE SET NULL,
            alert_id BIGINT REFERENCES alerts(id) ON DELETE SET NULL,
            title TEXT NOT NULL,
            priority TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'open',
            assigned_to TEXT,
            due_date DATE,
            resolution_notes TEXT,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS iot_readings (
            id BIGSERIAL PRIMARY KEY,
            asset_id BIGINT NOT NULL REFERENCES infrastructure_assets(id) ON DELETE CASCADE,
            reading_type TEXT NOT NULL,
            value DOUBLE PRECISION NOT NULL,
            unit TEXT NOT NULL,
            latitude DOUBLE PRECISION,
            longitude DOUBLE PRECISION,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    ];

    for statement in operational_schema {
        sqlx::query(statement).execute(pool).await?;
    }

    sqlx::query(
        r#"
        DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1 FROM pg_constraint WHERE conname = 'mobile_phone_stats_pcode_unique'
            ) THEN
                ALTER TABLE mobile_phone_stats
                ADD CONSTRAINT mobile_phone_stats_pcode_unique
                UNIQUE (pcode);
            END IF;

            IF NOT EXISTS (
                SELECT 1 FROM pg_constraint WHERE conname = 'mobile_phone_stats_gps_bounds'
            ) THEN
                ALTER TABLE mobile_phone_stats
                ADD CONSTRAINT mobile_phone_stats_gps_bounds
                CHECK (
                    latitude BETWEEN 1.5 AND 13.5
                    AND longitude BETWEEN 8.0 AND 16.5
                );
            END IF;

            IF NOT EXISTS (
                SELECT 1 FROM pg_constraint WHERE conname = 'mobile_phone_stats_non_negative_counts'
            ) THEN
                ALTER TABLE mobile_phone_stats
                ADD CONSTRAINT mobile_phone_stats_non_negative_counts
                CHECK (
                    (phone_owners IS NULL OR phone_owners >= 0)
                    AND (population IS NULL OR population >= 0)
                    AND (
                        phone_owners IS NULL
                        OR population IS NULL
                        OR phone_owners <= population
                    )
                );
            END IF;

            IF NOT EXISTS (
                SELECT 1 FROM pg_constraint WHERE conname = 'mobile_phone_stats_area_bounds'
            ) THEN
                ALTER TABLE mobile_phone_stats
                ADD CONSTRAINT mobile_phone_stats_area_bounds
                CHECK (area_sqkm IS NULL OR area_sqkm >= 0);
            END IF;
        END $$;
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn seed_operational_demo(pool: &PgPool) -> Result<(), sqlx::Error> {
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

    let seeded_reports: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM field_reports")
        .fetch_one(pool)
        .await?;
    if seeded_reports.0 == 0 {
        sqlx::query(
            r#"
            INSERT INTO field_reports (
                asset_id, report_type, region, department, commune, latitude, longitude,
                status, notes, submitted_by
            )
            SELECT id, 'inspection', region, department, commune, latitude, longitude,
                   'needs_followup', 'Technician observed irregular flow and community queueing.',
                   'Demo field agent'
            FROM infrastructure_assets
            WHERE name = 'Moungo borehole cluster'
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO field_reports (
                asset_id, report_type, region, department, commune, latitude, longitude,
                status, notes, submitted_by
            )
            SELECT id, 'signal_check', region, department, commune, latitude, longitude,
                   'verified', 'Evening signal quality remains acceptable around the market.',
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
            INSERT INTO alerts (asset_id, severity, title, message, status)
            SELECT id, 'critical', 'Pump telemetry offline',
                   'Ngaoundéré pump station missed recent IoT heartbeats.', 'open'
            FROM infrastructure_assets
            WHERE name = 'Ngaoundéré pump station'
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO alerts (asset_id, severity, title, message, status)
            SELECT id, 'warning', 'Water flow below baseline',
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
                asset_id, alert_id, title, priority, status, assigned_to, due_date
            )
            SELECT a.asset_id, a.id, 'Dispatch technician to verify pump telemetry',
                   'urgent', 'open', 'North field team', CURRENT_DATE + INTERVAL '2 days'
            FROM alerts a
            WHERE a.title = 'Pump telemetry offline'
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO maintenance_tickets (
                asset_id, alert_id, title, priority, status, assigned_to, due_date
            )
            SELECT a.asset_id, a.id, 'Inspect borehole flow and evening demand pattern',
                   'high', 'scheduled', 'Littoral water unit', CURRENT_DATE + INTERVAL '5 days'
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
            ("Garoua market signal probe", "signal_strength", -78.0, "dBm"),
            ("Ngaoundéré pump station", "heartbeat_age", 18.0, "hours"),
        ];

        for reading in readings {
            sqlx::query(
                r#"
                INSERT INTO iot_readings (
                    asset_id, reading_type, value, unit, latitude, longitude
                )
                SELECT id, $2, $3, $4, latitude, longitude
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

    Ok(())
}

async fn seed_sample_data(pool: &PgPool) -> Result<(), sqlx::Error> {
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
        + a_lat.to_radians().cos()
            * b_lat.to_radians().cos()
            * (d_lon / 2.0).sin().powi(2);
    2.0 * radius_km * a.sqrt().asin()
}

fn urban_signal(row: &DbLocation) -> f64 {
    let signal = URBAN_ANCHORS
        .iter()
        .map(|anchor| {
            let distance = haversine_km(row.latitude, row.longitude, anchor.latitude, anchor.longitude);
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

    fractional_allocations.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
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

fn validate_gps(latitude: f64, longitude: f64) -> Result<(), String> {
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

fn priority_label(score: f64) -> String {
    if score >= 52.0 {
        "High".into()
    } else if score >= 38.0 {
        "Medium".into()
    } else {
        "Watch".into()
    }
}

async fn fetch_location_stats(pool: &PgPool) -> Result<Vec<LocationStat>, sqlx::Error> {
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

async fn fetch_summary(pool: &PgPool) -> Result<Summary, sqlx::Error> {
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

async fn fetch_assets(pool: &PgPool) -> Result<Vec<InfrastructureAsset>, sqlx::Error> {
    sqlx::query_as::<_, InfrastructureAsset>(
        r#"
        SELECT
            id,
            asset_type,
            name,
            region,
            department,
            commune,
            latitude,
            longitude,
            status,
            operator,
            installed_at::TEXT AS installed_at,
            last_checked_at::TEXT AS last_checked_at,
            notes
        FROM infrastructure_assets
        ORDER BY status DESC, region, department, commune, name
        "#,
    )
    .fetch_all(pool)
    .await
}

async fn fetch_reports(pool: &PgPool) -> Result<Vec<FieldReport>, sqlx::Error> {
    sqlx::query_as::<_, FieldReport>(
        r#"
        SELECT
            id,
            asset_id,
            report_type,
            region,
            department,
            commune,
            latitude,
            longitude,
            status,
            notes,
            submitted_by,
            created_at::TEXT AS created_at
        FROM field_reports
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

async fn fetch_alerts(pool: &PgPool) -> Result<Vec<Alert>, sqlx::Error> {
    sqlx::query_as::<_, Alert>(
        r#"
        SELECT
            id,
            asset_id,
            severity,
            title,
            message,
            status,
            created_at::TEXT AS created_at,
            resolved_at::TEXT AS resolved_at
        FROM alerts
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

async fn fetch_tickets(pool: &PgPool) -> Result<Vec<MaintenanceTicket>, sqlx::Error> {
    sqlx::query_as::<_, MaintenanceTicket>(
        r#"
        SELECT
            id,
            asset_id,
            alert_id,
            title,
            priority,
            status,
            assigned_to,
            due_date::TEXT AS due_date,
            resolution_notes,
            created_at::TEXT AS created_at,
            updated_at::TEXT AS updated_at
        FROM maintenance_tickets
        ORDER BY
            CASE status
                WHEN 'open' THEN 1
                WHEN 'scheduled' THEN 2
                WHEN 'in_progress' THEN 3
                WHEN 'blocked' THEN 4
                ELSE 5
            END,
            CASE priority
                WHEN 'urgent' THEN 1
                WHEN 'high' THEN 2
                WHEN 'medium' THEN 3
                ELSE 4
            END,
            due_date ASC NULLS LAST,
            created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

async fn fetch_iot_readings(pool: &PgPool) -> Result<Vec<IotReading>, sqlx::Error> {
    sqlx::query_as::<_, IotReading>(
        r#"
        SELECT
            id,
            asset_id,
            reading_type,
            value,
            unit,
            latitude,
            longitude,
            created_at::TEXT AS created_at
        FROM iot_readings
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

async fn build_priority_zones(pool: &PgPool) -> Result<Vec<PriorityZone>, sqlx::Error> {
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
            let key = (row.region.clone(), row.department.clone(), row.commune.clone());
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

#[get("/api/summary")]
async fn summary(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_summary(pool.get_ref()).await {
        Ok(summary) => HttpResponse::Ok().json(summary),
        Err(err) => {
            eprintln!("Failed to query summary: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/stats")]
async fn list_stats(pool: web::Data<PgPool>) -> impl Responder {
    let stats = fetch_location_stats(pool.get_ref()).await;

    match stats {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(err) => {
            eprintln!("Failed to query stats: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/stats/update")]
async fn update_stats(
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

#[get("/api/assets")]
async fn list_assets(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_assets(pool.get_ref()).await {
        Ok(assets) => HttpResponse::Ok().json(assets),
        Err(err) => {
            eprintln!("Failed to query assets: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/assets")]
async fn create_asset(
    pool: web::Data<PgPool>,
    payload: web::Json<AssetRequest>,
) -> impl Responder {
    if let Err(message) = validate_gps(payload.latitude, payload.longitude) {
        return HttpResponse::BadRequest().json(ApiError { message });
    }

    let result = sqlx::query(
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

#[get("/api/reports")]
async fn list_reports(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_reports(pool.get_ref()).await {
        Ok(reports) => HttpResponse::Ok().json(reports),
        Err(err) => {
            eprintln!("Failed to query reports: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/reports")]
async fn create_report(
    pool: web::Data<PgPool>,
    payload: web::Json<FieldReportRequest>,
) -> impl Responder {
    if let Err(message) = validate_gps(payload.latitude, payload.longitude) {
        return HttpResponse::BadRequest().json(ApiError { message });
    }

    let result = sqlx::query(
        r#"
        INSERT INTO field_reports (
            asset_id, report_type, region, department, commune, latitude, longitude,
            status, notes, submitted_by
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(payload.asset_id)
    .bind(&payload.report_type)
    .bind(&payload.region)
    .bind(&payload.department)
    .bind(&payload.commune)
    .bind(payload.latitude)
    .bind(payload.longitude)
    .bind(&payload.status)
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
async fn list_alerts(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_alerts(pool.get_ref()).await {
        Ok(alerts) => HttpResponse::Ok().json(alerts),
        Err(err) => {
            eprintln!("Failed to query alerts: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/alerts")]
async fn create_alert(
    pool: web::Data<PgPool>,
    payload: web::Json<AlertRequest>,
) -> impl Responder {
    let result = sqlx::query(
        r#"
        INSERT INTO alerts (asset_id, severity, title, message, status)
        VALUES ($1, $2, $3, $4, 'open')
        "#,
    )
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
async fn update_alert_status(
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
async fn list_tickets(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_tickets(pool.get_ref()).await {
        Ok(tickets) => HttpResponse::Ok().json(tickets),
        Err(err) => {
            eprintln!("Failed to query tickets: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/tickets")]
async fn create_ticket(
    pool: web::Data<PgPool>,
    payload: web::Json<MaintenanceTicketRequest>,
) -> impl Responder {
    let result = sqlx::query(
        r#"
        INSERT INTO maintenance_tickets (
            asset_id, alert_id, title, priority, status, assigned_to, due_date
        ) VALUES ($1, $2, $3, $4, 'open', $5, $6::DATE)
        "#,
    )
    .bind(payload.asset_id)
    .bind(payload.alert_id)
    .bind(&payload.title)
    .bind(&payload.priority)
    .bind(&payload.assigned_to)
    .bind(&payload.due_date)
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
async fn update_ticket_status(
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
async fn list_iot_readings(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_iot_readings(pool.get_ref()).await {
        Ok(readings) => HttpResponse::Ok().json(readings),
        Err(err) => {
            eprintln!("Failed to query IoT readings: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/api/iot/readings")]
async fn create_iot_reading(
    pool: web::Data<PgPool>,
    payload: web::Json<IotReadingRequest>,
) -> impl Responder {
    let result = sqlx::query(
        r#"
        INSERT INTO iot_readings (
            asset_id, reading_type, value, unit, latitude, longitude
        ) VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
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
async fn priority_zones(pool: web::Data<PgPool>) -> impl Responder {
    match build_priority_zones(pool.get_ref()).await {
        Ok(zones) => HttpResponse::Ok().json(zones),
        Err(err) => {
            eprintln!("Failed to build priority zones: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/api/decision-report")]
async fn decision_report(pool: web::Data<PgPool>) -> impl Responder {
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
    let open_alerts = alerts.iter().filter(|alert| alert.status != "resolved").count() as i64;

    HttpResponse::Ok().json(DecisionReport {
        generated_for: "InfraPulse Cameroon MVP".into(),
        summary: report_summary,
        open_alerts,
        monitored_assets: assets.len() as i64,
        field_reports: reports.len() as i64,
        top_priority_zones: report_priority_zones,
        recommendations: vec![
            "Start with monitored water and solar assets in high-priority arrondissements.".into(),
            "Use field reports to validate matrix estimates before hardware deployment.".into(),
            "Convert repeated alerts into maintenance tickets with technician assignments.".into(),
            "Package monthly council/NGO reports around uptime, response time, and beneficiary reach.".into(),
        ],
    })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1/cameroon_stats".into());
    let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let server_port = env::var("SERVER_PORT")
        .ok()
        .and_then(|port| port.parse::<u16>().ok())
        .unwrap_or(8081);

    let pool = PgPool::connect(&database_url)
        .await
        .map_err(|err| {
            eprintln!("Unable to connect to database: {}", err);
            std::io::Error::new(std::io::ErrorKind::Other, err)
        })?;

    ensure_schema(&pool).await.map_err(|err| {
        eprintln!("Schema creation error: {}", err);
        std::io::Error::new(std::io::ErrorKind::Other, err)
    })?;

    seed_sample_data(&pool).await.map_err(|err| {
        eprintln!("Failed to seed sample data: {}", err);
        std::io::Error::new(std::io::ErrorKind::Other, err)
    })?;

    seed_operational_demo(&pool).await.map_err(|err| {
        eprintln!("Failed to seed operational demo data: {}", err);
        std::io::Error::new(std::io::ErrorKind::Other, err)
    })?;

    println!(
        "Starting Cameroon phone monitor at http://{}:{}",
        server_host, server_port
    );

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .service(summary)
            .service(list_stats)
            .service(update_stats)
            .service(list_assets)
            .service(create_asset)
            .service(list_reports)
            .service(create_report)
            .service(list_alerts)
            .service(create_alert)
            .service(update_alert_status)
            .service(list_tickets)
            .service(create_ticket)
            .service(update_ticket_status)
            .service(list_iot_readings)
            .service(create_iot_reading)
            .service(priority_zones)
            .service(decision_report)
            .service(Files::new("/static", "static").show_files_listing())
            .default_service(web::get().to(|| async {
                HttpResponse::Found()
                    .append_header(("Location", "/static/index.html"))
                    .finish()
            }))
    })
    .bind((server_host, server_port))?
    .run()
    .await
}
