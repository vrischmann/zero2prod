use actix_web::http::header::LOCATION;
use actix_web::HttpResponse;
pub use admin_change_password::*;
pub use admin_dashboard::*;
pub use home::*;
pub use login::*;
pub use newsletters::*;
use std::fmt;
pub use subscriptions::*;
pub use subscriptions_confirm::*;

mod admin_change_password;
mod admin_dashboard;
mod home;
mod login;
mod newsletters;
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

pub fn to_internal_server_error<T>(err: T) -> actix_web::Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    actix_web::error::ErrorInternalServerError(err)
}

pub fn see_other(location: &str) -> HttpResponse {
    HttpResponse::SeeOther()
        .insert_header((LOCATION, location))
        .finish()
}

pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().finish()
}
