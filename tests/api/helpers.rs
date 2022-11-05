use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use once_cell::sync::Lazy;
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::MockServer;
use zero2prod::configuration::get_configuration;
use zero2prod::startup::Application;
use zero2prod::telemetry;

static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".into();
    let subscriber_name = "test".into();

    std::env::set_var("RUST_LOG", "sqlx=error,info");

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

pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub text: reqwest::Url,
}

pub struct TestApp {
    pub address: String,
    pub port: u16,
    pub pool: PgPool,
    pub email_server: MockServer,

    pub test_user: TestUser,
}

impl TestApp {
    pub async fn post_subscriptions<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        reqwest::Client::new()
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_newsletters(&self, body: serde_json::Value) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/newsletters", &self.address))
            .basic_auth(&self.test_user.username, Some(&self.test_user.password))
            .json(&body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

        let get_link = |s: &str| -> reqwest::Url {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();

            assert_eq!(links.len(), 1, "expected 1 link, got {}", links.len());

            let raw_link = links[0].as_str();

            let mut confirmation_link = reqwest::Url::parse(raw_link).unwrap();
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
            confirmation_link.set_port(Some(self.port)).unwrap();

            confirmation_link
        };

        let html_link = get_link(body["html"].as_str().unwrap());
        let text_link = get_link(body["text"].as_str().unwrap());

        ConfirmationLinks {
            html: html_link,
            text: text_link,
        }
    }
}

pub async fn spawn_app(pool: sqlx::PgPool) -> TestApp {
    Lazy::force(&TRACING);

    //

    let email_server = MockServer::start().await;

    let mut configuration = get_configuration().expect("Failed to read configuration");
    configuration.application.port = 0;
    configuration.tem.base_url = email_server.uri();

    //

    let app = Application::build_with_pool(configuration, pool)
        .await
        .expect("Failed to build application");
    let app_port = app.port;

    let pool = app.pool.clone();

    let _ = tokio::spawn(app.run_until_stopped());

    //

    let test_app = TestApp {
        address: format!("http://127.0.0.1:{}", app_port),
        port: app_port,
        pool,
        email_server,
        test_user: TestUser::generate(),
    };

    test_app.test_user.store(&test_app.pool).await;

    test_app
}

pub struct TestUser {
    pub user_id: Uuid,
    pub username: String,
    pub password: String,
}

impl TestUser {
    fn generate() -> Self {
        Self {
            user_id: Uuid::new_v4(),
            username: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string(),
        }
    }

    async fn store(&self, pool: &sqlx::PgPool) {
        let salt = SaltString::generate(&mut rand::thread_rng());

        let hasher = Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            argon2::Params::new(15000, 2, 1, None).unwrap(),
        );

        let password_hash = hasher
            .hash_password(self.password.as_bytes(), &salt)
            .unwrap()
            .to_string();

        sqlx::query!(
            r#"
            INSERT INTO users(user_id, username, password_hash)
            VALUES ($1, $2, $3)
            "#,
            self.user_id,
            self.username,
            password_hash,
        )
        .execute(pool)
        .await
        .expect("Failed to create test users");
    }
}

#[derive(serde::Serialize)]
pub struct SubscriptionBody {
    pub name: String,
    pub email: String,
}

impl UrlEncodedBody for SubscriptionBody {}

pub trait UrlEncodedBody
where
    Self: serde::Serialize,
{
    fn encode(&self) -> String {
        serde_urlencoded::to_string(self).expect("Failed to encode body")
    }
}
