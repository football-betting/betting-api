use actix_web::{web, App, HttpServer};

mod db;
mod routes;
mod service;

const MAX_BODY_BYTES: usize = 16 * 1024;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .app_data(web::PayloadConfig::new(MAX_BODY_BYTES))
            .app_data(web::JsonConfig::default().limit(MAX_BODY_BYTES))
            .service(routes::status)
            .service(routes::rating)
            .service(routes::user_by_id)
            .service(routes::get_past_result_by_game_id)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
