use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName};
use crate::startup::ApplicationBaseUrl;
use crate::tem;
use actix_web::http::StatusCode;
use actix_web::{web, HttpResponse};
use anyhow::Context;
use askama::Template;
use chrono::Utc;
use rand::Rng;
use std::fmt;
use tracing::{event, Level};
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    name: String,
    email: String,
}

#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl actix_web::ResponseError for SubscribeError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Validation(_) => StatusCode::BAD_REQUEST,
            Self::Unexpected(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<String> for SubscribeError {
    fn from(e: String) -> Self {
        Self::Validation(e)
    }
}

//

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(base_url, pool, email_client, form),
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name
    )
)]
pub async fn subscribe(
    base_url: web::Data<ApplicationBaseUrl>,
    pool: web::Data<sqlx::PgPool>,
    email_client: web::Data<tem::Client>,
    form: web::Form<FormData>,
) -> Result<HttpResponse, SubscribeError> {
    let mut tx = pool
        .begin()
        .await
        .context("Failed to acquire a Postgre connection from the pool")?;

    let new_subscriber = form.0.try_into().map_err(SubscribeError::Validation)?;
    let subscriber_id = insert_subscriber(&mut tx, &new_subscriber)
        .await
        .context("Failed to insert new subscriber in the database")?;

    let subscription_token = generate_subscription_token();
    store_token(&mut tx, subscriber_id, &subscription_token)
        .await
        .context("Failed to store the confirmation token for a new subscriber.")?;

    tx.commit()
        .await
        .context("Failed to commit SQL transaction to store a new subscriber")?;

    //

    send_confirmation_email(
        &base_url,
        &email_client,
        new_subscriber,
        &subscription_token,
    )
    .await
    .context("Failed to send a confirmation email.")?;

    //

    Ok(HttpResponse::Ok().finish())
}

#[derive(askama::Template)]
#[template(path = "html_content.html")]
struct HtmlContentTemplate<'a> {
    confirmation_link: &'a str,
}

#[derive(askama::Template)]
#[template(path = "text_content.txt")]
struct TextContentTemplate<'a> {
    confirmation_link: &'a str,
}

#[tracing::instrument(
    name = "Send a confirmation email to a new subscriber",
    skip(base_url, email_client)
)]
async fn send_confirmation_email(
    base_url: &ApplicationBaseUrl,
    email_client: &tem::Client,
    new_subscriber: NewSubscriber,
    subscription_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url.0, subscription_token
    );

    event!(Level::INFO, confirmation_link, "computed confirmation link");

    let html_content = HtmlContentTemplate {
        confirmation_link: &confirmation_link,
    };

    let text_content = TextContentTemplate {
        confirmation_link: &confirmation_link,
    };

    email_client
        .send_email(
            new_subscriber.email,
            "Welcome!",
            &html_content.render().unwrap(),
            &text_content.render().unwrap(),
        )
        .await
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(value.name)?;
        let email = SubscriberEmail::parse(value.email)?;

        Ok(NewSubscriber { email, name })
    }
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(tx, new_subscriber)
)]
async fn insert_subscriber(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    new_subscriber: &crate::domain::NewSubscriber,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO subscriptions(id, email, name, subscribed_at, status)
        VALUES($1, $2, $3, $4, $5)"#,
        subscriber_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now(),
        "pending_confirmation",
    )
    .execute(tx)
    .await
    .map_err(|err| {
        tracing::error!("Failed to execute query: {:?}", err);
        err
    })?;

    Ok(subscriber_id)
}

fn generate_subscription_token() -> String {
    let mut rng = rand::thread_rng();

    let mut token = String::new();
    for _ in 0..25 {
        let n = rng.sample(rand::distributions::Alphanumeric);
        let c = char::from(n);
        token.push(c);
    }
    token
}

pub struct StoreTokenError(sqlx::Error);

impl fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to store a subscription token"
        )
    }
}

impl fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

#[tracing::instrument(
    name = "Store the subscription token in the database",
    skip(tx, subscription_token)
)]
async fn store_token(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), StoreTokenError> {
    sqlx::query!(
        r#"
        INSERT INTO subscription_tokens(subscriber_id, subscription_token)
        VALUES($1, $2)"#,
        subscriber_id,
        subscription_token,
    )
    .execute(tx)
    .await
    .map_err(|err| {
        tracing::error!("Failed to execute query: {:?}", err);
        StoreTokenError(err)
    })?;

    Ok(())
}

fn error_chain_fmt(err: &impl std::error::Error, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    writeln!(f, "{}\n", err)?;
    let mut current = err.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subscription_token_is_25_chars_long() {
        let token = generate_subscription_token();
        assert_eq!(token.len(), 25);
    }
}
