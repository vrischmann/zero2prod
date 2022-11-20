use crate::authentication::UserId;
use crate::domain::SubscriberEmail;
use crate::routes::{error_chain_fmt, get_username};
use crate::tem;
use actix_web::http::header::ContentType;
use actix_web::http::StatusCode;
use actix_web::web;
use actix_web::HttpResponse;
use actix_web_flash_messages::IncomingFlashMessages;
use anyhow::{anyhow, Context};
use askama::Template;
use std::fmt;
use tracing::error;

#[derive(askama::Template)]
#[template(path = "admin_newsletters.html.j2")]
pub struct NewsletterTemplate {
    flash_messages: Option<IncomingFlashMessages>,
}

pub async fn newsletter_form() -> Result<HttpResponse, actix_web::Error> {
    let tpl = NewsletterTemplate {
        flash_messages: None,
    };

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(tpl.render().unwrap()))
}

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
    fn error_response(&self) -> HttpResponse {
        match self {
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
    skip(pool, email_client, body),
    fields(
        username = tracing::field::Empty,
        user_id = tracing::field::Empty
    )
)]
pub async fn publish_newsletter(
    pool: web::Data<sqlx::PgPool>,
    email_client: web::Data<tem::Client>,
    user_id: web::ReqData<UserId>,
    body: web::Json<BodyData>,
) -> Result<HttpResponse, PublishError> {
    let user_id = user_id.into_inner();

    let username = get_username(&pool, *user_id).await?;

    tracing::Span::current().record("username", &tracing::field::display(&username));

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
