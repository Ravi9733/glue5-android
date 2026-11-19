//! Conservative compatibility gate for third-party GTA V vehicle DLC.
//!
//! This classifies inspected package contents only. It never attempts to mount
//! an RPF, alter the game's dlclist, or claim that a vehicle can be spawned.

use crate::vehicle_dlc::DlcMetadataReport;
use crate::vehicle_inspect::{RpfEncryption, VehiclePackageReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlcCompatibility {
    NoRpfArchive,
    RpfHeaderInvalid,
    EncryptedArchiveUnsupported,
    MissingDlcMetadata,
    MissingDlcListRegistration,
    OpenRpfMetadataPresent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DlcCompatibilityReport {
    pub status: DlcCompatibility,
    pub rpf_count: usize,
    pub open_rpf_count: usize,
    pub encrypted_or_unknown_count: usize,
    pub metadata_complete: bool,
    /// Always false until native mount + model registration + spawn are verified.
    pub runtime_mount_supported: bool,
    pub explanation: String,
}

pub fn assess_dlc_compatibility(
    package: &VehiclePackageReport,
    metadata: &DlcMetadataReport,
    expected_dlc_id: &str,
) -> DlcCompatibilityReport {
    let open = package.rpf_files.iter()
        .filter(|r| r.encryption == RpfEncryption::Open).count();
    let invalid = package.rpf_files.iter()
        .filter(|r| r.encryption == RpfEncryption::NotRpf7).count();
    let encrypted_or_unknown = package.rpf_files.len().saturating_sub(open + invalid);
    let has_setup_and_content = !metadata.setup_orders.is_empty()
        && !metadata.content_xml_files.is_empty();
    let dlclist_registered = metadata.dlclist_entries.iter()
        .any(|id| id.eq_ignore_ascii_case(expected_dlc_id));
    let metadata_complete = has_setup_and_content && dlclist_registered;

    let (status, explanation) = if package.rpf_files.is_empty() {
        (DlcCompatibility::NoRpfArchive,
         "No RPF archive was found; this package is not an RPF/DLC candidate.".to_string())
    } else if invalid > 0 {
        (DlcCompatibility::RpfHeaderInvalid,
         format!("{invalid} archive(s) do not have a recognized GTA V RPF7 header."))
    } else if encrypted_or_unknown > 0 {
        (DlcCompatibility::EncryptedArchiveUnsupported,
         format!("{encrypted_or_unknown} archive(s) are AES-encrypted or have an unknown encryption marker; runtime support is not established."))
    } else if !has_setup_and_content {
        (DlcCompatibility::MissingDlcMetadata,
         "RPF headers are OPEN, but setup2.xml or content.xml metadata is missing.".to_string())
    } else if !dlclist_registered {
        (DlcCompatibility::MissingDlcListRegistration,
         format!("DLC metadata is present, but dlclist.xml does not register the expected package id '{expected_dlc_id}'."))
    } else {
        (DlcCompatibility::OpenRpfMetadataPresent,
         "All detected RPF archives have OPEN markers and setup2.xml/content.xml were found. Native mount and spawn are still unverified.".to_string())
    };

    DlcCompatibilityReport {
        status,
        rpf_count: package.rpf_files.len(),
        open_rpf_count: open,
        encrypted_or_unknown_count: encrypted_or_unknown,
        metadata_complete,
        runtime_mount_supported: false,
        explanation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vehicle_inspect::RpfArchiveInfo;

    fn package(encryption: RpfEncryption) -> VehiclePackageReport {
        VehiclePackageReport {
            files_scanned: 1, total_bytes: 128, rpf_archives: 1,
            rpf7_open: usize::from(encryption == RpfEncryption::Open),
            rpf7_aes: usize::from(encryption == RpfEncryption::Aes),
            rpf7_ng_or_unknown: usize::from(encryption == RpfEncryption::NgOrUnknown),
            rpf_files: vec![RpfArchiveInfo { path: "dlc/dlc.rpf".into(), size_bytes: 128, encryption }],
            vehicle_assets: 0, metadata_files: vec![], blocked_executables: vec![],
            warnings: vec![], spawn_supported: false,
        }
    }

    fn metadata(complete: bool) -> DlcMetadataReport {
        DlcMetadataReport {
            dlclist_entries: vec!["dyfurai".into()],
            setup_orders: if complete { vec![("setup2.xml".into(), 42)] } else { vec![] },
            content_xml_files: if complete { vec!["content.xml".into()] } else { vec![] },
            warnings: vec![],
        }
    }

    #[test]
    fn open_rpf_with_metadata_is_only_a_candidate_not_spawnable() {
        let report = assess_dlc_compatibility(&package(RpfEncryption::Open), &metadata(true), "dyfurai");
        assert_eq!(report.status, DlcCompatibility::OpenRpfMetadataPresent);
        assert!(!report.runtime_mount_supported);
    }

    #[test]
    fn encrypted_or_unknown_archive_is_rejected_from_native_candidate_path() {
        let report = assess_dlc_compatibility(&package(RpfEncryption::Aes), &metadata(true), "dyfurai");
        assert_eq!(report.status, DlcCompatibility::EncryptedArchiveUnsupported);
        assert!(!report.runtime_mount_supported);
    }

    #[test]
    fn requires_dlclist_to_register_the_expected_package_id() {
        let mut metadata = metadata(true);
        metadata.dlclist_entries = vec!["another_car".into()];
        let report = assess_dlc_compatibility(&package(RpfEncryption::Open), &metadata, "dyfurai");
        assert_eq!(report.status, DlcCompatibility::MissingDlcListRegistration);
        assert!(!report.metadata_complete);
        assert!(!report.runtime_mount_supported);
    }

    #[test]
    fn missing_setup_or_content_metadata_is_not_ready() {
        let report = assess_dlc_compatibility(&package(RpfEncryption::Open), &metadata(false), "dyfurai");
        assert_eq!(report.status, DlcCompatibility::MissingDlcMetadata);
    }
}
