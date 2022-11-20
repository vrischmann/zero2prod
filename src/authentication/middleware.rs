use crate::routes::{see_other, e500};
use crate::sessions::TypedSession;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::InternalError;
use actix_web::FromRequest;
use actix_web::HttpMessage;
use actix_web_lab::middleware::Next;
use anyhow::anyhow;
use std::ops::Deref;
use uuid::Uuid;

#[derive(Copy, Clone, Debug)]
pub struct UserId(Uuid);

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for UserId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub async fn reject_anonymous_users(
    mut req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let session_result = {
        let (http_request, payload) = req.parts_mut();
        TypedSession::from_request(http_request, payload).await
    };
    let session = session_result?;

    let user_id_result = session.get_user_id().map_err(e500)?;
    match user_id_result {
        Some(user_id) => {
            req.extensions_mut().insert(UserId(user_id));
            next.call(req).await
        }
        None => {
            let response = see_other("/login");
            let err = anyhow!("The user has not logged in");
            Err(InternalError::from_response(err, response).into())
        }
    }
}
