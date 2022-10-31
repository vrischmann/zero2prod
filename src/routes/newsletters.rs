use crate::domain::SubscriberEmail;
use crate::routes::error_chain_fmt;
use crate::tem;
use actix_web::http::header;
use actix_web::http::header::{HeaderMap, HeaderValue};
use actix_web::http::StatusCode;
use actix_web::web;
use actix_web::{HttpRequest, HttpResponse};
use anyhow::{anyhow, Context};
use secrecy::{ExposeSecret, Secret};
use std::fmt;
use tracing::error;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("Authentication failed")]
    Auth(#[source] anyhow::Error),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl fmt::Debug for PublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl actix_web::ResponseError for PublishError {
    fn error_response(&self) -> HttpResponse {
        match self {
            Self::Auth(_) => {
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();

                let mut response = HttpResponse::new(StatusCode::UNAUTHORIZED);
                response
                    .headers_mut()
                    .insert(header::WWW_AUTHENTICATE, header_value);

                response
            }
            Self::Unexpected(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}

#[derive(serde::Deserialize)]
pub struct Content {
    html: String,
    text: String,
}

#[tracing::instrument(
    name = "Publish newsletter",
    skip(pool, email_client, request, body),
    fields(
        username = tracing::field::Empty,
        user_id = tracing::field::Empty
    )
)]
pub async fn publish_newsletter(
    pool: web::Data<sqlx::PgPool>,
    email_client: web::Data<tem::Client>,
    request: HttpRequest,
    body: web::Json<BodyData>,
) -> Result<HttpResponse, PublishError> {
    let credentials = basic_authentication(request.headers()).map_err(PublishError::Auth)?;
    tracing::Span::current().record("username", &tracing::field::display(&credentials.username));

    let user_id = validate_credentials(&pool, credentials).await?;
    tracing::Span::current().record("user_id", &tracing::field::display(&user_id));

    let subscribers = get_confirmed_subscribers(&pool).await?;
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        &body.title,
                        &body.content.html,
                        &body.content.text,
                    )
                    .await
                    .with_context(|| {
                        format!("Failed to send newsletter issue to {}", subscriber.email)
                    })?;
            }
            Err(err) => {
                error!(
                    error.cause_chain = ?err,
                    "Skipping a confirmed subscriber, their stored contact details are invalid",
                )
            }
        }
    }

    Ok(HttpResponse::Ok().finish())
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(name = "Get confirmed subscriber", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &sqlx::PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let result = sqlx::query!(
        r#"
        SELECT email FROM subscriptions WHERE status = 'confirmed'
        "#,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| match SubscriberEmail::parse(r.email) {
        Ok(email) => Ok(ConfirmedSubscriber { email }),
        Err(err) => Err(anyhow!(err)),
    })
    .collect();

    Ok(result)
}

async fn validate_credentials(
    pool: &sqlx::PgPool,
    credentials: Credentials,
) -> Result<Uuid, PublishError> {
    let user_id: Option<_> = sqlx::query!(
        r#"
        SELECT user_id
        FROM users
        WHERE username = $1 AND password = $2
        "#,
        credentials.username,
        credentials.password.expose_secret(),
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to validate auth credentials")
    .map_err(PublishError::Unexpected)?;

    user_id
        .map(|row| row.user_id)
        .ok_or_else(|| anyhow!("Invalid username or password"))
        .map_err(PublishError::Auth)
}

struct Credentials {
    username: String,
    password: Secret<String>,
}

fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    // Get the header value, decode as base64

    let header_value = headers
        .get("Authorization")
        .context("The 'Authorization' header was missing")?;
    let header_value_string = header_value
        .to_str()
        .context("The 'Authorization' header was not a valid ASCII string")?;

    let encoded_segment = header_value_string
        .strip_prefix("Basic ")
        .context("The 'Authorization' header scheme was not 'Basic'")?;

    let decoded_bytes = base64::decode_config(encoded_segment, base64::STANDARD)
        .context("Failed to base64 decode the 'Basic' credentials")?;
    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("The decoded credentials string is not valid UTF-8")?;

    //

    let mut credentials_iter = decoded_credentials.splitn(2, ':');

    let username = credentials_iter
        .next()
        .ok_or_else(|| anyhow!("A username must be provided in 'Basic' auth"))?;
    let password = credentials_iter
        .next()
        .ok_or_else(|| anyhow!("A password must be provided in 'Basic' auth"))?;

    Ok(Credentials {
        username: username.to_string(),
        password: Secret::new(password.to_string()),
    })
}
