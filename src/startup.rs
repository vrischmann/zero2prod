use crate::configuration::Settings;
use crate::routes;
use crate::sessions::{CleanupConfig, PgSessionStore};
use crate::tem;
use actix_files::Files;
use actix_session::SessionMiddleware;
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use actix_web_flash_messages::storage::CookieMessageStore;
use actix_web_flash_messages::FlashMessagesFramework;
use secrecy::{ExposeSecret, Secret};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::io;
use std::net::TcpListener;
use std::time::Duration;
use tracing_actix_web::TracingLogger;

pub struct ApplicationBaseUrl(pub String);

#[derive(Clone)]
pub struct HmacSecret(pub Secret<String>);

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

        Application::build_with_pool(configuration, pool).await
    }

    pub async fn build_with_pool(configuration: Settings, pool: PgPool) -> Result<Self, io::Error> {
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

        let session_store = PgSessionStore::new(
            pool.clone(),
            CleanupConfig::new(
                configuration.session.cleanup_enabled,
                configuration.session.cleanup_interval(),
            ),
        );

        //

        let listener = TcpListener::bind(&format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        ))?;
        let port = listener.local_addr().unwrap().port();

        let server = run(
            listener,
            pool.clone(),
            tem_client,
            session_store,
            ApplicationBaseUrl(configuration.application.base_url),
            HmacSecret(configuration.application.hmac_secret),
            configuration.session.ttl(),
        )?;

        Ok(Self { port, pool, server })
    }

    pub async fn run_until_stopped(self) -> Result<(), anyhow::Error> {
        self.server.await?;
        Ok(())
    }
}

fn run(
    listener: TcpListener,
    pool: PgPool,
    email_client: tem::Client,
    session_store: PgSessionStore,
    base_url: ApplicationBaseUrl,
    hmac_secret: HmacSecret,
    session_ttl: time::Duration,
) -> Result<Server, io::Error> {
    let cookie_signing_key = actix_web::cookie::Key::from(hmac_secret.0.expose_secret().as_bytes());

    // Flash messages
    let flash_messages_store = CookieMessageStore::builder(cookie_signing_key.clone()).build();
    let flash_messages_framework = FlashMessagesFramework::builder(flash_messages_store).build();

    // Session store

    let pool = web::Data::new(pool);
    let email_client = web::Data::new(email_client);
    let base_url = web::Data::new(base_url);

    let server = HttpServer::new(move || {
        let session_middleware =
            SessionMiddleware::builder(session_store.clone(), cookie_signing_key.clone())
                .session_length(actix_session::SessionLength::BrowserSession {
                    state_ttl: Some(session_ttl),
                })
                .build();

        App::new()
            .wrap(flash_messages_framework.clone())
            .wrap(session_middleware)
            .wrap(TracingLogger::default())
            .service(Files::new("/static", "./static").prefer_utf8(true))
            .route("/health_check", web::get().to(routes::health_check))
            .route("/subscriptions", web::post().to(routes::subscribe))
            .route("/subscriptions/confirm", web::get().to(routes::confirm))
            .route("/newsletters", web::post().to(routes::publish_newsletter))
            .route("/", web::get().to(routes::home))
            .route("/login", web::get().to(routes::login_form))
            .route("/login", web::post().to(routes::login))
            .route("/admin/logout", web::post().to(routes::logout))
            .route("/admin/dashboard", web::get().to(routes::admin_dashboard))
            .route(
                "/admin/password",
                web::get().to(routes::admin_change_password_form),
            )
            .route(
                "/admin/password",
                web::post().to(routes::admin_change_password),
            )
            .app_data(pool.clone())
            .app_data(email_client.clone())
            .app_data(base_url.clone())
            .app_data(hmac_secret.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
