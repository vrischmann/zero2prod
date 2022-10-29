use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName};
use crate::startup::ApplicationBaseUrl;
use crate::tem;
use actix_web::{web, HttpResponse};
use chrono::Utc;
use rand::Rng;
use tracing::{event, Level};
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    name: String,
    email: String,
}

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
) -> HttpResponse {
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let new_subscriber = match form.0.try_into() {
        Ok(subscriber) => subscriber,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    let subscriber_id = match insert_subscriber(&mut tx, &new_subscriber).await {
        Ok(subscriber_id) => subscriber_id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let subscription_token = generate_subscription_token();
    if store_token(&mut tx, subscriber_id, &subscription_token)
        .await
        .is_err()
    {
        return HttpResponse::InternalServerError().finish();
    }

    if tx.commit().await.is_err() {
        return HttpResponse::InternalServerError().finish();
    }

    //

    if send_confirmation_email(
        &base_url,
        &email_client,
        new_subscriber,
        &subscription_token,
    )
    .await
    .is_err()
    {
        return HttpResponse::InternalServerError().finish();
    }

    //

    HttpResponse::Ok().finish()
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

    let html_content = format!(
        r#"
Welcome to our newsletter!<br/>
Click <a href="{}">here</a> to confirm your subscription.
    "#,
        confirmation_link
    );

    let text_content = format!(
        r#"
Welcome to our newsletter!<br/>
Visit {} to confirm your subscription.
    "#,
        confirmation_link
    );

    email_client
        .send_email(
            new_subscriber.email,
            "Welcome!",
            &html_content,
            &text_content,
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

#[tracing::instrument(
    name = "Store the subscription token in the database",
    skip(tx, subscription_token)
)]
async fn store_token(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), sqlx::Error> {
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
        err
    })?;

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
