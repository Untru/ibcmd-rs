//! Exact-passthrough policy for opaque support and signature data.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::profile::EffectiveProfile;
use sha2::{Digest, Sha256};

use super::{BodyProfileError, SelectedBodyProfile};

const LAYOUT_KEY: &str = "bootstrap.body.support.layout";
const LAYOUT: &str = "opaque-exact-passthrough-v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupportPolicyProfile(SelectedBodyProfile);

impl SupportPolicyProfile {
    pub fn from_effective(profile: &EffectiveProfile) -> Result<Self, BodyProfileError> {
        SelectedBodyProfile::from_effective(profile, LAYOUT_KEY, LAYOUT).map(Self)
    }

    pub const fn profile_id(&self) -> &ProfileId {
        self.0.profile_id()
    }

    #[cfg(test)]
    fn fixture(profile_id: &str) -> Self {
        Self(SelectedBodyProfile::fixture(profile_id))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SupportDataKind {
    ParentConfigurations,
    MobileClientSignature,
    StandaloneConfigurationContent,
    VendorSupportData,
}

impl SupportDataKind {
    pub const fn is_signature(self) -> bool {
        matches!(self, Self::MobileClientSignature)
    }
}

/// Bytes captured from an observed artifact together with independently
/// supplied provenance and digest. There is deliberately no constructor for
/// synthetic signatures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedSupportData {
    kind: SupportDataKind,
    source_profile: ProfileId,
    source_path: String,
    bytes: Vec<u8>,
    sha256: String,
}

impl ObservedSupportData {
    pub fn capture_verified(
        profile: &SupportPolicyProfile,
        kind: SupportDataKind,
        source_path: &str,
        bytes: &[u8],
        expected_sha256: &str,
    ) -> Result<Self, SupportDataError> {
        if source_path.is_empty() {
            return Err(SupportDataError::MissingProvenance("source path"));
        }
        if kind.is_signature() && bytes.is_empty() {
            return Err(SupportDataError::InvalidSignature(
                "an observed signature cannot be empty",
            ));
        }
        validate_sha256(expected_sha256)?;
        let actual = sha256_hex(bytes);
        if actual != expected_sha256 {
            return Err(SupportDataError::DigestMismatch {
                expected: expected_sha256.to_owned(),
                actual,
            });
        }
        Ok(Self {
            kind,
            source_profile: profile.profile_id().clone(),
            source_path: source_path.to_owned(),
            bytes: bytes.to_vec(),
            sha256: expected_sha256.to_owned(),
        })
    }

    pub const fn kind(&self) -> SupportDataKind {
        self.kind
    }

    pub const fn source_profile(&self) -> &ProfileId {
        &self.source_profile
    }

    pub fn source_path(&self) -> &str {
        &self.source_path
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupportTransferPolicy {
    Error,
    Drop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SupportTransferOutcome {
    Preserved { bytes: Vec<u8>, sha256: String },
    Dropped(SupportLossReport),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupportLossReport {
    pub kind: SupportDataKind,
    pub source_profile: ProfileId,
    pub target_profile: ProfileId,
    pub source_path: String,
    pub sha256: String,
    pub reason: &'static str,
}

pub fn transfer_support_data(
    target: &SupportPolicyProfile,
    data: &ObservedSupportData,
    policy: SupportTransferPolicy,
) -> Result<SupportTransferOutcome, SupportDataError> {
    let actual = sha256_hex(data.bytes());
    if actual != data.sha256 {
        return Err(SupportDataError::DigestMismatch {
            expected: data.sha256.clone(),
            actual,
        });
    }
    if target.profile_id() == data.source_profile() {
        return Ok(SupportTransferOutcome::Preserved {
            bytes: data.bytes.clone(),
            sha256: data.sha256.clone(),
        });
    }
    let reason = if data.kind.is_signature() {
        "a signature is valid only as byte-identical same-profile passthrough"
    } else {
        "opaque support data has no evidenced cross-profile migration rule"
    };
    match policy {
        SupportTransferPolicy::Error => Err(SupportDataError::CrossProfileBlocked {
            kind: data.kind,
            source: data.source_profile.clone(),
            target: target.profile_id().clone(),
            reason,
        }),
        SupportTransferPolicy::Drop => Ok(SupportTransferOutcome::Dropped(SupportLossReport {
            kind: data.kind,
            source_profile: data.source_profile.clone(),
            target_profile: target.profile_id().clone(),
            source_path: data.source_path.clone(),
            sha256: data.sha256.clone(),
            reason,
        })),
    }
}

fn validate_sha256(value: &str) -> Result<(), SupportDataError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(SupportDataError::InvalidDigest);
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SupportDataError {
    Profile(BodyProfileError),
    MissingProvenance(&'static str),
    InvalidDigest,
    DigestMismatch {
        expected: String,
        actual: String,
    },
    InvalidSignature(&'static str),
    CrossProfileBlocked {
        kind: SupportDataKind,
        source: ProfileId,
        target: ProfileId,
        reason: &'static str,
    },
}

impl Display for SupportDataError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => Display::fmt(source, formatter),
            Self::MissingProvenance(field) => {
                write!(formatter, "opaque support data has no {field}")
            }
            Self::InvalidDigest => formatter
                .write_str("support-data SHA-256 must be 64 lowercase hexadecimal characters"),
            Self::DigestMismatch { expected, actual } => write!(
                formatter,
                "support-data digest mismatch: expected {expected}, actual {actual}"
            ),
            Self::InvalidSignature(reason) => write!(formatter, "invalid signature: {reason}"),
            Self::CrossProfileBlocked {
                kind,
                source,
                target,
                reason,
            } => write!(
                formatter,
                "cross-profile transfer of {kind:?} from `{source}` to `{target}` is blocked: {reason}"
            ),
        }
    }
}

impl Error for SupportDataError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(source) => Some(source),
            _ => None,
        }
    }
}

impl From<BodyProfileError> for SupportDataError {
    fn from(source: BodyProfileError) -> Self {
        Self::Profile(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIGNATURE: &[u8] = b"observed-signature-bytes";
    const SIGNATURE_SHA: &str = "13f47edeadaa921f76369d7891a989bc54268921a35f104ee5d7e1d7e81fb031";

    #[test]
    fn signature_same_profile_passthrough_is_byte_exact() {
        let profile = SupportPolicyProfile::fixture("platform-8.3.27.1989");
        let data = ObservedSupportData::capture_verified(
            &profile,
            SupportDataKind::MobileClientSignature,
            "Ext/MobileClientSignature.bin",
            SIGNATURE,
            SIGNATURE_SHA,
        )
        .unwrap();
        assert_eq!(
            transfer_support_data(&profile, &data, SupportTransferPolicy::Error).unwrap(),
            SupportTransferOutcome::Preserved {
                bytes: SIGNATURE.to_vec(),
                sha256: SIGNATURE_SHA.to_owned(),
            }
        );
    }

    #[test]
    fn signature_is_never_forged_or_silently_dropped() {
        let source = SupportPolicyProfile::fixture("platform-8.3.27.1989");
        let target = SupportPolicyProfile::fixture("platform-future");
        let data = ObservedSupportData::capture_verified(
            &source,
            SupportDataKind::MobileClientSignature,
            "Ext/MobileClientSignature.bin",
            SIGNATURE,
            SIGNATURE_SHA,
        )
        .unwrap();
        assert!(matches!(
            transfer_support_data(&target, &data, SupportTransferPolicy::Error),
            Err(SupportDataError::CrossProfileBlocked { .. })
        ));
        let dropped = transfer_support_data(&target, &data, SupportTransferPolicy::Drop).unwrap();
        let SupportTransferOutcome::Dropped(report) = dropped else {
            panic!("explicit drop must emit a loss report")
        };
        assert_eq!(report.sha256, SIGNATURE_SHA);
        assert_eq!(report.source_path, "Ext/MobileClientSignature.bin");
    }

    #[test]
    fn capture_requires_an_independently_matching_digest() {
        let profile = SupportPolicyProfile::fixture("platform-8.3.27.1989");
        assert!(matches!(
            ObservedSupportData::capture_verified(
                &profile,
                SupportDataKind::VendorSupportData,
                "support.bin",
                b"content",
                &"0".repeat(64),
            ),
            Err(SupportDataError::DigestMismatch { .. })
        ));
    }
}
