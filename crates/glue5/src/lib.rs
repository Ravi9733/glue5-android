//! Glue 5: the game as one application for every platform — desktop (macOS,
//! Linux, Windows) and mobile (iOS, Android).
//!
//! The engine runs on its own threads ([`glue5_runtime`]) and renders with
//! [`glue5_gpu`]; this crate is the platform side, through SDL: window,
//! input, audio and the application's own settings. One entry point serves
//! every platform:
//!
//! - desktop: the `glue5` binary calls [`main`];
//! - iOS: the Xcode project's `main` calls `glue5_main`, which hands the
//!   process to SDL (`SDL_RunApp` / `UIApplicationMain`);
//! - Android: SDL's Java activity (`platform/android`) loads SDL's library
//!   and this one (built as `libglue5.so`), then calls `SDL_main`.

mod app;
mod audio;
mod benchmark;
mod haptics;
mod keymap;
mod pads;
mod paths;
mod settings;
mod sharing;
mod touch;
mod ui;

use sdl3::messagebox::{MessageBoxFlag, show_simple_message_box};

/// Runs the application; returns the process exit code.
pub fn main() -> i32 {
    #[cfg(not(target_os = "android"))]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    #[cfg(target_os = "android")]
    {
        // RUST_LOG as on desktop (the launch intent's extras, see Glue5Activity).
        let spec = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
        let filter = android_logger::FilterBuilder::new().parse(&spec).build();
        let level = filter.filter();
        android_logger::init_once(android_logger::Config::default().with_tag("glue5").with_max_level(level).with_filter(filter));
    }
    // A host thread serving the engine (the renderer's, ...) that panics
    // leaves it waiting forever, as an engine thread's trap does.
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        report(info);
        if glue5_runtime::exit::is_panic_recoverable() {
            return;
        }
        let thread = std::thread::current();
        let what = info.payload_as_str().unwrap_or("panic");
        glue5_runtime::exit::fail(format!("its {} thread stopped ({what})", thread.name().unwrap_or("unnamed")));
    }));
    match app::run() {
        Ok(()) => 0,
        Err(e) => {
            log::error!("{e:#}");
            let _ = show_simple_message_box(MessageBoxFlag::ERROR, "Glue 5", &format!("{e:#}"), None);
            1
        }
    }
}

#[cfg(any(target_os = "ios", target_os = "tvos"))]
#[unsafe(no_mangle)]
pub extern "C" fn glue5_main(argc: std::ffi::c_int, argv: *mut *mut std::ffi::c_char) -> std::ffi::c_int {
    // SDL's UIKit backend keeps the process running once its main returns
    // (for apps that leave the rest to the run loop), and the engine's
    // threads never end: the window would stay black. Ends it, as desktop does.
    unsafe extern "C" fn sdl_main(_: std::ffi::c_int, _: *mut *mut std::ffi::c_char) -> std::ffi::c_int {
        std::process::exit(main())
    }
    unsafe { sdl3::sys::main::SDL_RunApp(argc, argv, Some(sdl_main), std::ptr::null_mut()) }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn SDL_main(_: std::ffi::c_int, _: *mut *mut std::ffi::c_char) -> std::ffi::c_int {
    // The process ends with main, as on desktop and iOS: the engine's threads
    // never end, and SDL's activity only finishes when main returns, leaving
    // them running in the process Android keeps cached.
    let code = main();
    glue5_runtime::qos::end();
    std::process::exit(code)
}


/// JNI entry point used by the Android OIV importer. It edits only a staged
/// copy of the selected RPF and returns a short status string to Java.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_local_glue5_Glue5Activity_applyOivArchiveNative(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    archive_path: jni::objects::JString,
    archive_xml: jni::objects::JString,
    content_root: jni::objects::JString,
    output_path: jni::objects::JString,
    keys_dir: jni::objects::JString,
) -> jni::sys::jstring {
    let result = (|| -> anyhow::Result<String> {
        let archive_path: String = env.get_string(&archive_path)?.into();
        let archive_xml: String = env.get_string(&archive_xml)?.into();
        let content_root: String = env.get_string(&content_root)?.into();
        let output_path: String = env.get_string(&output_path)?.into();
        let keys_dir: String = env.get_string(&keys_dir)?.into();
        let applied = glue5_runtime::rpf_mod::apply_oiv_rpf_archive(
            std::path::Path::new(&archive_path),
            &archive_xml,
            std::path::Path::new(&content_root),
            std::path::Path::new(&output_path),
            Some(std::path::Path::new(&keys_dir)),
        )?;
        Ok(format!("OK:{applied}"))
    })()
    .unwrap_or_else(|error| format!("ERROR:{error:#}"));

    env.new_string(result)
        .map(|value| value.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
