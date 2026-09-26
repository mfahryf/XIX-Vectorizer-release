(function (root, factory) {
  // Assign the global unconditionally: bundlers that inject a CommonJS shim
  // (Vite/Rollup) make `typeof module === "object"` true, so an else-only
  // assignment would leave `window.XixLicensingUI` undefined in the bundle.
  const api = factory();
  if (typeof module === "object" && module.exports) {
    module.exports = api;
  }
  root.XixLicensingUI = api;
})(typeof globalThis === "object" ? globalThis : this, function () {
  const ENGINE_IDS = ["vectorize-v1", "vectorize-v2", "pngtosvg"];
  const TRIAL_TOTAL_LIMIT = 10;

  function formatLicenseExpiry(value) {
    const timestamp = Number(value);
    if (!Number.isFinite(timestamp) || timestamp <= 0) return null;
    const date = new Date(timestamp * 1000);
    if (Number.isNaN(date.getTime())) return null;
    return new Intl.DateTimeFormat("en-US", {
      day: "numeric",
      month: "long",
      year: "numeric",
      timeZone: "UTC",
    }).format(date);
  }

  function deriveLicenseView(status) {
    const state = status && status.license_state ? status.license_state : "unavailable";
    const remaining = (status && status.trial_remaining_by_engine) || {};
    const reportedTotal = Number(status && status.trial_remaining);
    const trialRemaining = Number.isFinite(reportedTotal)
      ? Math.max(0, Math.min(TRIAL_TOTAL_LIMIT, reportedTotal))
      : Math.max(
          0,
          Math.min(
            TRIAL_TOTAL_LIMIT,
            Object.values(remaining).reduce((total, value) => total + (Number(value) || 0), 0),
          ),
        );
    const engineCounters = ENGINE_IDS.map((id) => ({
      id,
      remaining: Number.isFinite(Number(remaining[id])) ? Number(remaining[id]) : 0,
    }));
    const hasTrial = trialRemaining > 0;
    const licensed = state === "licensed" || state === "licensed-offline";
    const canProcess = licensed || (state === "trial" && hasTrial);
    const canStart = canProcess || state === "unactivated";
    let badge = "LOCKED";
    let message = "Connect to the internet to validate your license.";
    if (state === "unactivated") {
      badge = "NOT ACTIVATED";
      message = "Process the first file to activate your online trial.";
    } else if (state === "trial") {
      badge = "TRIAL";
      message = hasTrial
        ? `Trial: ${trialRemaining} successful files total.`
        : "Trial exhausted. Activate a license to continue.";
    } else if (state === "licensed") {
      badge = "LICENSED";
      message = "License is active.";
    } else if (state === "licensed-offline") {
      badge = "OFFLINE LICENSE";
      message = "License is active temporarily without a connection.";
    } else if (state === "subscription-expired" || state === "subscription_expired") {
      badge = "EXPIRED";
      message = "Subscription expired. Renew your license to continue.";
    } else if (state === "provider_inactive" || state === "provider-inactive") {
      badge = "MAYAR INACTIVE";
      message = "Mayar license code is not active. Contact XIXLabs for help.";
    } else if (state === "revoked") {
      badge = "REVOKED";
      message = "License revoked. Contact your admin for help.";
    } else if (state === "device-conflict") {
      badge = "DEVICE CONFLICT";
      message = "License is linked to another device. Contact your admin to reset the device.";
    } else if (state === "expired-offline") {
      badge = "RECONNECT";
    } else if (state === "device-identity-lost") {
      badge = "RECOVERY";
      const code = status && status.recovery_request_code;
      const contact = status && status.recovery_contact;
      message = code
        ? `Identity lost. Recovery code: ${code}. ${contact || "Contact your admin."}`
        : "Device identity is lost. Contact your admin for recovery.";
    } else if (state === "clock-rollback") {
      badge = "CLOCK CHECK";
      message = "Device clock has moved backwards. Check the clock, then validate the license.";
    } else if (state === "unavailable") {
      badge = "UNAVAILABLE";
      message = "License status is temporarily unavailable.";
    }
    const helpMessage = licensed ? "" : message;
    return {
      canProcess,
      canStart,
      showActivation: state !== "unavailable" && !licensed,
      badge,
      message,
      helpMessage,
      trialRemaining,
      expiryText: formatLicenseExpiry(status && status.subscription_expires_at),
      engineCounters,
      deviceState: status && status.device_state ? status.device_state : "unknown",
      recoveryRequestCode: status && status.recovery_request_code
        ? status.recovery_request_code
        : null,
      recoveryContact: status && status.recovery_contact ? status.recovery_contact : null,
      offlineDaysRemaining: status && status.offline_days_remaining != null
        ? status.offline_days_remaining
        : null,
    };
  }

  return { deriveLicenseView, ENGINE_IDS };
});
