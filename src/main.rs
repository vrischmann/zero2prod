use sqlx::PgPool;
use std::io;
use std::net::TcpListener;
use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;

#[tokio::main]
async fn main() -> io::Result<()> {
    let configuration = get_configuration().expect("Failed to read configuration");
    let pool = PgPool::connect(&configuration.database.connection_string())
        .await
        .expect("Failed to connect to PostgreSQL");

    let listener = TcpListener::bind(&format!("127.0.0.1:{}", configuration.application_port))?;
    run(listener, pool)?.await
}
