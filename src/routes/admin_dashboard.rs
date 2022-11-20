use crate::authentication::UserId;
use crate::routes::e500;
use actix_web::http::header::ContentType;
use actix_web::web;
use actix_web::HttpResponse;
use actix_web_flash_messages::IncomingFlashMessages;
use anyhow::Context;
use askama::Template;
use uuid::Uuid;

#[derive(askama::Template)]
#[template(path = "admin_dashboard.html.j2")]
pub struct DashboardTemplate {
    user_id: Option<Uuid>,
    username: String,
    flash_messages: Option<IncomingFlashMessages>,
}

pub async fn admin_dashboard(
    pool: web::Data<sqlx::PgPool>,
    flash_messages: IncomingFlashMessages,
    user_id: web::ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();

    let username = get_username(&pool, *user_id).await.map_err(e500)?;

    let tpl = DashboardTemplate {
        user_id: Some(*user_id),
        username,
        flash_messages: Some(flash_messages),
    };

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(tpl.render().unwrap()))
}

#[tracing::instrument(name = "Get username", skip(pool))]
pub async fn get_username(pool: &sqlx::PgPool, user_id: Uuid) -> Result<String, anyhow::Error> {
    let row = sqlx::query!(
        r#"
        SELECT username FROM users
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_one(pool)
    .await
    .context("Failed to perform query to retrieve the username")?;

    Ok(row.username)
}
