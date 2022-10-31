pub use health_check::*;
pub use newsletters::*;
use std::fmt;
pub use subscriptions::*;
pub use subscriptions_confirm::*;

pub mod health_check;
pub mod newsletters;
pub mod subscriptions;
pub mod subscriptions_confirm;

pub fn error_chain_fmt(err: &impl std::error::Error, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    writeln!(f, "{}\n", err)?;
    let mut current = err.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}
