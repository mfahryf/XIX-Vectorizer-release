pub mod client;
pub mod device;
pub mod error;
pub mod models;
pub mod storage;
pub mod usage;

pub use client::{
    canonical_lease_bytes, verify_lease_signature, LicenseClient, TrialClaimResponse,
};
pub use usage::{UsageLedger, UsageRecord};

pub use device::{DeviceIdentity, DeviceIdentityStore};
pub use models::{
    AccessDecision, LeasePayload, LicenseState, LicenseStatus, LocalLicenseState, TrialState,
};
pub use storage::LicenseStore;

use crate::licensing::error::LicenseError;
use crate::licensing::models::{LicenseStatus as Status, PRODUCT_ID};
use crate::licensing::usage::unix_now;
use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub struct LicenseManager {
    dir: PathBuf,
    store: LicenseStore,
    client: LicenseClient,
    identity: Mutex<Option<DeviceIdentity>>,
    state: Mutex<LocalLicenseState>,
}

impl LicenseManager {
    pub fn production(dir: &Path) -> Result<Self, LicenseError> {
        Self::with_client(dir, LicenseClient::production()?)
    }

    pub fn with_client(dir: &Path, client: LicenseClient) -> Result<Self, LicenseError> {
        let store = LicenseStore::new(dir);
        let state = store.load()?;
        Ok(Self {
            dir: dir.to_path_buf(),
            store,
            client,
            identity: Mutex::new(None),
            state: Mutex::new(state),
        })
    }

    fn identity(&self) -> Result<DeviceIdentity, LicenseError> {
        if let Some(identity) = self.identity.lock().as_ref() {
            return Ok(identity.clone());
        }
        let identity = DeviceIdentityStore::load_or_create(&self.dir)?;
        *self.identity.lock() = Some(identity.clone());
        Ok(identity)
    }

    pub async fn status(&self) -> Result<Status, LicenseError> {
        let identity = match self.identity() {
            Ok(identity) => identity,
            Err(LicenseError::DeviceIdentityLost) => {
                let mut status = Status::trial_default();
                status.license_state = LicenseState::DeviceIdentityLost;
                status.device_state = "identity-lost".into();
                status.reason = Some("buat pemulihan perangkat melalui admin".into());
                return Ok(status);
            }
            Err(error) => return Err(error),
        };
        let local = self.state.lock().clone();
        let now = unix_now();
        let mut status = Status::trial_default();
        status.device_state = if local.lease.is_some() {
            "bound"
        } else {
            "registered"
        }
        .into();
        status.trial_remaining_by_engine = local.trial.remaining_by_engine();
        status.license_key_fingerprint = local.license_key_fingerprint;
        if let Some(lease) = local.lease {
            status.subscription_expires_at = Some(lease.subscription_expires_at);
            status.lease_expires_at = Some(lease.lease_expires_at);
            status.server_time = Some(lease.server_time);
            status.key_id = Some(lease.key_id.clone());
            status.signature = Some(lease.signature.clone());
            if local.lease_verified && lease.is_valid_for(PRODUCT_ID, &identity.fingerprint(), now)
            {
                let recently_online = local
                    .last_server_time
                    .map(|time| now.saturating_sub(time) <= 300)
                    .unwrap_or(false);
                status.license_state = if recently_online {
                    LicenseState::Licensed
                } else {
                    LicenseState::LicensedOffline
                };
                status.offline_days_remaining =
                    Some(((lease.lease_expires_at - now).max(0) as u64).div_ceil(86_400) as u16);
            } else if lease.subscription_expires_at <= now {
                status.license_state = LicenseState::SubscriptionExpired;
                status.reason = Some("perpanjang langganan untuk melanjutkan pemrosesan".into());
            } else if matches!(lease.license_state.as_str(), "revoked" | "revoke") {
                status.license_state = LicenseState::Revoked;
                status.reason = Some("hubungi admin untuk bantuan lisensi".into());
            } else {
                status.license_state = LicenseState::ExpiredOffline;
                status.reason = Some("hubungkan internet untuk memvalidasi lisensi".into());
            }
        }
        Ok(status)
    }

    pub async fn activate(&self, license_key: String) -> Result<Status, LicenseError> {
        let identity = self.identity()?;
        let lease = self
            .client
            .activate(&identity, env!("CARGO_PKG_VERSION"), &license_key)
            .await?;
        {
            let mut state = self.state.lock();
            state.lease_verified = true;
            state.last_server_time = Some(lease.server_time);
            state.license_key_fingerprint = Some(fingerprint_secret(&license_key));
            state.lease = Some(lease);
            self.store.save(&state)?;
        }
        self.status().await
    }

    pub async fn refresh(&self) -> Result<Status, LicenseError> {
        let identity = self.identity()?;
        let lease = if self.state.lock().lease.is_some() {
            self.client
                .renew(&identity, env!("CARGO_PKG_VERSION"))
                .await?
        } else {
            match self
                .client
                .status(&identity, env!("CARGO_PKG_VERSION"))
                .await?
            {
                Some(lease) => lease,
                None => return self.status().await,
            }
        };
        {
            let mut state = self.state.lock();
            state.lease_verified = true;
            state.last_server_time = Some(lease.server_time);
            state.lease = Some(lease);
            self.store.save(&state)?;
        }
        self.status().await
    }

    pub async fn preflight(
        &self,
        engine_id: &str,
        requested_files: usize,
    ) -> Result<AccessDecision, LicenseError> {
        if !models::ENGINE_IDS.contains(&engine_id) {
            return Err(LicenseError::InvalidEngine(engine_id.into()));
        }
        let status = self.status().await?;
        match status.license_state {
            LicenseState::Licensed | LicenseState::LicensedOffline => {
                return Ok(AccessDecision::allowed_with_state(
                    engine_id,
                    status.license_state,
                ));
            }
            LicenseState::Trial => {}
            state => {
                return Ok(AccessDecision::denied_with_state(
                    engine_id,
                    state,
                    status
                        .reason
                        .unwrap_or_else(|| "pemrosesan terkunci".into()),
                ))
            }
        }
        if self.state.lock().trial.claimed_at.is_none() {
            let identity = self.identity()?;
            let claim = self
                .client
                .claim_trial(&identity, env!("CARGO_PKG_VERSION"))
                .await?;
            let mut state = self.state.lock();
            state.trial.claimed_at = Some(claim.claimed_at.unwrap_or_else(unix_now));
            if !claim.trial_remaining_by_engine.is_empty() {
                state
                    .trial
                    .set_remaining_by_engine(&claim.trial_remaining_by_engine);
            }
            self.store.save(&state)?;
        }
        Ok(self
            .state
            .lock()
            .trial
            .preflight(engine_id, requested_files))
    }

    pub fn record_success(
        &self,
        engine_id: &str,
        input: &Path,
        output: &Path,
    ) -> Result<(), LicenseError> {
        let record = UsageRecord::from_paths(engine_id, input, output)?;
        let device_fingerprint = self.identity()?.fingerprint();
        let mut state = self.state.lock();
        if state
            .usage
            .records
            .iter()
            .any(|existing| existing.event_id == record.event_id)
        {
            return Ok(());
        }
        let now = unix_now();
        let paid_active = state.lease_verified
            && state
                .lease
                .as_ref()
                .map(|lease| lease.is_valid_for(PRODUCT_ID, &device_fingerprint, now))
                .unwrap_or(false);
        if !paid_active && !state.trial.record_success(engine_id, &record.event_id) {
            return Err(LicenseError::Locked("trial engine ini sudah habis".into()));
        }
        state.usage.record(record);
        self.store.save(&state)
    }

    pub async fn sync_pending_usage(&self) -> Result<(), LicenseError> {
        let identity = self.identity()?;
        let pending = self.state.lock().usage.pending();
        if pending.is_empty() {
            return Ok(());
        }
        let mut synced = Vec::new();
        for record in pending {
            self.client
                .record_usage(&identity, env!("CARGO_PKG_VERSION"), &record)
                .await?;
            synced.push(record.event_id);
        }
        let mut state = self.state.lock();
        state.usage.mark_synced(synced);
        self.store.save(&state)
    }
}

pub struct LicensingState {
    pub manager: std::sync::Arc<LicenseManager>,
}

impl LicensingState {
    pub fn new(dir: &Path) -> Result<Self, LicenseError> {
        Ok(Self {
            manager: std::sync::Arc::new(LicenseManager::production(dir)?),
        })
    }
}

fn fingerprint_secret(value: &str) -> String {
    let digest = Sha256::digest(value.trim().as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests;
