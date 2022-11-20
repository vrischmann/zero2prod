use crate::authentication::{change_password, validate_credentials};
use crate::authentication::{AuthError, Credentials, UserId};
use crate::routes::admin_dashboard::get_username;
use crate::routes::{e500, error_chain_fmt, see_other};
use actix_web::http::header::ContentType;
use actix_web::web;
use actix_web::HttpResponse;
use actix_web_flash_messages::{FlashMessage, IncomingFlashMessages};
use askama::Template;
use secrecy::{ExposeSecret, Secret};
use std::fmt;

#[derive(askama::Template)]
#[template(path = "admin_change_password.html.j2")]
pub struct ChangePasswordTemplate {
    flash_messages: Option<IncomingFlashMessages>,
}

pub async fn admin_change_password_form(
    _user_id: web::ReqData<UserId>,
    flash_messages: IncomingFlashMessages,
) -> Result<HttpResponse, actix_web::Error> {
    let tpl = ChangePasswordTemplate {
        flash_messages: Some(flash_messages),
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
    user_id: web::ReqData<UserId>,
    form: web::Form<ChangePasswordFormData>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();

    let form = form.0;

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
    let username = get_username(&pool, *user_id).await.map_err(e500)?;

    // Validate the credentials
    let credentials = Credentials {
        username,
        password: form.current_password,
    };

    if let Err(err) = validate_credentials(&pool, credentials).await {
        match err {
            AuthError::InvalidCredentials(_) => {
                FlashMessage::error("The current password is incorrect").send();
                return Ok(see_other("/admin/password"));
            }
            AuthError::Unexpected(_) => return Err(e500(err).into()),
        }
    }

    // All good; change the password
    change_password(&pool, *user_id, form.new_password)
        .await
        .map_err(e500)?;

    FlashMessage::warning("Your password has been changed").send();

    Ok(see_other("/admin/password"))
}
