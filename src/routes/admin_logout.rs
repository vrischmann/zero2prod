use crate::routes::{see_other, to_internal_server_error};
use crate::sessions::TypedSession;
use actix_web::HttpResponse;
use actix_web_flash_messages::FlashMessage;

#[tracing::instrument(
    name = "Do logout",
    skip(session),
    fields(
        username = tracing::field::Empty,
        user_id = tracing::field::Empty,
    )
)]
pub async fn logout(session: TypedSession) -> Result<HttpResponse, actix_web::Error> {
    let user_id = session.get_user_id().map_err(to_internal_server_error)?;
    match user_id {
        Some(_) => {
            session.logout();
            FlashMessage::info("You have successfully logged out").send();
            Ok(see_other("/login"))
        }
        None => Ok(see_other("/login")),
    }
}
