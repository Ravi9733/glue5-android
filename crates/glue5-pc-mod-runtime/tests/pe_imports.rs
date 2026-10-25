use glue5_pc_mod_runtime::{
    inspect_pe_imports, plan_compatibility, Architecture, DependencyGroup, PeParseError,
};

fn put_u16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// Minimal PE32+ fixture with one .idata-like section and one imported DLL.
fn pe64_with_import(dll: &str) -> Vec<u8> {
    let pe = 0x40;
    let coff = pe + 4;
    let optional = coff + 20;
    let optional_size = 0xf0;
    let section = optional + optional_size;
    let raw = section + 40;
    let raw_size = 0x200;
    let mut bytes = vec![0u8; raw + raw_size];

    bytes[0..2].copy_from_slice(b"MZ");
    put_u32(&mut bytes, 0x3c, pe as u32);
    bytes[pe..pe + 4].copy_from_slice(b"PE\0\0");
    put_u16(&mut bytes, coff, 0x8664); // AMD64
    put_u16(&mut bytes, coff + 2, 1); // section count
    put_u16(&mut bytes, coff + 16, optional_size as u16);
    put_u16(&mut bytes, optional, 0x20b); // PE32+
    put_u32(&mut bytes, optional + 108, 16); // data-directory count
    put_u32(&mut bytes, optional + 112 + 8, 0x1000); // import directory RVA
    put_u32(&mut bytes, optional + 112 + 12, 40); // two descriptors incl. terminator

    put_u32(&mut bytes, section + 8, raw_size as u32); // virtual size
    put_u32(&mut bytes, section + 12, 0x1000); // virtual address
    put_u32(&mut bytes, section + 16, raw_size as u32); // raw size
    put_u32(&mut bytes, section + 20, raw as u32); // raw file pointer

    // IMAGE_IMPORT_DESCRIPTOR.Name points at RVA 0x1040; following descriptor is zeroed.
    put_u32(&mut bytes, raw + 12, 0x1040);
    bytes[raw + 0x40..raw + 0x40 + dll.len()].copy_from_slice(dll.as_bytes());
    bytes[raw + 0x40 + dll.len()] = 0;
    bytes
}

#[test]
fn detects_scripthookv_and_reports_it_as_a_blocker_not_as_supported() {
    let bytes = pe64_with_import("ScriptHookV.dll");
    let report = inspect_pe_imports(&bytes).expect("valid fixture");
    assert_eq!(report.architecture, Architecture::X86_64);
    assert!(report.is_pe32_plus);
    assert!(report.uses_scripthookv);
    assert_eq!(report.imported_dlls, ["ScriptHookV.dll"]);

    let plan = plan_compatibility(&report);
    assert!(plan.groups.contains(&DependencyGroup::ScriptHookV));
    assert!(!plan.executable_here);
    assert!(plan.blockers.iter().any(|blocker| blocker.contains("ScriptHookV")));
}

#[test]
fn classifies_windows_runtime_imports_without_claiming_execution_support() {
    let report = inspect_pe_imports(&pe64_with_import("kernel32.dll")).expect("valid fixture");
    let plan = plan_compatibility(&report);
    assert!(plan.groups.contains(&DependencyGroup::WindowsKernel));
    assert!(!plan.executable_here);
}

#[test]
fn rejects_a_truncated_or_non_pe_file() {
    assert_eq!(inspect_pe_imports(b"not a PE").unwrap_err(), PeParseError::NotPe);
    let mut bytes = pe64_with_import("kernel32.dll");
    bytes.truncate(0x80);
    assert!(inspect_pe_imports(&bytes).is_err());
}
