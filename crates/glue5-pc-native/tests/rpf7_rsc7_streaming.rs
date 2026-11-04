use flate2::{write::DeflateEncoder, Compression};
use glue5_pc_native::extract_rpf7_rsc7_entry_to_writer;
use std::io::Write;

fn rpf7_resource_fixture() -> (Vec<u8>, Vec<u8>) {
    let expected = vec![0x5A; 512];
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&expected).unwrap();
    let compressed = encoder.finish().unwrap();

    let mut payload = b"RSC7".to_vec();
    payload.extend_from_slice(&1_u32.to_le_bytes());
    payload.extend_from_slice(&(1_u32 << 27).to_le_bytes());
    payload.extend_from_slice(&0_u32.to_le_bytes());
    payload.extend_from_slice(&compressed);

    let mut archive = vec![0_u8; 512 + payload.len()];
    archive[0..4].copy_from_slice(&0x5250_4637_u32.to_le_bytes());
    archive[4..8].copy_from_slice(&2_u32.to_le_bytes());
    archive[8..12].copy_from_slice(&16_u32.to_le_bytes());
    archive[12..16].copy_from_slice(&0x4E45_504F_u32.to_le_bytes());

    // Root directory with one child.
    archive[16..20].copy_from_slice(&0_u32.to_le_bytes());
    archive[20..24].copy_from_slice(&0x7FFF_FF00_u32.to_le_bytes());
    archive[24..28].copy_from_slice(&1_u32.to_le_bytes());
    archive[28..32].copy_from_slice(&1_u32.to_le_bytes());

    // Resource entry: name offset 5, stored at block 1.
    archive[32..34].copy_from_slice(&5_u16.to_le_bytes());
    let payload_len = payload.len() as u32;
    archive[34] = payload_len as u8;
    archive[35] = (payload_len >> 8) as u8;
    archive[36] = (payload_len >> 16) as u8;
    archive[37] = 1;
    archive[39] |= 0x80; // RPF7 resource-entry marker
    archive[40..44].copy_from_slice(&(1_u32 << 27).to_le_bytes());
    archive[44..48].copy_from_slice(&0_u32.to_le_bytes());
    archive[48..64].copy_from_slice(b"root\0sample.rsc\0");
    archive[512..].copy_from_slice(&payload);
    (archive, expected)
}

#[test]
fn toc_indexed_streaming_extracts_only_selected_resource() {
    let (archive, expected) = rpf7_resource_fixture();
    let mut output = Vec::new();
    let report = extract_rpf7_rsc7_entry_to_writer(&archive, 1, &mut output, 1024).unwrap();
    assert_eq!(report.bytes_written, expected.len() as u64);
    assert_eq!(output, expected);
}

#[test]
fn toc_indexed_streaming_rejects_directory_unknown_and_non_resource_entries() {
    let (archive, _) = rpf7_resource_fixture();
    let mut output = Vec::new();
    assert!(extract_rpf7_rsc7_entry_to_writer(&archive, 0, &mut output, 1024).is_err());
    assert!(extract_rpf7_rsc7_entry_to_writer(&archive, 99, &mut output, 1024).is_err());

    let mut binary_archive = archive;
    binary_archive[39] &= 0x7F;
    binary_archive[40..44].copy_from_slice(&24_u32.to_le_bytes());
    binary_archive[44..48].copy_from_slice(&0_u32.to_le_bytes());
    assert!(extract_rpf7_rsc7_entry_to_writer(&binary_archive, 1, &mut output, 1024).is_err());
}

#[test]
fn toc_indexed_streaming_rejects_metadata_overlap_and_output_cap() {
    let (archive, _) = rpf7_resource_fixture();
    let mut output = Vec::new();
    assert!(extract_rpf7_rsc7_entry_to_writer(&archive, 1, &mut output, 128).is_err());

    let mut overlapping = archive;
    overlapping[37..40].copy_from_slice(&[0, 0, 0]);
    assert!(extract_rpf7_rsc7_entry_to_writer(&overlapping, 1, &mut output, 1024).is_err());
}
