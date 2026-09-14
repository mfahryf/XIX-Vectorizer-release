use super::*;
use crate::licensing::{
    AccessDecision, LeasePayload, LicenseManager, LicenseState, LicenseStore, LocalLicenseState,
    TrialState, UsageLedger, UsageRecord,
};

#[test]
fn trial_uses_real_engine_ids_and_locks_only_exhausted_engine() {
    let mut trial = TrialState::new(["vectorize-v1", "vectorize-v2", "pngtosvg"]);
    assert_eq!(trial.remaining("pngtosvg"), 5);
    for index in 0..5 {
        assert!(trial.record_success("pngtosvg", &format!("usage-{index}")));
    }
    assert_eq!(trial.remaining("pngtosvg"), 0);
    assert!(trial.is_locked("pngtosvg"));
    assert_eq!(trial.remaining("vectorize-v1"), 5);
    assert_eq!(trial.state(), LicenseState::Trial);
}

#[test]
fn trial_usage_is_idempotent_and_never_goes_negative() {
    let mut trial = TrialState::new(["vectorize-v1"]);
    assert!(trial.record_success("vectorize-v1", "same-event"));
    assert!(!trial.record_success("vectorize-v1", "same-event"));
    for index in 0..10 {
        let _ = trial.record_success("vectorize-v1", &format!("event-{index}"));
    }
    assert_eq!(trial.remaining("vectorize-v1"), 0);
    assert!(trial.is_locked("vectorize-v1"));
}

#[test]
fn preflight_locks_only_when_requested_files_exceed_engine_quota() {
    let mut trial = TrialState::new(["vectorize-v1", "vectorize-v2", "pngtosvg"]);
    for index in 0..3 {
        assert!(trial.record_success("vectorize-v2", &format!("event-{index}")));
    }
    assert!(trial.preflight("vectorize-v2", 2).allowed);
    let denied = trial.preflight("vectorize-v2", 3);
    assert!(!denied.allowed);
    assert_eq!(denied.remaining, 2);
    assert!(trial.preflight("pngtosvg", 5).allowed);
}

#[test]
fn lease_validation_rejects_expired_or_mismatched_payload() {
    let lease = LeasePayload {
        product_id: "xix-vectorizer".into(),
        device_fingerprint: "device-a".into(),
        license_state: "active".into(),
        subscription_expires_at: 2_000,
        lease_expires_at: 1_500,
        issued_at: 1_000,
        server_time: 1_000,
        key_id: "gateway-2026-01".into(),
        signature: String::new(),
    };
    assert!(lease.is_valid_for("xix-vectorizer", "device-a", 1_200));
    assert!(!lease.is_valid_for("xix-vectorizer", "device-a", 1_500));
    assert!(!lease.is_valid_for("other-product", "device-a", 1_200));
    assert!(!lease.is_valid_for("xix-vectorizer", "device-b", 1_200));
}

#[test]
fn canonical_request_signature_changes_when_payload_changes() {
    let identity = DeviceIdentity::generate().expect("identity generation");
    let first = identity.sign_request("nonce-a", br#"{"action":"status"}"#);
    let second = identity.sign_request("nonce-b", br#"{"action":"status"}"#);
    let changed_payload = identity.sign_request("nonce-a", br#"{"action":"renew"}"#);
    assert_ne!(first.signature, second.signature);
    assert_ne!(first.signature, changed_payload.signature);
    assert_eq!(first.public_key, identity.public_key_base64());
}

#[test]
fn status_does_not_expose_private_material() {
    let status = LicenseStatus::trial_default();
    let json = serde_json::to_string(&status).unwrap();
    assert!(!json.contains("private"));
    assert!(!json.contains("secret"));
    assert!(!json.contains("raw_key"));
}

#[test]
fn device_identity_roundtrips_through_protected_storage() {
    let dir = tempfile_dir("identity");
    let first = DeviceIdentityStore::load_or_create(&dir).expect("create identity");
    let second = DeviceIdentityStore::load_or_create(&dir).expect("load identity");
    assert_eq!(first.fingerprint(), second.fingerprint());
    assert_eq!(first.public_key_base64(), second.public_key_base64());
    assert_ne!(
        std::fs::read(dir.join("device-identity.dat")).unwrap(),
        first.private_key_bytes()
    );
}

#[test]
fn usage_ledger_counts_each_successful_input_once_and_queues_sync() {
    let dir = tempfile_dir("usage");
    let input = dir.join("input.png");
    let output = dir.join("output.svg");
    std::fs::write(&input, b"input-content").unwrap();
    std::fs::write(&output, b"output-content").unwrap();

    let mut ledger = UsageLedger::default();
    let first = ledger
        .record_success("vectorize-v1", &input, &output)
        .unwrap();
    let second = ledger
        .record_success("vectorize-v1", &input, &output)
        .unwrap();
    assert!(first);
    assert!(!second);
    assert_eq!(ledger.pending().len(), 1);
    assert_eq!(ledger.pending()[0].engine_id, "vectorize-v1");
    assert!(!ledger.pending()[0].input_fingerprint.is_empty());
}

#[test]
fn usage_record_event_id_is_stable_for_same_file_contents() {
    let dir = tempfile_dir("usage-stable");
    let input = dir.join("input.png");
    let output = dir.join("output.svg");
    std::fs::write(&input, b"same-input").unwrap();
    std::fs::write(&output, b"same-output").unwrap();

    let first = UsageRecord::from_paths("pngtosvg", &input, &output).unwrap();
    let second = UsageRecord::from_paths("pngtosvg", &input, &output).unwrap();
    assert_eq!(first.event_id, second.event_id);
    assert_eq!(first.input_fingerprint, second.input_fingerprint);
}

#[test]
fn lease_signature_verification_rejects_tampering() {
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine;
    use ed25519_dalek::{Signer, SigningKey};
    use rand_core::OsRng;

    let signing_key = SigningKey::generate(&mut OsRng);
    let mut lease = LeasePayload {
        product_id: "xix-vectorizer".into(),
        device_fingerprint: "device-a".into(),
        license_state: "active".into(),
        subscription_expires_at: 2_000,
        lease_expires_at: 1_500,
        issued_at: 1_000,
        server_time: 1_000,
        key_id: "gateway-2026-01".into(),
        signature: String::new(),
    };
    let signature = signing_key.sign(&crate::licensing::canonical_lease_bytes(&lease));
    lease.signature = BASE64.encode(signature.to_bytes());
    assert!(crate::licensing::verify_lease_signature(
        &lease,
        &signing_key.verifying_key()
    ));
    lease.lease_expires_at = 1_400;
    assert!(!crate::licensing::verify_lease_signature(
        &lease,
        &signing_key.verifying_key()
    ));
}

#[test]
fn local_license_state_roundtrips_without_raw_license_key() {
    let dir = tempfile_dir("state");
    let store = LicenseStore::new(&dir);
    let state = LocalLicenseState::default();
    store.save(&state).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded, state);
    let raw = std::fs::read_to_string(store.path()).unwrap();
    assert!(!raw.contains("license_key"));
    assert!(!raw.contains("private_key"));
}

#[tokio::test]
async fn fresh_manager_reports_trial_without_network_call() {
    let dir = tempfile_dir("manager");
    let client = crate::licensing::LicenseClient::new("http://127.0.0.1:1", None).unwrap();
    let manager = LicenseManager::with_client(&dir, client).unwrap();
    let status = manager.status().await.unwrap();
    assert_eq!(status.license_state, LicenseState::Trial);
    assert_eq!(status.trial_remaining_by_engine["vectorize-v1"], 5);
    assert_eq!(status.trial_remaining_by_engine["vectorize-v2"], 5);
    assert_eq!(status.trial_remaining_by_engine["pngtosvg"], 5);
}

#[test]
fn licensed_access_decision_is_not_limited_by_trial_counter() {
    let decision = AccessDecision::allowed_with_state("pngtosvg", LicenseState::Licensed);
    assert!(decision.allowed);
    assert_eq!(decision.state, LicenseState::Licensed);
}

#[test]
fn file_done_contains_input_for_usage_deduplication() {
    let event = crate::batch::BatchEvent::FileDone {
        input: "C:/in/a.png".into(),
        name: "a.png".into(),
        output: "C:/out/a.svg".into(),
    };
    match event {
        crate::batch::BatchEvent::FileDone { input, output, .. } => {
            assert_eq!(input, "C:/in/a.png");
            assert!(output.ends_with("a.svg"));
        }
        _ => unreachable!(),
    }
}

fn tempfile_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "xix-vectorizer-license-{label}-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
