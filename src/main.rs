use secrecy::ExposeSecret;
use sqlx::PgPool;
use std::io;
use std::net::TcpListener;
use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;
use zero2prod::telemetry;

#[tokio::main]
async fn main() -> io::Result<()> {
    let subscriber = telemetry::get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    telemetry::init_subscriber(subscriber);

    //

    let configuration = get_configuration().expect("Failed to read configuration");

    tracing::info!(application_host=%configuration.application.host, "got configuration");

    let pool = PgPool::connect(configuration.database.connection_string().expose_secret())
        .await
        .expect("Failed to connect to PostgreSQL");

    let listener = TcpListener::bind(&format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    ))?;
    run(listener, pool)?.await
}
