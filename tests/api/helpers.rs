use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use once_cell::sync::Lazy;
use secrecy::ExposeSecret;
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::MockServer;
use zero2prod::configuration::get_configuration;
use zero2prod::startup::Application;
use zero2prod::startup::{get_connection_pool, get_tem_client};
use zero2prod::telemetry;
use zero2prod::tem;

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
    pub email_client: tem::Client,
    pub http_client: reqwest::Client,

    pub test_user: TestUser,
}

impl TestApp {
    pub async fn post_subscriptions<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.http_client
            .post(&format!("{}/subscriptions", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_login<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.http_client
            .post(&format!("{}/login", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_logout(&self) -> reqwest::Response {
        self.http_client
            .post(&format!("{}/admin/logout", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_login_html(&self) -> String {
        let response = self
            .http_client
            .get(&format!("{}/login", &self.address))
            .send()
            .await
            .expect("Failed to execute request.");

        response.text().await.unwrap()
    }

    pub async fn get_admin_dashboard(&self) -> reqwest::Response {
        self.http_client
            .get(&format!("{}/admin/dashboard", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_admin_dashboard_html(&self) -> String {
        let response = self.get_admin_dashboard().await;
        response.text().await.unwrap()
    }

    pub async fn get_admin_change_password(&self) -> reqwest::Response {
        self.http_client
            .get(&format!("{}/admin/password", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_admin_change_password_html(&self) -> String {
        let response = self.get_admin_change_password().await;
        response.text().await.unwrap()
    }

    pub async fn post_admin_change_password<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.http_client
            .post(&format!("{}/admin/password", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_admin_newsletters(&self) -> reqwest::Response {
        self.http_client
            .get(&format!("{}/admin/newsletters", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_admin_newsletters_html(&self) -> String {
        let response = self.get_admin_newsletters().await;
        response.text().await.unwrap()
    }

    pub async fn post_admin_newsletters<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.http_client
            .post(&format!("{}/admin/newsletters", &self.address))
            .form(body)
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

pub async fn spawn_app() -> TestApp {
    let configuration = get_configuration().expect("Failed to read configuration");

    let pool = get_connection_pool(&configuration.database).await;

    spawn_app_with_pool(pool).await
}

pub async fn spawn_app_with_pool(pool: sqlx::PgPool) -> TestApp {
    Lazy::force(&TRACING);

    //

    let email_server = MockServer::start().await;

    let mut configuration = get_configuration().expect("Failed to read configuration");
    configuration.application.port = 0;
    configuration.tem.base_url = email_server.uri();

    // Build the stuff needed for the test harness
    let test_app_pool = pool.clone();
    let test_app_email_client = get_tem_client(&configuration.tem);

    // Build the application
    let app_pool = pool.clone();
    let app_email_client = get_tem_client(&configuration.tem);

    let app = Application::build_with_pool(configuration, app_pool, app_email_client)
        .await
        .expect("Failed to build application");
    let app_port = app.port;

    let _ = tokio::spawn(app.run_until_stopped());

    //

    let http_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .expect("Failed to build HTTP client");

    let test_app = TestApp {
        address: format!("http://127.0.0.1:{}", app_port),
        port: app_port,
        pool: test_app_pool,
        email_server,
        email_client: test_app_email_client,
        http_client,
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

pub fn assert_is_redirect_to(response: &reqwest::Response, location: &str) {
    assert_eq!(
        response.status().as_u16(),
        303,
        "got {}, expected {}",
        response.status().as_u16(),
        303
    );
    assert_eq!(response.headers().get("Location").unwrap(), location);
}

#[derive(serde::Serialize)]
pub struct SubscriptionBody {
    pub name: String,
    pub email: String,
}

#[derive(serde::Serialize)]
pub struct LoginBody {
    pub username: String,
    pub password: String,
}

#[derive(serde::Serialize)]
pub struct AdminChangePasswordBody {
    pub current_password: String,
    pub new_password: String,
    pub new_password_check: String,
}

#[derive(serde::Serialize)]
pub struct SubmitNewsletterBody {
    pub title: String,
    pub html_content: String,
    pub text_content: String,
    pub idempotency_key: Uuid,
}
