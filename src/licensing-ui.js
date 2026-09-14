(function (root, factory) {
  if (typeof module === "object" && module.exports) {
    module.exports = factory();
  } else {
    root.XixLicensingUI = factory();
  }
})(typeof globalThis === "object" ? globalThis : this, function () {
  const ENGINE_IDS = ["vectorize-v1", "vectorize-v2", "pngtosvg"];

  function deriveLicenseView(status) {
    const state = status && status.license_state ? status.license_state : "unavailable";
    const remaining = (status && status.trial_remaining_by_engine) || {};
    const engineCounters = ENGINE_IDS.map((id) => ({
      id,
      remaining: Number.isFinite(Number(remaining[id])) ? Number(remaining[id]) : 0,
    }));
    const hasTrial = engineCounters.some((engine) => engine.remaining > 0);
    const licensed = state === "licensed" || state === "licensed-offline";
    const canProcess = licensed || (state === "trial" && hasTrial);
    let badge = "LOCKED";
    let message = "Hubungkan internet untuk memvalidasi lisensi.";
    if (state === "trial") {
      badge = "TRIAL";
      message = hasTrial ? "Trial: 5 file berhasil per engine." : "Trial habis. Aktifkan lisensi untuk melanjutkan.";
    } else if (state === "licensed") {
      badge = "LICENSED";
      message = "Lisensi aktif.";
    } else if (state === "licensed-offline") {
      badge = "OFFLINE LICENSE";
      message = "Lisensi aktif sementara tanpa koneksi.";
    } else if (state === "subscription-expired") {
      badge = "EXPIRED";
      message = "Langganan berakhir. Perbarui lisensi untuk melanjutkan.";
    } else if (state === "revoked") {
      badge = "REVOKED";
      message = "Lisensi dicabut. Hubungi admin untuk bantuan.";
    } else if (state === "device-conflict") {
      badge = "DEVICE CONFLICT";
      message = "Lisensi terikat ke perangkat lain. Hubungi admin untuk reset perangkat.";
    } else if (state === "expired-offline") {
      badge = "RECONNECT";
    } else if (state === "device-identity-lost") {
      badge = "RECOVERY";
      message = "Identitas perangkat hilang. Hubungi admin untuk pemulihan.";
    }
    return {
      canProcess,
      showActivation: !licensed,
      badge,
      message,
      engineCounters,
      offlineDaysRemaining: status && status.offline_days_remaining != null
        ? status.offline_days_remaining
        : null,
    };
  }

  return { deriveLicenseView, ENGINE_IDS };
});
