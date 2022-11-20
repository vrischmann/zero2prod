use crate::routes::to_internal_server_error;
use crate::sessions::TypedSession;
use actix_web::http::header::ContentType;
use actix_web::http::header::LOCATION;
use actix_web::web;
use actix_web::HttpResponse;
use anyhow::Context;
use askama::Template;
use uuid::Uuid;

#[derive(askama::Template)]
#[template(path = "admin_dashboard.html.j2")]
pub struct LoginTemplate {
    error_messages: Vec<String>,
    info_messages: Vec<String>,
    username: String,
}

pub async fn admin_dashboard(
    pool: web::Data<sqlx::PgPool>,
    session: TypedSession,
) -> Result<HttpResponse, actix_web::Error> {
    let get_user_id_result = session.get_user_id().map_err(to_internal_server_error)?;

    let username = match get_user_id_result {
        Some(user_id) => get_username(&pool, user_id)
            .await
            .map_err(to_internal_server_error)?,
        None => {
            return Ok(HttpResponse::SeeOther()
                .insert_header((LOCATION, "/login"))
                .finish());
        }
    };

    let tpl = LoginTemplate {
        error_messages: Vec::new(),
        info_messages: Vec::new(),
        username,
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
