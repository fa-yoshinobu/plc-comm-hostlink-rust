use crate::error::HostLinkError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KvHostLinkPlcProfile {
    pub name: &'static str,
    pub display_name: &'static str,
    pub(crate) source_label: &'static str,
}

const PROFILES: &[KvHostLinkPlcProfile] = &[
    KvHostLinkPlcProfile {
        name: "keyence:kv-nano",
        display_name: "KEYENCE KV-NANO",
        source_label: "KV-NANO",
    },
    KvHostLinkPlcProfile {
        name: "keyence:kv-nano-xym",
        display_name: "KEYENCE KV-NANO (XYM)",
        source_label: "KV-NANO(XYM)",
    },
    KvHostLinkPlcProfile {
        name: "keyence:kv-3000",
        display_name: "KEYENCE KV-3000",
        source_label: "KV-3000",
    },
    KvHostLinkPlcProfile {
        name: "keyence:kv-3000-xym",
        display_name: "KEYENCE KV-3000 (XYM)",
        source_label: "KV-3000(XYM)",
    },
    KvHostLinkPlcProfile {
        name: "keyence:kv-5000",
        display_name: "KEYENCE KV-5000",
        source_label: "KV-5000",
    },
    KvHostLinkPlcProfile {
        name: "keyence:kv-5000-xym",
        display_name: "KEYENCE KV-5000 (XYM)",
        source_label: "KV-5000(XYM)",
    },
    KvHostLinkPlcProfile {
        name: "keyence:kv-7000",
        display_name: "KEYENCE KV-7000",
        source_label: "KV-7000",
    },
    KvHostLinkPlcProfile {
        name: "keyence:kv-7000-xym",
        display_name: "KEYENCE KV-7000 (XYM)",
        source_label: "KV-7000(XYM)",
    },
    KvHostLinkPlcProfile {
        name: "keyence:kv-8000",
        display_name: "KEYENCE KV-8000",
        source_label: "KV-8000",
    },
    KvHostLinkPlcProfile {
        name: "keyence:kv-8000-xym",
        display_name: "KEYENCE KV-8000 (XYM)",
        source_label: "KV-8000(XYM)",
    },
    KvHostLinkPlcProfile {
        name: "keyence:kv-x500",
        display_name: "KEYENCE KV-X500",
        source_label: "KV-X500",
    },
    KvHostLinkPlcProfile {
        name: "keyence:kv-x500-xym",
        display_name: "KEYENCE KV-X500 (XYM)",
        source_label: "KV-X500(XYM)",
    },
];

pub fn available_plc_profiles() -> Vec<String> {
    PROFILES
        .iter()
        .map(|profile| profile.name.to_owned())
        .collect()
}

pub fn display_name(plc_profile: impl AsRef<str>) -> Result<&'static str, HostLinkError> {
    Ok(profile_from_name(plc_profile)?.display_name)
}

pub fn profile_from_name(
    plc_profile: impl AsRef<str>,
) -> Result<&'static KvHostLinkPlcProfile, HostLinkError> {
    let normalized = normalize_plc_profile_text(plc_profile.as_ref());
    if normalized.is_empty() {
        return Err(HostLinkError::protocol("PLC profile must not be empty"));
    }
    PROFILES
        .iter()
        .find(|profile| profile.name == normalized)
        .ok_or_else(|| {
            let supported = available_plc_profiles().join(", ");
            HostLinkError::protocol(format!(
                "Unsupported PLC profile '{}'. Supported PLC profiles: {supported}.",
                plc_profile.as_ref()
            ))
        })
}

pub fn normalize_plc_profile(plc_profile: impl AsRef<str>) -> Result<String, HostLinkError> {
    Ok(profile_from_name(plc_profile)?.name.to_owned())
}

pub(crate) fn profiles() -> &'static [KvHostLinkPlcProfile] {
    PROFILES
}

fn normalize_plc_profile_text(text: &str) -> String {
    text.trim().trim_end_matches('\0').to_owned()
}
