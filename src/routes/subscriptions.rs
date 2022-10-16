use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use sqlx::PgPool;
use tracing::Instrument;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    name: String,
    email: String,
}

pub async fn subscribe(pool: web::Data<PgPool>, form: web::Form<FormData>) -> impl Responder {
    let request_id = Uuid::new_v4();

    let request_span = tracing::info_span!("Adding a new subscriber", %request_id, subscriber_email=%form.email, subscriber_name=%form.name);
    let _request_span_guard = request_span.enter();

    let query_span = tracing::info_span!("Saving new subscriber details in the database");

    match sqlx::query!(
        r#"
        INSERT INTO subscriptions(id, email, name, subscribed_at)
        VALUES($1, $2, $3, $4)"#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now(),
    )
    .execute(pool.get_ref())
    .instrument(query_span)
    .await
    {
        Ok(_) => {
            tracing::info!(
                "request_id={} New subscriber details have been saved",
                request_id
            );
            HttpResponse::Ok()
        }
        Err(err) => {
            tracing::error!(
                "request_id={} unable to insert into subscriptions, err: {:?}",
                request_id,
                err
            );
            HttpResponse::InternalServerError()
        }
    }
}
