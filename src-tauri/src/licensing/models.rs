use super::usage::UsageLedger;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const PRODUCT_ID: &str = "xix-vectorizer";
pub const ENGINE_IDS: [&str; 3] = ["vectorize-v1", "vectorize-v2", "pngtosvg"];
pub const TRIAL_FILE_LIMIT: u8 = 5;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LicenseState {
    Trial,
    Licensed,
    LicensedOffline,
    ExpiredOffline,
    SubscriptionExpired,
    Revoked,
    DeviceConflict,
    DeviceIdentityLost,
    ClockRollback,
    Unavailable,
    Unactivated,
}

impl LicenseState {
    pub fn is_processing_allowed(&self) -> bool {
        matches!(self, Self::Trial | Self::Licensed | Self::LicensedOffline)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineTrial {
    pub successful_files: u8,
    #[serde(default)]
    pub usage_event_ids: BTreeSet<String>,
}

impl Default for EngineTrial {
    fn default() -> Self {
        Self {
            successful_files: 0,
            usage_event_ids: BTreeSet::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrialState {
    pub engines: BTreeMap<String, EngineTrial>,
    pub claimed_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrialToken {
    pub payload: Value,
    pub key_id: String,
    pub signature: String,
}

impl TrialState {
    pub fn new<I, S>(engine_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let engines = engine_ids
            .into_iter()
            .map(|id| (id.as_ref().to_string(), EngineTrial::default()))
            .collect();
        Self {
            engines,
            claimed_at: None,
        }
    }

    pub fn remaining(&self, engine_id: &str) -> u8 {
        self.engines
            .get(engine_id)
            .map(|engine| TRIAL_FILE_LIMIT.saturating_sub(engine.successful_files))
            .unwrap_or(0)
    }

    pub fn is_locked(&self, engine_id: &str) -> bool {
        self.engines.contains_key(engine_id) && self.remaining(engine_id) == 0
    }

    pub fn record_success(&mut self, engine_id: &str, usage_event_id: &str) -> bool {
        let Some(engine) = self.engines.get_mut(engine_id) else {
            return false;
        };
        if engine.usage_event_ids.contains(usage_event_id)
            || engine.successful_files >= TRIAL_FILE_LIMIT
        {
            return false;
        }
        engine.usage_event_ids.insert(usage_event_id.to_string());
        engine.successful_files = engine.successful_files.saturating_add(1);
        true
    }

    pub fn preflight(&self, engine_id: &str, requested_files: usize) -> AccessDecision {
        let Some(_) = self.engines.get(engine_id) else {
            return AccessDecision::denied(engine_id, 0, "engine lisensi tidak dikenal");
        };
        let remaining = self.remaining(engine_id);
        if requested_files == 0 {
            return AccessDecision::denied(engine_id, remaining, "pilih setidaknya satu file");
        }
        if requested_files > remaining as usize {
            return AccessDecision::denied(
                engine_id,
                remaining,
                format!("trial engine ini tersisa {remaining} file berhasil"),
            );
        }
        AccessDecision::allowed(engine_id, remaining)
    }

    pub fn state(&self) -> LicenseState {
        LicenseState::Trial
    }

    pub fn set_remaining_by_engine(&mut self, remaining: &BTreeMap<String, u8>) {
        for (engine_id, engine) in &mut self.engines {
            if let Some(remaining) = remaining.get(engine_id) {
                engine.successful_files =
                    TRIAL_FILE_LIMIT.saturating_sub((*remaining).min(TRIAL_FILE_LIMIT));
            }
        }
    }

    pub fn remaining_by_engine(&self) -> BTreeMap<String, u8> {
        self.engines
            .keys()
            .map(|engine_id| (engine_id.clone(), self.remaining(engine_id)))
            .collect()
    }

    pub fn matches_token(&self, token: &TrialToken) -> bool {
        let Some(counters) = token
            .payload
            .get("trial_remaining_by_engine")
            .and_then(Value::as_object)
        else {
            return false;
        };
        self.engines.iter().all(|(engine_id, engine)| {
            counters
                .get(engine_id)
                .and_then(Value::as_u64)
                .is_some_and(|remaining| {
                    u64::from(TRIAL_FILE_LIMIT.saturating_sub(engine.successful_files)) <= remaining
                })
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessDecision {
    pub allowed: bool,
    pub state: LicenseState,
    pub engine_id: String,
    pub remaining: u8,
    pub message: String,
}

impl AccessDecision {
    pub fn allowed(engine_id: &str, remaining: u8) -> Self {
        Self {
            allowed: true,
            state: LicenseState::Trial,
            engine_id: engine_id.to_string(),
            remaining,
            message: "trial masih tersedia".into(),
        }
    }

    pub fn denied(engine_id: &str, remaining: u8, message: impl Into<String>) -> Self {
        Self {
            allowed: false,
            state: LicenseState::Trial,
            engine_id: engine_id.to_string(),
            remaining,
            message: message.into(),
        }
    }

    pub fn allowed_with_state(engine_id: &str, state: LicenseState) -> Self {
        Self {
            allowed: true,
            state,
            engine_id: engine_id.to_string(),
            remaining: 0,
            message: "lisensi aktif".into(),
        }
    }

    pub fn denied_with_state(
        engine_id: &str,
        state: LicenseState,
        message: impl Into<String>,
    ) -> Self {
        Self {
            allowed: false,
            state,
            engine_id: engine_id.to_string(),
            remaining: 0,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeasePayload {
    pub product_id: String,
    pub device_fingerprint: String,
    pub license_state: String,
    pub subscription_expires_at: i64,
    pub lease_expires_at: i64,
    pub issued_at: i64,
    pub server_time: i64,
    pub key_id: String,
    pub signature: String,
}

impl LeasePayload {
    pub fn is_valid_for(&self, product_id: &str, device_fingerprint: &str, now: i64) -> bool {
        self.product_id == product_id
            && self.device_fingerprint == device_fingerprint
            && self.lease_expires_at > now
            && self.lease_expires_at <= self.subscription_expires_at
            && self.issued_at <= self.server_time
            && matches!(self.license_state.as_str(), "active" | "licensed")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicenseStatus {
    pub license_state: LicenseState,
    pub subscription_expires_at: Option<i64>,
    pub lease_expires_at: Option<i64>,
    pub device_state: String,
    pub trial_remaining_by_engine: BTreeMap<String, u8>,
    pub reason: Option<String>,
    pub server_time: Option<i64>,
    pub key_id: Option<String>,
    pub signature: Option<String>,
    pub license_key_fingerprint: Option<String>,
    pub offline_days_remaining: Option<u16>,
    pub recovery_request_code: Option<String>,
    pub recovery_contact: Option<String>,
}

impl LicenseStatus {
    pub fn unactivated_default() -> Self {
        let mut status = Self::trial_default();
        status.license_state = LicenseState::Unactivated;
        status.device_state = "unregistered".into();
        status
    }

    pub fn trial_default() -> Self {
        let trial = TrialState::new(ENGINE_IDS);
        Self {
            license_state: LicenseState::Trial,
            subscription_expires_at: None,
            lease_expires_at: None,
            device_state: "unregistered".into(),
            trial_remaining_by_engine: trial.remaining_by_engine(),
            reason: None,
            server_time: None,
            key_id: None,
            signature: None,
            license_key_fingerprint: None,
            offline_days_remaining: None,
            recovery_request_code: None,
            recovery_contact: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalLicenseState {
    pub trial: TrialState,
    #[serde(default)]
    pub trial_token: Option<TrialToken>,
    pub lease: Option<LeasePayload>,
    pub lease_verified: bool,
    pub last_server_time: Option<i64>,
    pub license_key_fingerprint: Option<String>,
    pub usage: UsageLedger,
}

impl Default for LocalLicenseState {
    fn default() -> Self {
        Self {
            trial: TrialState::new(ENGINE_IDS),
            trial_token: None,
            lease: None,
            lease_verified: false,
            last_server_time: None,
            license_key_fingerprint: None,
            usage: UsageLedger::default(),
        }
    }
}
