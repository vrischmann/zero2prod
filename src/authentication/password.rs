use crate::telemetry::spawn_blocking_with_tracing;
use anyhow::{anyhow, Context};
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use secrecy::{ExposeSecret, Secret};
use uuid::Uuid;

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials(#[source] anyhow::Error),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

pub struct Credentials {
    pub username: String,
    pub password: Secret<String>,
}

#[tracing::instrument(name = "Validate credentials", skip(pool, credentials))]
pub async fn validate_credentials(
    pool: &sqlx::PgPool,
    credentials: Credentials,
) -> Result<Uuid, AuthError> {
    let mut user_id = None;
    let mut expected_password_hash = Secret::new(
        "$argon2id$v=19$m=15000,t=2,p=1$\
gZiV/M1gPc22ElAH/Jh1Hw$\
CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
            .into(),
    );

    if let Some(stored_credentials) = get_stored_credentials(pool, &credentials.username)
        .await
        .map_err(AuthError::Unexpected)?
    {
        user_id = Some(stored_credentials.0);
        expected_password_hash = stored_credentials.1;
    }

    //

    let verify_result = spawn_blocking_with_tracing(move || {
        verify_password_hash(expected_password_hash, credentials.password)
    })
    .await
    .context("Failed to spawn blocking task")
    .map_err(AuthError::Unexpected)?;

    verify_result?;

    //

    user_id
        .ok_or_else(|| anyhow!("Unknown username"))
        .map_err(AuthError::InvalidCredentials)
}

#[tracing::instrument(name = "Change password", skip(pool, password))]
pub async fn change_password(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    password: Secret<String>,
) -> Result<(), anyhow::Error> {
    // Compute the new hash
    let password_hash_result = spawn_blocking_with_tracing(move || compute_password_hash(password))
        .await
        .context("Failed to spawn blocking task")
        .map_err(Into::<anyhow::Error>::into)?;
    let password_hash = password_hash_result?;

    // Store it
    sqlx::query!(
        r#"
        UPDATE users
        SET password_hash = $1
        WHERE user_id = $2
        "#,
        password_hash.expose_secret(),
        user_id,
    )
    .execute(pool)
    .await
    .context("Failed to update the users password")?;

    Ok(())
}

pub fn compute_password_hash(password: Secret<String>) -> Result<Secret<String>, anyhow::Error> {
    let salt = SaltString::generate(&mut rand::thread_rng());
    let hasher = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(15000, 2, 1, None).unwrap(),
    );

    let password_hash = hasher.hash_password(password.expose_secret().as_bytes(), &salt)?;
    let password_hash_string = password_hash.to_string();

    Ok(Secret::from(password_hash_string))
}

#[tracing::instrument(
    name = "Verify password hash",
    skip(expected_password_hash, password_candidate)
)]
fn verify_password_hash(
    expected_password_hash: Secret<String>,
    password_candidate: Secret<String>,
) -> Result<(), AuthError> {
    let expected_password_hash = PasswordHash::new(expected_password_hash.expose_secret())
        .context("Failed to parse hash in PHC string format")
        .map_err(AuthError::Unexpected)?;

    Argon2::default()
        .verify_password(
            password_candidate.expose_secret().as_bytes(),
            &expected_password_hash,
        )
        .context("failed to verify password")
        .map_err(AuthError::InvalidCredentials)
}

#[tracing::instrument(name = "Get stored credentials", skip(pool))]
async fn get_stored_credentials(
    pool: &sqlx::PgPool,
    username: &str,
) -> Result<Option<(Uuid, Secret<String>)>, anyhow::Error> {
    let row = sqlx::query!(
        r#"
        SELECT user_id, password_hash
        FROM users
        WHERE username = $1
        "#,
        username,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to retrieve stored credentials")?;

    match row {
        Some(row) => {
            let result = (row.user_id, Secret::new(row.password_hash));
            Ok(Some(result))
        }
        None => Ok(None),
    }
}
