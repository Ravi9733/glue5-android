//! JNI entry point for the Android application.
//! The caller must copy a SAF-selected document to an app-private file first;
//! the native core then extracts into an app-private destination directory.
use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;
use std::ptr;

#[no_mangle]
pub extern "system" fn Java_com_ravi9733_autorpconverter_NativeRpf_extractArchive(
    mut env: JNIEnv,
    _class: JClass,
    source: JString,
    destination: JString,
) -> jstring {
    let result = (|| -> anyhow::Result<String> {
        let source: String = env.get_string(&source)?.into();
        let destination: String = env.get_string(&destination)?.into();
        let report = crate::extract_archive(source, destination, crate::Limits::default())?;
        Ok(format!(
            "Extraction complete\nFiles: {}\nBytes: {}\nOutput: {}",
            report.files_written,
            report.bytes_written,
            report.output_dir.display()
        ))
    })();

    let message = match result {
        Ok(message) => message,
        Err(error) => format!("Extraction failed: {error:#}"),
    };
    env.new_string(message)
        .map(|value| value.into_raw())
        .unwrap_or_else(|_| ptr::null_mut())
}


#[no_mangle]
pub extern "system" fn Java_com_ravi9733_autorpconverter_NativeRpf_buildAndroidRpf7(
    mut env: JNIEnv,
    _class: JClass,
    source_dir: JString,
    output_file: JString,
) -> jstring {
    let result = (|| -> anyhow::Result<String> {
        let source_dir: String = env.get_string(&source_dir)?.into();
        let output_file: String = env.get_string(&output_file)?.into();
        let (files, bytes, ytd) = crate::build_android_rpf7(
            source_dir,
            output_file.clone(),
            crate::Limits::default(),
        )?;
        Ok(format!(
            "RPF7 build complete\nFiles: {}\nArchive bytes: {}\nAdditional YTD conversions during repacking: {}\nOutput: {}\nGen9 asset conversion was performed in the preceding ragekit stage.",
            files, bytes, ytd, output_file
        ))
    })();
    let message = match result {
        Ok(message) => message,
        Err(error) => format!("RPF7 build failed: {error:#}"),
    };
    env.new_string(message)
        .map(|value| value.into_raw())
        .unwrap_or_else(|_| ptr::null_mut())
}
