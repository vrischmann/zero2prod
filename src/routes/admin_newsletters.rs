use crate::authentication::UserId;
use crate::domain::SubscriberEmail;
use crate::routes::{e500, error_chain_fmt, get_username, see_other};
use crate::tem;
use actix_web::error::InternalError;
use actix_web::http::header::ContentType;
use actix_web::http::StatusCode;
use actix_web::web;
use actix_web::HttpResponse;
use actix_web_flash_messages::{FlashMessage, IncomingFlashMessages};
use anyhow::anyhow;
use askama::Template;
use std::fmt;
use tracing::error;
use uuid::Uuid;

#[derive(askama::Template)]
#[template(path = "admin_newsletters.html.j2")]
pub struct NewsletterTemplate {
    user_id: Option<Uuid>,
    flash_messages: Option<IncomingFlashMessages>,
}

pub async fn newsletter_form(
    user_id: web::ReqData<UserId>,
    flash_messages: IncomingFlashMessages,
) -> Result<HttpResponse, actix_web::Error> {
    let tpl = NewsletterTemplate {
        user_id: Some(*user_id.into_inner()),
        flash_messages: Some(flash_messages),
    };

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(tpl.render().unwrap()))
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("missing title")]
    MissingTitle,
    #[error("missing content")]
    MissingContent,
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl fmt::Debug for PublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        error_chain_fmt(self, f)
    }
}

#[derive(serde::Deserialize)]
pub struct NewsletterData {
    title: String,
    text_content: String,
    html_content: String,
}

#[tracing::instrument(
    name = "Publish newsletter",
    skip(pool, email_client, form),
    fields(
        username = tracing::field::Empty,
        user_id = tracing::field::Empty
    )
)]
pub async fn publish_newsletter(
    pool: web::Data<sqlx::PgPool>,
    email_client: web::Data<tem::Client>,
    user_id: web::ReqData<UserId>,
    form: web::Form<NewsletterData>,
) -> Result<HttpResponse, InternalError<PublishError>> {
    let user_id = user_id.into_inner();

    let username_result = get_username(&pool, *user_id)
        .await
        .map_err(Into::<PublishError>::into)
        .map_err(e500);

    let username = username_result?;
    tracing::Span::current().record("username", &tracing::field::display(&username));

    // Validate the content

    if form.title.is_empty() {
        let err = InternalError::new(PublishError::MissingTitle, StatusCode::BAD_REQUEST);
        return Err(err);
    }

    if form.text_content.is_empty() || form.html_content.is_empty() {
        let err = InternalError::new(PublishError::MissingContent, StatusCode::BAD_REQUEST);
        return Err(err);
    }

    //

    let subscribers = get_confirmed_subscribers(&pool)
        .await
        .map_err(Into::<PublishError>::into)
        .map_err(e500)?;

    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                let send_result = email_client
                    .send_email(
                        &subscriber.email,
                        &form.title,
                        &form.html_content,
                        &form.text_content,
                    )
                    .await;

                if send_result.is_err() {
                    FlashMessage::error(&format!(
                        "Unable to send newsletter to subscriber {}",
                        &subscriber.email
                    ))
                    .send();
                    return Ok(see_other("/admin/newsletters"));
                }
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
