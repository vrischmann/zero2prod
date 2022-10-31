use crate::domain::SubscriberEmail;
use crate::routes::error_chain_fmt;
use crate::tem;
use actix_web::http::StatusCode;
use actix_web::web;
use actix_web::HttpResponse;
use anyhow::{anyhow, Context};
use std::fmt;
use tracing::error;

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl fmt::Debug for PublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl actix_web::ResponseError for PublishError {
    fn status_code(&self) -> StatusCode {
        match self {
            PublishError::Unexpected(_) => StatusCode::INTERNAL_SERVER_ERROR,
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

pub async fn publish_newsletter(
    pool: web::Data<sqlx::PgPool>,
    email_client: web::Data<tem::Client>,
    body: web::Json<BodyData>,
) -> Result<HttpResponse, PublishError> {
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
    let rows = sqlx::query!(
        r#"
        SELECT email FROM subscriptions WHERE status = 'confirmed'
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for row in rows {
        // TODO(vincent): refactor
        result.push(match SubscriberEmail::parse(row.email) {
            Ok(email) => Ok(ConfirmedSubscriber { email }),
            Err(err) => Err(anyhow!(err)),
        })
    }

    Ok(result)
}
