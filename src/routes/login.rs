use crate::authentication::{validate_credentials, AuthError, Credentials};
use crate::routes::error_chain_fmt;
use actix_web::error::InternalError;
use actix_web::http::header::{ContentType, LOCATION};
use actix_web::web;
use actix_web::HttpResponse;
use actix_web_flash_messages::{FlashMessage, IncomingFlashMessages, Level as FlashLevel};
use askama::Template;
use secrecy::Secret;
use std::fmt;

#[derive(askama::Template)]
#[template(path = "login.html.j2")]
pub struct LoginTemplate {
    error_messages: Vec<String>,
    info_messages: Vec<String>,
}

pub async fn login_form(flash_messages: IncomingFlashMessages) -> HttpResponse {
    let mut error_messages = Vec::<String>::new();

    for m in flash_messages.iter() {
        if m.level() != FlashLevel::Error {
            continue;
        }
        error_messages.push(m.content().to_string());
    }

    let tpl = LoginTemplate { error_messages, info_messages:Vec::new(), };

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(tpl.render().unwrap())
}

#[derive(thiserror::Error)]
pub enum LoginError {
    #[error("Authentication failed")]
    Auth(#[source] anyhow::Error),
    #[error("Something went wrong")]
    Unexpected(#[from] anyhow::Error),
}

impl fmt::Debug for LoginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        error_chain_fmt(self, f)
    }
}

#[derive(serde::Deserialize)]
pub struct LoginFormData {
    username: String,
    password: Secret<String>,
}

#[tracing::instrument(
    name = "Do login",
    skip(pool, form),
    fields(
        username = tracing::field::Empty,
        user_id = tracing::field::Empty,
    )
)]
pub async fn login(
    pool: web::Data<sqlx::PgPool>,
    form: web::Form<LoginFormData>,
) -> Result<HttpResponse, InternalError<LoginError>> {
    let credentials = Credentials {
        username: form.0.username,
        password: form.0.password,
    };

    tracing::Span::current().record("username", &tracing::field::display(&credentials.username));

    match validate_credentials(&pool, credentials).await {
        Err(err) => {
            let err = match err {
                AuthError::InvalidCredentials(_) => LoginError::Auth(err.into()),
                AuthError::Unexpected(_) => LoginError::Unexpected(err.into()),
            };

            FlashMessage::error(err.to_string()).send();

            let response = HttpResponse::SeeOther()
                .insert_header((LOCATION, "/login"))
                .finish();

            Err(InternalError::from_response(err, response))
        }
        Ok(user_id) => {
            tracing::Span::current().record("user_id", &tracing::field::display(&user_id));

            Ok(HttpResponse::SeeOther()
                .insert_header((LOCATION, "/"))
                .finish())
        }
    }
}
