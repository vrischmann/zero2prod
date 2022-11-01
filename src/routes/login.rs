use crate::authentication::{validate_credentials, AuthError, Credentials};
use crate::routes::error_chain_fmt;
use crate::startup::HmacSecret;
use actix_web::error::InternalError;
use actix_web::http::header::{ContentType, LOCATION};
use actix_web::web;
use actix_web::HttpResponse;
use askama::Template;
use hmac::{Hmac, Mac};
use secrecy::{ExposeSecret, Secret};
use std::fmt;

#[derive(askama::Template)]
#[template(path = "login.html.j2")]
pub struct LoginTemplate {
    error_message: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct QueryParams {
    error: Option<String>,
}

pub async fn login_form(query: web::Query<QueryParams>) -> HttpResponse {
    let tpl = LoginTemplate {
        error_message: query.error.clone(),
    };

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
    skip(pool, hmac_secret, form),
    fields(
        username = tracing::field::Empty,
        user_id = tracing::field::Empty,
    )
)]
pub async fn login(
    pool: web::Data<sqlx::PgPool>,
    hmac_secret: web::Data<HmacSecret>,
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

            let query_string = serde_urlencoded::to_string(&[("error", err.to_string())]).unwrap();

            let hmac_tag = {
                let mut mac =
                    Hmac::<sha2::Sha256>::new_from_slice(hmac_secret.0.expose_secret().as_bytes())
                        .unwrap();
                mac.update(query_string.as_bytes());
                mac.finalize().into_bytes()
            };

            let response = HttpResponse::SeeOther()
                .insert_header((
                    LOCATION,
                    format!("/login?{}&tag={:x}", query_string, hmac_tag),
                ))
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
