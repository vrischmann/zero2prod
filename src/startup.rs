use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use std::io;
use std::net::TcpListener;

pub fn run(listener: TcpListener) -> Result<Server, io::Error> {
    let server = HttpServer::new(|| {
        App::new()
            .route("/health_check", web::get().to(crate::routes::health_check))
            .route("/subscriptions", web::post().to(crate::routes::subscribe))
    })
    .listen(listener)?
    .run();

    Ok(server)
}
