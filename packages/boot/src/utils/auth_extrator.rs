use anyhow::Result;

use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    response::{IntoResponse, Response},
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use hyper::StatusCode;

use _database::{functions::api::auth::verify, types::response::api::AuthInfo};

pub struct ExtractAuthInfo(pub AuthInfo);

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for ExtractAuthInfo
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state).await {
            Ok(bearer) => {
                // TODO - 增加对 Guest 的支持
                let token = bearer.token().to_string();
                let info = verify(token).await.map_err(|err| {
                    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response()
                })?;

                Ok(Self(Some(info)))
            }
            Err(_) => Ok(Self(None)),
        }
    }
}
