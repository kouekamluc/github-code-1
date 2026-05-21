use actix_files::Files;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::env;

#[derive(Serialize, FromRow)]
struct LocationStat {
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
    phone_rate: Option<f64>,
    data_source: String,
}

#[derive(Serialize)]
struct ApiError {
    message: String,
}

#[derive(Serialize)]
struct Summary {
    total_phone_owners: Option<i64>,
    total_population: Option<i64>,
    percent_with_phone: Option<f64>,
    region_count: i64,
    department_count: i64,
    commune_count: i64,
    phone_data_count: i64,
}

#[derive(FromRow)]
struct SummaryRow {
    total_phone_owners: Option<i64>,
    total_population: Option<i64>,
    region_count: i64,
    department_count: i64,
    commune_count: i64,
    phone_data_count: i64,
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

async fn fetch_location_stats(pool: &PgPool) -> Result<Vec<LocationStat>, sqlx::Error> {
    sqlx::query_as::<_, LocationStat>(
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
            CASE
                WHEN phone_owners IS NOT NULL AND population > 0
                THEN (phone_owners::double precision / population::double precision) * 100.0
                ELSE NULL
            END AS phone_rate,
            data_source
        FROM mobile_phone_stats
        ORDER BY region, department, commune
        "#,
    )
    .fetch_all(pool)
    .await
}

#[get("/api/summary")]
async fn summary(pool: web::Data<PgPool>) -> impl Responder {
    let row = sqlx::query_as::<_, SummaryRow>(
        r#"
        SELECT
            SUM(phone_owners)::BIGINT AS total_phone_owners,
            SUM(population)::BIGINT AS total_population,
            COUNT(DISTINCT region) AS region_count,
            COUNT(DISTINCT department) AS department_count,
            COUNT(DISTINCT commune) AS commune_count,
            COUNT(phone_owners) AS phone_data_count
        FROM mobile_phone_stats
        "#,
    )
    .fetch_one(pool.get_ref())
    .await;

    match row {
        Ok(record) => {
            let total_phone_owners = record.total_phone_owners;
            let total_population = record.total_population;
            let percent_with_phone = match (total_phone_owners, total_population) {
                (Some(total_phone_owners), Some(total_population)) if total_population > 0 => {
                    Some((total_phone_owners as f64 / total_population as f64) * 100.0)
                }
                _ => None,
            };

            HttpResponse::Ok().json(Summary {
                total_phone_owners,
                total_population,
                percent_with_phone,
                region_count: record.region_count,
                department_count: record.department_count,
                commune_count: record.commune_count,
                phone_data_count: record.phone_data_count,
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
