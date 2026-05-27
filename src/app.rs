use actix_web::{web, App, HttpServer};
use dotenvy::dotenv;
use sqlx::PgPool;
use std::env;

use crate::db::ensure_schema;
use crate::routes;
use crate::seeds::{seed_operational_demo, seed_root_user, seed_sample_data};

pub(crate) async fn run() -> std::io::Result<()> {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1/cameroon_stats".into());
    let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let server_port = env::var("SERVER_PORT")
        .ok()
        .and_then(|port| port.parse::<u16>().ok())
        .unwrap_or(8081);

    let pool = PgPool::connect(&database_url).await.map_err(|err| {
        eprintln!("Unable to connect to database: {}", err);
        std::io::Error::other(err)
    })?;

    ensure_schema(&pool).await.map_err(|err| {
        eprintln!("Schema creation error: {}", err);
        std::io::Error::other(err)
    })?;

    seed_sample_data(&pool).await.map_err(|err| {
        eprintln!("Failed to seed sample data: {}", err);
        std::io::Error::other(err)
    })?;

    seed_root_user(&pool).await.map_err(|err| {
        eprintln!("Failed to seed root user: {}", err);
        std::io::Error::other(err)
    })?;

    seed_operational_demo(&pool).await.map_err(|err| {
        eprintln!("Failed to seed operational demo data: {}", err);
        std::io::Error::other(err)
    })?;

    println!(
        "Starting Cameroon phone monitor at http://{}:{}",
        server_host, server_port
    );

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .configure(routes::configure)
    })
    .bind((server_host, server_port))?
    .run()
    .await
}
