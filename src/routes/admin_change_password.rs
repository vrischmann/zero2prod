use crate::authentication::{validate_credentials, AuthError, Credentials};
use crate::routes::admin_dashboard::get_username;
use crate::routes::{error_chain_fmt, see_other, to_internal_server_error};
use crate::sessions::TypedSession;
use actix_web::http::header::ContentType;
use actix_web::web;
use actix_web::HttpResponse;
use actix_web_flash_messages::{FlashMessage, IncomingFlashMessages, Level as FlashLevel};
use askama::Template;
use secrecy::{ExposeSecret, Secret};
use std::fmt;

#[derive(askama::Template)]
#[template(path = "admin_change_password.html.j2")]
pub struct ChangePasswordTemplate {
    error_messages: Vec<String>,
    info_messages: Vec<String>,
}

pub async fn admin_change_password_form(
    session: TypedSession,
    flash_messages: IncomingFlashMessages,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id_result = session.get_user_id().map_err(to_internal_server_error)?;
    if user_id_result.is_none() {
        return Ok(see_other("/login"));
    }

    let mut error_messages = Vec::<String>::new();

    for m in flash_messages.iter() {
        if m.level() != FlashLevel::Error {
            continue;
        }
        error_messages.push(m.content().to_string());
    }

    //

    let tpl = ChangePasswordTemplate {
        error_messages,
        info_messages: Vec::new(),
    };

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(tpl.render().unwrap()))
}

#[derive(thiserror::Error)]
pub enum ChangePasswordError {
    #[error("Something went wrong")]
    Unexpected(#[from] anyhow::Error),
}

impl fmt::Debug for ChangePasswordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        error_chain_fmt(self, f)
    }
}

#[derive(serde::Deserialize)]
pub struct ChangePasswordFormData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>,
}

pub async fn admin_change_password(
    pool: web::Data<sqlx::PgPool>,
    session: TypedSession,
    form: web::Form<ChangePasswordFormData>,
) -> Result<HttpResponse, actix_web::Error> {
    // Get the user id from the session
    let user_id_result = session.get_user_id().map_err(to_internal_server_error)?;
    if user_id_result.is_none() {
        return Ok(see_other("/login"));
    }
    let user_id = user_id_result.unwrap();

    // Validate new password
    if form.new_password.expose_secret() != form.new_password_check.expose_secret() {
        FlashMessage::error(
            "You entered two different new passwords - the field values must match.",
        )
        .send();
        return Ok(see_other("/admin/password"));
    }
    let new_password_len = form.new_password.expose_secret().len();
    if new_password_len < 12 {
        FlashMessage::error("New password is too short").send();
        return Ok(see_other("/admin/password"));
    }
    if new_password_len > 128 {
        FlashMessage::error("New password is too long").send();
        return Ok(see_other("/admin/password"));
    }

    // Obtain the username
    let username = get_username(&pool, user_id)
        .await
        .map_err(to_internal_server_error)?;

    // Validate the credentials
    let credentials = Credentials {
        username,
        password: form.0.current_password,
    };

    if let Err(err) = validate_credentials(&pool, credentials).await {
        match err {
            AuthError::InvalidCredentials(_) => {
                FlashMessage::error("The current password is incorrect").send();
                return Ok(see_other("/admin/password"));
            }
            AuthError::Unexpected(_) => return Err(to_internal_server_error(err)),
        }
    }

    todo!()
}
