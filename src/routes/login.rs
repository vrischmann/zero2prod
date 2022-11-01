use crate::authentication::{validate_credentials, AuthError, Credentials};
use crate::routes::error_chain_fmt;
use actix_web::http::header::{ContentType, LOCATION};
use actix_web::http::StatusCode;
use actix_web::web;
use actix_web::{HttpResponse, ResponseError};
use askama::Template;
use secrecy::Secret;
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

impl ResponseError for LoginError {
    fn status_code(&self) -> StatusCode {
        StatusCode::SEE_OTHER
    }

    fn error_response(&self) -> HttpResponse {
        let encoded_error = serde_urlencoded::to_string(&[("error", self.to_string())]).unwrap();

        HttpResponse::build(self.status_code())
            .insert_header((LOCATION, format!("/login?{}", encoded_error)))
            .finish()
    }
}

#[derive(serde::Deserialize)]
pub struct LoginFormData {
    username: String,
    password: Secret<String>,
}

#[tracing::instrument(name = "Do login", skip(pool, form))]
pub async fn login(
    pool: web::Data<sqlx::PgPool>,
    form: web::Form<LoginFormData>,
) -> Result<HttpResponse, LoginError> {
    let credentials = Credentials {
        username: form.0.username,
        password: form.0.password,
    };

    tracing::Span::current().record("username", &tracing::field::display(&credentials.username));

    let user_id = validate_credentials(&pool, credentials)
        .await
        .map_err(|err| match err {
            AuthError::InvalidCredentials(_) => LoginError::Auth(err.into()),
            AuthError::Unexpected(_) => LoginError::Unexpected(err.into()),
        })?;

    tracing::Span::current().record("user_id", &tracing::field::display(&user_id));

    Ok(HttpResponse::SeeOther()
        .insert_header((LOCATION, "/"))
        .finish())
}
