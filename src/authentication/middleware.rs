use crate::session_state::TypedSession;
use crate::utils::{e500, see_other};
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::InternalError;
use actix_web::middleware::Next;
use actix_web::FromRequest;
use actix_web::HttpMessage;
use std::ops::Deref;
use uuid::Uuid;

// we will attach the user_id wrapped in this struct to the http request
#[derive(Copy, Clone, Debug)]
pub struct UserId(Uuid);
// just unwrap the inner
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
    // get a session handler
    let session = {
        // get the request and 'payload' - ie any messages etc associated with it
        // create a new isntance of our session handler
        let (http_request, payload) = req.parts_mut();
        TypedSession::from_request(http_request, payload).await
    }?;

    // check if the session state contains a user id
    match session.get_user_id().map_err(e500)? {
        // if so, invoke the session handler
        Some(user_id) => {
            // add the user id to the request via an 'extension'
            req.extensions_mut().insert(UserId(user_id));
            next.call(req).await
        }
        // if no session redirect to login
        None => {
            let response = see_other("/login");
            let e = anyhow::anyhow!("The user has not logged in");
            Err(InternalError::from_response(e, response).into())
        }
    }
}
