use actix_web::http::header::LOCATION;
use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use std::fmt;

pub use admin_change_password::*;
pub use admin_dashboard::*;
pub use admin_logout::*;
pub use admin_newsletters::*;
pub use home::*;
pub use login::*;
pub use subscriptions::*;
pub use subscriptions_confirm::*;

mod admin_change_password;
mod admin_dashboard;
mod admin_logout;
mod admin_newsletters;
mod home;
mod login;
mod subscriptions;
mod subscriptions_confirm;

pub fn error_chain_fmt(err: &impl std::error::Error, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    writeln!(f, "{}\n", err)?;
    let mut current = err.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

pub fn e500<T>(err: T) -> actix_web::error::InternalError<T>
where
    T: fmt::Debug + fmt::Display + 'static,
{
    actix_web::error::InternalError::new(err, StatusCode::INTERNAL_SERVER_ERROR)
}

pub fn e400<T>(err: T) -> actix_web::error::InternalError<T>
where
    T: fmt::Debug + fmt::Display + 'static,
{
    actix_web::error::InternalError::new(err, StatusCode::BAD_REQUEST)
}

pub fn see_other(location: &str) -> HttpResponse {
    HttpResponse::SeeOther()
        .insert_header((LOCATION, location))
        .finish()
}

// pub fn flash_messages_to_strings(flash_messages: IncomingFlashMessages)

pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().finish()
}
