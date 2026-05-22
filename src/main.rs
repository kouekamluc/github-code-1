mod app;
mod db;
mod handlers;
mod models;
mod routes;
mod seeds;
mod services;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    app::run().await
}
