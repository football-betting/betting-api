use actix_web::{web, App, HttpServer};
use std::env;

mod db;
mod routes;
mod service;

const MAX_BODY_BYTES: usize = 16 * 1024;
const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8080";

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load .env ONCE at startup — never per DB connection (that hammered the
    // global env lock + file I/O and serialized the hot path under load).
    dotenvy::dotenv().ok();

    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string());

    HttpServer::new(|| {
        App::new()
            .app_data(web::PayloadConfig::new(MAX_BODY_BYTES))
            .app_data(web::JsonConfig::default().limit(MAX_BODY_BYTES))
            .service(routes::status)
            .service(routes::rating)
            .service(routes::user_by_id)
            .service(routes::get_past_result_by_game_id)
    })
    .bind(bind_addr)?
    .run()
    .await
}
