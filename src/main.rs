use actix_files::Files;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
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

#[get("/api/summary")]
async fn summary(pool: web::Data<PgPool>) -> impl Responder {
    match fetch_location_stats(pool.get_ref()).await {
        Ok(stats) => {
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
                .collect::<std::collections::HashSet<_>>()
                .len() as i64;
            let department_count = stats
                .iter()
                .map(|row| &row.department)
                .collect::<std::collections::HashSet<_>>()
                .len() as i64;
            let commune_count = stats
                .iter()
                .map(|row| &row.commune)
                .collect::<std::collections::HashSet<_>>()
                .len() as i64;
            let measured_location_count = stats
                .iter()
                .filter(|row| row.metric_source == "Measured local update")
                .count() as i64;
            HttpResponse::Ok().json(Summary {
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
