use axum::{
    AddExtensionLayer,
    async_trait,
    extract::{Extension, FromRequest, RequestParts, TypedHeader},
};
use headers::{Authorization, authorization::Bearer};
use http::StatusCode;
use jsonwebtoken::jwk::JwkSet;
use reqwest::Client;
use serde::de::DeserializeOwned;
use tracing::{debug, error, warn};

use crate::{Error, JwtConfig};

#[derive(Clone)]
pub struct JwtAuth {
    validate_expiry_time: bool,
    jwk_set: JwkSet,
}

impl JwtAuth {
    pub async fn new(config: &JwtConfig) -> Result<Self, Error> {
        Self::new_with_client(config, Client::new()).await
    }

    pub async fn new_with_client(config: &JwtConfig, client: reqwest::Client) -> Result<Self, Error> {
        let jwk_set = crate::request_jwk_set(&config.jwk_url, &client).await?;
        let validate_expiry_time = config.validate_expiry_time;
        Ok(Self {
            validate_expiry_time,
            jwk_set,
        })
    }

    pub fn into_layer(self) -> AddExtensionLayer<Self> {
        AddExtensionLayer::new(self)
    }
}

pub struct Jwt<C: DeserializeOwned>(pub C);

#[async_trait]
impl<B, C> FromRequest<B> for Jwt<C>
    where
        B: Send,
        C: DeserializeOwned,
{
    type Rejection = StatusCode;

    #[tracing::instrument(name = "parse-jwt", skip(req))]
    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = TypedHeader::<Authorization<Bearer>>::from_request(req)
            .await
            .map_err(|_err| {
                debug!("No 'Authorization' header found");
                StatusCode::UNAUTHORIZED
            })?;

        let Extension(auth) = Extension::<JwtAuth>::from_request(req).await.map_err(|_err| {
            error!("No 'JwtAuth' found on request. Did you add the layer?");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        match crate::decode::<C>(&auth.jwk_set, bearer.token(), auth.validate_expiry_time) {
            Ok(claims) => Ok(Jwt(claims)),
            Err(err) => {
                warn!("Token is invalid: {:?}", err);
                Err(StatusCode::UNAUTHORIZED)
            }
        }
    }
}
