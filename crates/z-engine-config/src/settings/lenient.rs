//! Field adapters accepting every spelling the protocol enums parse (v1 and
//! Claude Code names), so a synonym such as `bypassPermissions` does not
//! reject a whole settings layer.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use z_engine_protocol::{CheckKind, Effort, PermissionMode, VerificationMode};

fn parsed<'de, D, T>(
    deserializer: D,
    parse: fn(&str) -> Option<T>,
    what: &str,
) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    parse(&raw).ok_or_else(|| D::Error::custom(format!("unknown {what} `{raw}`")))
}

pub(crate) fn permission_mode<'de, D: Deserializer<'de>>(d: D) -> Result<PermissionMode, D::Error> {
    parsed(d, PermissionMode::parse, "permission mode")
}

pub(crate) fn verification_mode<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<VerificationMode, D::Error> {
    parsed(d, VerificationMode::parse, "verification mode")
}

pub(crate) fn check_kind<'de, D: Deserializer<'de>>(d: D) -> Result<CheckKind, D::Error> {
    parsed(d, CheckKind::parse, "check kind")
}

pub(crate) fn effort<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Effort>, D::Error> {
    match Option::<String>::deserialize(d)? {
        None => Ok(None),
        Some(raw) => Effort::parse(&raw)
            .map(Some)
            .ok_or_else(|| D::Error::custom(format!("unknown effort `{raw}`"))),
    }
}
