use crate::authentication::UserId;
use crate::domain::SubscriberEmail;
use crate::idempotency::IdempotencyKey;
use crate::idempotency::{get_saved_response, save_response};
use crate::routes::{e400, e500, error_chain_fmt, get_username, see_other};
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
    idempotency_key: String,
}

pub async fn newsletter_form(
    user_id: web::ReqData<UserId>,
    flash_messages: IncomingFlashMessages,
) -> Result<HttpResponse, actix_web::Error> {
    let tpl = NewsletterTemplate {
        user_id: Some(*user_id.into_inner()),
        flash_messages: Some(flash_messages),
        idempotency_key: Uuid::new_v4().to_string(),
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
    idempotency_key: String,
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

    // Need to destructure to make the borrow-checker happy
    let NewsletterData {
        title,
        text_content,
        html_content,
        idempotency_key,
    } = form.0;

    // Handle idempotency key if necessary

    let idempotency_key: IdempotencyKey = idempotency_key
        .try_into()
        .map_err(Into::<PublishError>::into)
        .map_err(e400)?;

    let saved_response = get_saved_response(&pool, *user_id, &idempotency_key)
        .await
        .map_err(Into::<PublishError>::into)
        .map_err(e500)?;
    if let Some(saved_response) = saved_response {
        return Ok(saved_response);
    }

    //

    let username_result = get_username(&pool, *user_id)
        .await
        .map_err(Into::<PublishError>::into)
        .map_err(e500);

    let username = username_result?;
    tracing::Span::current().record("username", &tracing::field::display(&username));

    // Validate the content

    if title.is_empty() {
        let err = InternalError::new(PublishError::MissingTitle, StatusCode::BAD_REQUEST);
        return Err(err);
    }

    if text_content.is_empty() || html_content.is_empty() {
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
                    .send_email(&subscriber.email, &title, &html_content, &text_content)
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

    FlashMessage::info("The newsletter issue has been published").send();

    let response = see_other("/admin/newsletters");
    let response = save_response(&pool, *user_id, &idempotency_key, response)
        .await
        .map_err(Into::<PublishError>::into)
        .map_err(e500)?;

    Ok(response)
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
