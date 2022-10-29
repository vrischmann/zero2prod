use once_cell::sync::Lazy;
use sqlx::PgPool;
use wiremock::MockServer;
use zero2prod::configuration::get_configuration;
use zero2prod::startup::Application;
use zero2prod::telemetry;

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
    pub email_server: MockServer,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }
}

const TABLES: &[&str] = &["subscriptions"];

pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    //

    let email_server = MockServer::start().await;

    let mut configuration = get_configuration().expect("Failed to read configuration");
    configuration.application.port = 0;
    configuration.tem.base_url = email_server.uri();

    let app = Application::build(configuration)
        .await
        .expect("Failed to build application");

    let address = format!("http://127.0.0.1:{}", app.port);

    let pool = app.pool.clone();
    for table in TABLES {
        tracing::warn!(%table, "truncating table");

        sqlx::query(&format!("TRUNCATE {} CASCADE", table))
            .execute(&pool)
            .await
            .expect("Failed to truncate everything");
    }

    //

    let _ = tokio::spawn(app.run_until_stopped());

    TestApp {
        address,
        pool,
        email_server,
    }
}
