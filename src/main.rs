use actix_web::{App, HttpServer};

mod db;
mod routes;
mod service;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(routes::status)
            .service(routes::rating)
            .service(routes::user_by_id)
            .service(routes::get_past_result_by_game_id)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
