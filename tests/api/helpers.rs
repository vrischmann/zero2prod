use once_cell::sync::Lazy;
use secrecy::ExposeSecret;
use sqlx::PgPool;
use std::net::TcpListener;
use zero2prod::configuration::{get_configuration, DatabaseSettings};
use zero2prod::telemetry;
use zero2prod::tem;

static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".into();
    let subscriber_name = "test".into();

    if std::env::var("TEST_LOG").is_ok() {
        let subscriber =
            telemetry::get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        telemetry::init_subscriber(subscriber);
    } else {
        let subscriber =
            telemetry::get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        telemetry::init_subscriber(subscriber);
    }
});

pub struct TestApp {
    pub address: String,
    pub pool: PgPool,
}

const TABLES: &[&str] = &["subscriptions"];

async fn connect_pool(config: &DatabaseSettings) -> PgPool {
    let pool = PgPool::connect(config.connection_string().expose_secret())
        .await
        .expect("Failed to connect to PostgreSQL");

    for table in TABLES {
        sqlx::query(&format!("TRUNCATE {}", table))
            .execute(&pool)
            .await
            .expect("Failed to truncate everything");
    }

    pool
}

pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    //

    let configuration = get_configuration().expect("Failed to read configuration");
    let pool = connect_pool(&configuration.database).await;

    //

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let port = listener.local_addr().unwrap().port();

    //

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

    let server = zero2prod::startup::run(listener, pool.clone(), tem_client)
        .expect("Failed to bind address");
    let _ = tokio::spawn(server);

    TestApp {
        address: format!("http://127.0.0.1:{}", port),
        pool,
    }
}
