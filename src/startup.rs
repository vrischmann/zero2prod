use actix_web::dev::Server;
use actix_web::middleware::Logger;
use actix_web::{web, App, HttpServer};
use sqlx::PgPool;
use std::io;
use std::net::TcpListener;

pub fn run(listener: TcpListener, pool: PgPool) -> Result<Server, io::Error> {
    let pool = web::Data::new(pool);

    let server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .route("/health_check", web::get().to(crate::routes::health_check))
            .route("/subscriptions", web::post().to(crate::routes::subscribe))
            .app_data(pool.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
