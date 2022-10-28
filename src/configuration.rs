use crate::domain::SubscriberEmail;
use secrecy::{ExposeSecret, Secret};

#[derive(serde::Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSetttings,
    pub tem: TEMSettings,
}

#[derive(serde::Deserialize)]
pub struct ApplicationSetttings {
    pub host: String,
    pub port: u16,
}

#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,
    pub port: u16,
    pub host: String,
    pub database_name: String,
}

impl DatabaseSettings {
    pub fn connection_string(&self) -> Secret<String> {
        Secret::new(format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username,
            self.password.expose_secret(),
            self.host,
            self.port,
            self.database_name
        ))
    }
}

#[derive(serde::Deserialize)]
pub struct TEMSettings {
    pub base_url: String,
    pub auth_key: Secret<String>,
    pub project_id: String,
    pub sender_email: String,
    pub timeout_milliseconds: u64,
}

impl TEMSettings {
    pub fn sender(&self) -> Result<SubscriberEmail, String> {
        SubscriberEmail::parse(self.sender_email.clone())
    }

    pub fn project_id(&self) -> crate::tem::ProjectId {
        crate::tem::ProjectId::new(self.project_id.clone())
    }

    pub fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.timeout_milliseconds)
    }
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let settings = config::Config::builder()
        .add_source(
            config::File::new("configuration.yml", config::FileFormat::Yaml).required(false),
        )
        .add_source(
            config::File::new("/etc/zero2prod.yml", config::FileFormat::Yaml).required(false),
        )
        .add_source(
            config::Environment::default()
                .try_parsing(true)
                .separator("_"),
        )
        .build()?;

    settings.try_deserialize::<Settings>()
}
