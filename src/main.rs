use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;
use std::io;
use std::net::TcpListener;
use std::time::Duration;
use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;
use zero2prod::telemetry;
use zero2prod::tem;

#[tokio::main]
async fn main() -> io::Result<()> {
    let subscriber = telemetry::get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    telemetry::init_subscriber(subscriber);

    //

    let configuration = get_configuration().expect("Failed to read configuration");

    tracing::info!(application_host=%configuration.application.host, "got configuration");

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
    );

    let listener = TcpListener::bind(&format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    ))?;
    run(listener, pool, tem_client)?.await
}
