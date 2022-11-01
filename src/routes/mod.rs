use actix_web::HttpResponse;
pub use home::*;
pub use login::*;
pub use newsletters::*;
use std::fmt;
pub use subscriptions::*;
pub use subscriptions_confirm::*;

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

pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().finish()
}
