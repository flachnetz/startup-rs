use jsonwebtoken::{DecodingKey, Validation};
use jsonwebtoken::jwk::{AlgorithmParameters, Jwk, JwkSet};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;

pub use crate::http::{Jwt, JwtAuth};

mod http;

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtConfig {
    pub jwk_url: String,
    pub validate_expiry_time: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to fetch 'jwks.json'")]
    Http(#[from] reqwest::Error),

    #[error("failed to decode jwt header")]
    DecodeHeader(#[source] jsonwebtoken::errors::Error),

    #[error("jwt header does not contain 'kid' field")]
    NoKeyInHeader,

    #[error("key with kid={0:?} not found")]
    KeyNotFound(String),

    #[error("key has no algorithm configured")]
    KeyHasNoAlgorithm,

    #[error("key type {0:?} is not supported")]
    UnsupportedKeyType(&'static str),

    #[error("decode key")]
    DecodeKey(#[source] jsonwebtoken::errors::Error),

    #[error("decode jwt")]
    DecodeJwt(#[source] jsonwebtoken::errors::Error),
}

pub(crate) async fn request_jwk_set(url: &str, client: &Client) -> Result<JwkSet, Error> {
    tracing::info!("Loading JwkSet from {:?}", url);
    let response = client.get(url).send().await?;
    Ok(response.json().await?)
}

pub(crate) fn decode<C: DeserializeOwned>(keys: &JwkSet, token: &str, validate_exp: bool) -> Result<C, Error> {
    // TODO maybe cache decoding keys
    let header = jsonwebtoken::decode_header(token).map_err(Error::DecodeHeader)?;
    let kid = header.kid.ok_or(Error::NoKeyInHeader)?;
    let key = keys.find(&kid).ok_or(Error::KeyNotFound(kid))?;

    let algorithm = key.common.algorithm.ok_or(Error::KeyHasNoAlgorithm)?;

    let mut validation = Validation::new(algorithm);
    validation.validate_exp = validate_exp;

    let decoding_key = convert_to_decoding_key(key)?;

    // decode token
    let data = jsonwebtoken::decode(token, &decoding_key, &validation).map_err(Error::DecodeJwt)?;

    Ok(data.claims)
}

fn convert_to_decoding_key(key: &Jwk) -> Result<DecodingKey, Error> {
    match &key.algorithm {
        AlgorithmParameters::RSA(p) => Ok(DecodingKey::from_rsa_components(&p.n, &p.e).map_err(Error::DecodeKey)?),

        AlgorithmParameters::EllipticCurve(_) => Err(Error::UnsupportedKeyType("EllipticCurve")),

        AlgorithmParameters::OctetKey(_) => Err(Error::UnsupportedKeyType(")")),

        AlgorithmParameters::OctetKeyPair(_) => Err(Error::UnsupportedKeyType("OctetKeyPair")),
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Claims {
    #[serde(rename = "customerNumber")]
    pub customer_number: u64,
    pub site: String,

    pub grant_type: String,
    pub locale: String,
    pub scope: Vec<String>,
    pub user_name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CustomerClaim {
    #[serde(rename = "customerNumber")]
    pub customer_number: u64,
    pub site: String,
}
