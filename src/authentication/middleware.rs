use crate::routes::{see_other, to_internal_server_error};
use crate::sessions::TypedSession;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::InternalError;
use actix_web::FromRequest;
use actix_web_lab::middleware::Next;
use anyhow::anyhow;

pub async fn reject_anonymous_users(
    mut req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let session_result = {
        let (http_request, payload) = req.parts_mut();
        TypedSession::from_request(http_request, payload).await
    };
    let session = session_result?;

    let user_id_result = session.get_user_id().map_err(to_internal_server_error)?;
    match user_id_result {
        Some(_) => next.call(req).await,
        None => {
            let response = see_other("/login");
            let err = anyhow!("The user has not logged in");
            Err(InternalError::from_response(err, response).into())
        }
    }
}
