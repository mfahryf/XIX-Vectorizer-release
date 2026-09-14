use crate::licensing::device::{DeviceIdentity, SignedRequest};
use crate::licensing::error::LicenseError;
use crate::licensing::models::{LeasePayload, PRODUCT_ID};
use crate::licensing::usage::UsageRecord;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const DEFAULT_GATEWAY_URL: &str = "https://payment.xixlabs.net";

#[derive(Clone)]
pub struct LicenseClient {
    base_url: String,
    http: Client,
    gateway_public_key: Option<VerifyingKey>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TrialClaimResponse {
    #[serde(default)]
    pub trial_remaining_by_engine: BTreeMap<String, u8>,
    pub claimed_at: Option<i64>,
    pub server_time: Option<i64>,
}

impl LicenseClient {
    pub fn new(
        base_url: impl Into<String>,
        gateway_public_key_b64: Option<&str>,
    ) -> Result<Self, LicenseError> {
        crate::net::http::ensure_crypto_provider();
        let gateway_public_key = gateway_public_key_b64
            .filter(|key| !key.trim().is_empty())
            .map(parse_public_key)
            .transpose()?;
        let http = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|error| LicenseError::Configuration(error.to_string()))?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http,
            gateway_public_key,
        })
    }

    pub fn production() -> Result<Self, LicenseError> {
        Self::new(
            DEFAULT_GATEWAY_URL,
            option_env!("XIX_GATEWAY_PUBLIC_KEY_B64"),
        )
    }

    pub fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    pub async fn claim_trial(
        &self,
        identity: &DeviceIdentity,
        app_version: &str,
    ) -> Result<TrialClaimResponse, LicenseError> {
        let value = self
            .post_signed(
                "/v1/desktop/trial/claim",
                identity,
                app_version,
                "trial_claim",
                json!({}),
            )
            .await?;
        let source = value
            .get("trial")
            .or_else(|| value.get("data").and_then(|data| data.get("trial")))
            .unwrap_or(&value);
        serde_json::from_value(source.clone())
            .map_err(|error| LicenseError::Network(error.to_string()))
    }

    pub async fn activate(
        &self,
        identity: &DeviceIdentity,
        app_version: &str,
        license_key: &str,
    ) -> Result<LeasePayload, LicenseError> {
        if license_key.trim().is_empty() {
            return Err(LicenseError::InvalidLicenseKey);
        }
        let value = self
            .post_signed(
                "/v1/desktop/license/activate",
                identity,
                app_version,
                "license_activate",
                json!({ "license_key": license_key.trim() }),
            )
            .await?;
        self.parse_and_verify_lease(value, identity)
    }

    pub async fn status(
        &self,
        identity: &DeviceIdentity,
        app_version: &str,
    ) -> Result<Option<LeasePayload>, LicenseError> {
        let nonce = uuid::Uuid::new_v4().to_string();
        let payload = json!({
            "action": "license_status",
            "product_id": PRODUCT_ID,
            "app_version": app_version,
            "device_fingerprint": identity.fingerprint(),
        });
        let signed = identity.sign_request(&nonce, &canonical_json(&payload)?);
        let response = self
            .http
            .get(self.endpoint("/v1/desktop/license/status"))
            .headers(request_headers(&signed))
            .query(&[("product_id", PRODUCT_ID), ("app_version", app_version)])
            .send()
            .await
            .map_err(|error| LicenseError::Network(error.to_string()))?;
        let value = response_value(response).await?;
        if value.is_null() || value.get("lease").is_some_and(Value::is_null) {
            return Ok(None);
        }
        self.parse_and_verify_lease(value, identity).map(Some)
    }

    pub async fn renew(
        &self,
        identity: &DeviceIdentity,
        app_version: &str,
    ) -> Result<LeasePayload, LicenseError> {
        let value = self
            .post_signed(
                "/v1/desktop/license/renew",
                identity,
                app_version,
                "license_renew",
                json!({}),
            )
            .await?;
        self.parse_and_verify_lease(value, identity)
    }

    pub async fn record_usage(
        &self,
        identity: &DeviceIdentity,
        app_version: &str,
        record: &UsageRecord,
    ) -> Result<(), LicenseError> {
        let _ = self
            .post_signed(
                "/v1/desktop/usage/record",
                identity,
                app_version,
                "usage_record",
                serde_json::to_value(record)
                    .map_err(|error| LicenseError::Network(error.to_string()))?,
            )
            .await?;
        Ok(())
    }

    async fn post_signed(
        &self,
        path: &str,
        identity: &DeviceIdentity,
        app_version: &str,
        action: &str,
        payload: Value,
    ) -> Result<Value, LicenseError> {
        let nonce = uuid::Uuid::new_v4().to_string();
        let signed_payload = json!({
            "action": action,
            "product_id": PRODUCT_ID,
            "app_version": app_version,
            "payload": payload,
        });
        let signed = identity.sign_request(&nonce, &canonical_json(&signed_payload)?);
        let body = json!({
            "product_id": PRODUCT_ID,
            "app_version": app_version,
            "action": action,
            "payload": payload,
            "public_key": signed.public_key,
            "device_fingerprint": signed.device_fingerprint,
            "registration_id": signed.registration_id,
            "nonce": signed.nonce,
            "signature": signed.signature,
        });
        let response = self
            .http
            .post(self.endpoint(path))
            .header("X-Desktop-Product", PRODUCT_ID)
            .json(&body)
            .send()
            .await
            .map_err(|error| LicenseError::Network(error.to_string()))?;
        response_value(response).await
    }

    fn parse_and_verify_lease(
        &self,
        value: Value,
        identity: &DeviceIdentity,
    ) -> Result<LeasePayload, LicenseError> {
        let source = value
            .get("lease")
            .or_else(|| value.get("data").and_then(|data| data.get("lease")))
            .unwrap_or(&value);
        let lease: LeasePayload = serde_json::from_value(source.clone())
            .map_err(|error| LicenseError::InvalidLease(error.to_string()))?;
        let key = self.gateway_public_key.as_ref().ok_or_else(|| {
            LicenseError::InvalidLease(
                "verification key gateway belum dipasang pada build ini".into(),
            )
        })?;
        if !verify_lease_signature(&lease, key) {
            return Err(LicenseError::InvalidLease(
                "signature lease tidak cocok".into(),
            ));
        }
        let now = lease.server_time;
        if !lease.is_valid_for(PRODUCT_ID, &identity.fingerprint(), now) {
            return Err(LicenseError::InvalidLease(
                "isi lease tidak sesuai perangkat atau masa berlaku".into(),
            ));
        }
        Ok(lease)
    }
}

fn parse_public_key(value: &str) -> Result<VerifyingKey, LicenseError> {
    let bytes = BASE64
        .decode(value.trim())
        .map_err(|error| LicenseError::Configuration(error.to_string()))?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| LicenseError::Configuration("verification key harus 32 byte".into()))?;
    VerifyingKey::from_bytes(&bytes).map_err(|error| LicenseError::Configuration(error.to_string()))
}

fn request_headers(signed: &SignedRequest) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    let values = [
        ("X-Device-Public-Key", signed.public_key.as_str()),
        ("X-Device-Fingerprint", signed.device_fingerprint.as_str()),
        ("X-Device-Registration", signed.registration_id.as_str()),
        ("X-Device-Nonce", signed.nonce.as_str()),
        ("X-Device-Signature", signed.signature.as_str()),
    ];
    for (name, value) in values {
        if let (Ok(header_name), Ok(header_value)) = (
            reqwest::header::HeaderName::from_bytes(name.as_bytes()),
            reqwest::header::HeaderValue::from_str(value),
        ) {
            headers.insert(header_name, header_value);
        }
    }
    headers
}

async fn response_value(response: reqwest::Response) -> Result<Value, LicenseError> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| LicenseError::Network(error.to_string()))?;
    let value: Value = serde_json::from_str(&body).unwrap_or_else(|_| json!({ "message": body }));
    if !status.is_success() {
        return Err(match status {
            StatusCode::UNAUTHORIZED => LicenseError::Unauthorized,
            StatusCode::FORBIDDEN | StatusCode::CONFLICT => LicenseError::DeviceConflict,
            StatusCode::GONE => LicenseError::SubscriptionExpired,
            StatusCode::TOO_MANY_REQUESTS => {
                LicenseError::Network("permintaan terlalu sering".into())
            }
            _ => LicenseError::Network(
                value
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("server error")
                    .into(),
            ),
        });
    }
    Ok(value)
}

pub fn canonical_json(value: &Value) -> Result<Vec<u8>, LicenseError> {
    serde_json::to_vec(value).map_err(|error| LicenseError::Configuration(error.to_string()))
}

pub fn canonical_lease_bytes(lease: &LeasePayload) -> Vec<u8> {
    let mut value = serde_json::to_value(lease).unwrap_or_else(|_| json!({}));
    if let Some(object) = value.as_object_mut() {
        object.remove("signature");
    }
    serde_json::to_vec(&value).unwrap_or_default()
}

pub fn verify_lease_signature(lease: &LeasePayload, key: &VerifyingKey) -> bool {
    let Ok(bytes) = BASE64.decode(lease.signature.trim()) else {
        return false;
    };
    let Ok(signature) = Signature::from_slice(&bytes) else {
        return false;
    };
    key.verify(&canonical_lease_bytes(lease), &signature)
        .is_ok()
}
