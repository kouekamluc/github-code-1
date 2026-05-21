use actix_files::Files;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::env;

#[derive(Serialize, FromRow)]
struct LocationStat {
    region: String,
    department: String,
    commune: String,
    location: String,
    latitude: f64,
    longitude: f64,
    phone_owners: i64,
    population: i64,
    phone_rate: f64,
}

#[derive(Serialize)]
struct Summary {
    total_phone_owners: i64,
    total_population: i64,
    percent_with_phone: f64,
    region_count: i64,
    department_count: i64,
}

#[derive(Deserialize)]
struct UpdateLocationRequest {
    region: String,
    department: String,
    commune: String,
    location: String,
    latitude: f64,
    longitude: f64,
    phone_owners: i64,
    population: i64,
}

async fn ensure_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS mobile_phone_stats (
            id SERIAL PRIMARY KEY,
            region TEXT NOT NULL,
            department TEXT NOT NULL,
            commune TEXT NOT NULL,
            location TEXT NOT NULL,
            latitude DOUBLE PRECISION NOT NULL,
            longitude DOUBLE PRECISION NOT NULL,
            phone_owners INTEGER NOT NULL,
            population INTEGER NOT NULL,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE(region, department, commune)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn seed_sample_data(pool: &PgPool) -> Result<(), sqlx::Error> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM mobile_phone_stats")
        .fetch_one(pool)
        .await?;

    if row.0 == 0 {
        let sample = vec![
            (
                "Centre",
                "Mefou-et-Afamba",
                "Yaoundé I",
                "Yaoundé",
                3.8480,
                11.5021,
                720000,
                900000,
            ),
            (
                "Littoral",
                "Wouri",
                "Douala I",
                "Douala",
                4.0511,
                9.7679,
                1400000,
                1600000,
            ),
            (
                "West",
                "Mifi",
                "Bafoussam",
                "Bafoussam",
                5.4839,
                10.4170,
                600000,
                750000,
            ),
            (
                "North West",
                "Mezam",
                "Bamenda",
                "Bamenda",
                5.9631,
                10.1594,
                480000,
                600000,
            ),
            (
                "South West",
                "Fako",
                "Buea",
                "Buea",
                4.1575,
                9.2407,
                420000,
                520000,
            ),
            (
                "Far North",
                "Diamaré",
                "Maroua",
                "Maroua",
                10.5950,
                14.3247,
                340000,
                520000,
            ),
        ];

        for row in sample {
            sqlx::query(
                r#"
                INSERT INTO mobile_phone_stats (
                    region,
                    department,
                    commune,
                    location,
                    latitude,
                    longitude,
                    phone_owners,
                    population
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (region, department, commune) DO NOTHING
                "#,
            )
            .bind(row.0)
            .bind(row.1)
            .bind(row.2)
            .bind(row.3)
            .bind(row.4)
            .bind(row.5)
            .bind(row.6)
            .bind(row.7)
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}

#[get("/api/summary")]
async fn summary(pool: web::Data<PgPool>) -> impl Responder {
    let row = sqlx::query!(
        r#"
        SELECT
            COALESCE(SUM(phone_owners), 0) AS total_phone_owners,
            COALESCE(SUM(population), 0) AS total_population,
            COUNT(DISTINCT region) AS region_count,
            COUNT(DISTINCT department) AS department_count
        FROM mobile_phone_stats
        "#,
    )
    .fetch_one(pool.get_ref())
    .await;

    match row {
        Ok(record) => {
            let total_phone_owners = record.total_phone_owners.unwrap_or(0);
            let total_population = record.total_population.unwrap_or(0);
            let percent_with_phone = if total_population > 0 {
                (total_phone_owners as f64 / total_population as f64) * 100.0
            } else {
                0.0
            };

            HttpResponse::Ok().json(Summary {
                total_phone_owners,
                total_population,
                percent_with_phone,
                region_count: record.region_count.unwrap_or(0),
                department_count: record.department_count.unwrap_or(0),
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
    let stats = sqlx::query_as::<_, LocationStat>(
        r#"
        SELECT
            region,
            department,
            commune,
            location,
            latitude,
            longitude,
            phone_owners,
            population,
            CASE
                WHEN population > 0 THEN (phone_owners::double precision / population::double precision) * 100.0
                ELSE 0.0
            END AS phone_rate
        FROM mobile_phone_stats
        ORDER BY region, department, commune
        "#,
    )
    .fetch_all(pool.get_ref())
    .await;

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
    let result = sqlx::query(
        r#"
        INSERT INTO mobile_phone_stats (
            region,
            department,
            commune,
            location,
            latitude,
            longitude,
            phone_owners,
            population
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (region, department, commune)
        DO UPDATE SET
            location = EXCLUDED.location,
            latitude = EXCLUDED.latitude,
            longitude = EXCLUDED.longitude,
            phone_owners = EXCLUDED.phone_owners,
            population = EXCLUDED.population,
            updated_at = NOW()
        "#,
    )
    .bind(&payload.region)
    .bind(&payload.department)
    .bind(&payload.commune)
    .bind(&payload.location)
    .bind(payload.latitude)
    .bind(payload.longitude)
    .bind(payload.phone_owners)
    .bind(payload.population)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => {
            let stats = sqlx::query_as::<_, LocationStat>(
                r#"
                SELECT
                    region,
                    department,
                    commune,
                    location,
                    latitude,
                    longitude,
                    phone_owners,
                    population,
                    CASE
                        WHEN population > 0 THEN (phone_owners::double precision / population::double precision) * 100.0
                        ELSE 0.0
                    END AS phone_rate
                FROM mobile_phone_stats
                ORDER BY region, department, commune
                "#,
            )
            .fetch_all(pool.get_ref())
            .await;

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

    println!("Starting Cameroon phone monitor at http://127.0.0.1:8080");

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
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
