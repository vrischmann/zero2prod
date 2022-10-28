use crate::configuration::Settings;
use crate::tem;
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::io;
use std::net::TcpListener;
use std::time::Duration;
use tracing_actix_web::TracingLogger;

pub struct Application {
    pub port: u16,
    pub pool: PgPool,
    server: Server,
}

impl Application {
    pub async fn build(configuration: Settings) -> Result<Self, io::Error> {
        let pool = PgPoolOptions::new()
            .acquire_timeout(Duration::from_secs(2))
            .connect(configuration.database.connection_string().expose_secret())
            .await
            .expect("Failed to connect to PostgreSQL");

        let sender_email = configuration
            .tem
            .sender()
            .expect("Invalid sender email address");

        let tem_client = tem::Client::new(
            configuration.tem.base_url.clone(),
            configuration.tem.project_id(),
            configuration.tem.auth_key.clone(),
            sender_email,
            configuration.tem.timeout(),
        );

        let listener = TcpListener::bind(&format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        ))?;
        let port = listener.local_addr().unwrap().port();

        let server = run(listener, pool.clone(), tem_client)?;

        Ok(Self { port, pool, server })
    }

    pub async fn run_until_stopped(self) -> Result<(), io::Error> {
        self.server.await
    }
}

fn run(
    listener: TcpListener,
    pool: PgPool,
    email_client: tem::Client,
) -> Result<Server, io::Error> {
    let pool = web::Data::new(pool);
    let email_client = web::Data::new(email_client);

    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(crate::routes::health_check))
            .route("/subscriptions", web::post().to(crate::routes::subscribe))
            .app_data(pool.clone())
            .app_data(email_client.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
