use crate::authentication::UserId;
use crate::idempotency::{save_response, try_processing};
use crate::idempotency::{IdempotencyKey, NextAction};
use crate::routes::{e400, e500, error_chain_fmt, see_other};
use actix_web::error::InternalError;
use actix_web::http::header::ContentType;
use actix_web::http::StatusCode;
use actix_web::web;
use actix_web::HttpResponse;
use actix_web_flash_messages::{FlashMessage, IncomingFlashMessages};
use anyhow::Context;
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

const SUCCESS_MESSAGE: &str = "The newsletter issue has been published";

#[derive(serde::Deserialize)]
pub struct NewsletterData {
    title: String,
    text_content: String,
    html_content: String,
    idempotency_key: String,
}

#[tracing::instrument(name = "Publish newsletter", skip(pool, form))]
pub async fn publish_newsletter(
    pool: web::Data<sqlx::PgPool>,
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

    // 1) Handle idempotency key if necessary

    let idempotency_key: IdempotencyKey = idempotency_key
        .try_into()
        .map_err(Into::<PublishError>::into)
        .map_err(e400)?;

    let next_action = try_processing(&pool, *user_id, &idempotency_key)
        .await
        .map_err(Into::<PublishError>::into)
        .map_err(e500)?;
    let mut transaction = match next_action {
        NextAction::StartProcessing(transaction) => transaction,
        NextAction::ReturnSavedResponse(response) => {
            FlashMessage::info(SUCCESS_MESSAGE).send();
            return Ok(response);
        }
    };

    // 2) Validate the content

    if title.is_empty() {
        let err = InternalError::new(PublishError::MissingTitle, StatusCode::BAD_REQUEST);
        return Err(err);
    }

    if text_content.is_empty() || html_content.is_empty() {
        let err = InternalError::new(PublishError::MissingContent, StatusCode::BAD_REQUEST);
        return Err(err);
    }

    // 3) Insert newsletter issue and enqueue delivery tasks

    let issue_id = insert_newsletter_issue(&mut transaction, &title, &text_content, &html_content)
        .await
        .context("Failed to store newsletter issue details")
        .map_err(Into::<PublishError>::into)
        .map_err(e500)?;

    enqueue_delivery_tasks(&mut transaction, issue_id)
        .await
        .context("Failed to enqueue delivery tasks")
        .map_err(Into::<PublishError>::into)
        .map_err(e500)?;

    // 4) Finally produce the response and save it to the database

    let response = see_other("/admin/newsletters");
    let response = save_response(transaction, *user_id, &idempotency_key, response)
        .await
        .map_err(Into::<PublishError>::into)
        .map_err(e500)?;

    FlashMessage::info(SUCCESS_MESSAGE).send();

    Ok(response)
}

#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO newsletter_issues(id, title, text_content, html_content, published_at)
        VALUES ($1, $2, $3, $4, now())
        "#,
        id,
        title,
        text_content,
        html_content,
    )
    .execute(transaction)
    .await?;

    Ok(id)
}

#[tracing::instrument(skip_all)]
async fn enqueue_delivery_tasks(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    newsletter_issue_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO issue_delivery_queue(newsletter_issue_id, subscriber_email)
        SELECT $1, email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
        newsletter_issue_id,
    )
    .execute(transaction)
    .await?;

    Ok(())
}
