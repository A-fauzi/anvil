use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{environment::Environment, ui};

fn absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

pub fn run(project_dir: &Path, verbose: bool) -> Result<()> {
    let project = absolute(project_dir)?;
    let config = project.join("anvil.toml");
    let android = project.join("android");
    let rust_manifest = project.join("rust/app_core/Cargo.toml");

    if !config.is_file() {
        bail!("Anvil project config not found: {}", config.display());
    }
    if !android.join("gradlew").is_file() {
        bail!(
            "Gradle wrapper not found: {}",
            android.join("gradlew").display()
        );
    }
    if !rust_manifest.is_file() {
        bail!("Rust crate manifest not found: {}", rust_manifest.display());
    }

    let environment = Environment::detect();

    let cargo = environment
        .cargo
        .as_deref()
        .context("Cargo not found. Install Rust before building.")?;

    let java = environment
        .java
        .as_deref()
        .context("Java not found. Install a supported JDK.")?;

    let java_home = java
        .parent()
        .and_then(Path::parent)
        .context("Unable to resolve JAVA_HOME from Java executable")?;

    let android_sdk = environment
        .android_sdk
        .as_deref()
        .context("Android SDK not found. Configure ANDROID_HOME.")?;

    let android_ndk = environment
        .android_ndk
        .as_deref()
        .context("Android NDK not found. Install an Android NDK.")?;

    if !environment.android_target {
        bail!("Rust target aarch64-linux-android is missing. Install it with rustup.");
    }

    if !environment.cargo_ndk {
        bail!("cargo-ndk is missing. Install it with cargo install cargo-ndk.");
    }

    let rust_dir = rust_manifest.parent().context("Invalid Rust crate path")?;
    let library = rust_dir.join("target/aarch64-linux-android/release/libapp_core.so");
    let bindings = project.join("uniffi/kotlin");
    let jni_lib = android.join("app/src/main/jniLibs/arm64-v8a/libapp_core.so");

    ui::banner("ANDROID BUILD", "Forge your Android application");
    ui::key_value("Project", &project.display().to_string());
    ui::key_value("Target", "arm64-v8a");

    let mut progress = ui::BuildProgress::new(3, verbose)?;

    progress.run_step(
        Command::new(cargo)
            .current_dir(rust_dir)
            .env("ANDROID_HOME", android_sdk)
            .env("ANDROID_SDK_ROOT", android_sdk)
            .env("ANDROID_NDK_HOME", android_ndk)
            .env("NDK_HOME", android_ndk)
            .args(["ndk", "-t", "arm64-v8a", "build", "--manifest-path"])
            .arg(&rust_manifest)
            .arg("--release"),
        "Compile Rust for Android",
    )?;

    if !library.is_file() {
        bail!("Rust library was not produced: {}", library.display());
    }

    if bindings.exists() {
        fs::remove_dir_all(&bindings)
            .with_context(|| format!("Cleaning old bindings: {}", bindings.display()))?;
    }

    fs::create_dir_all(&bindings)
        .with_context(|| format!("Creating bindings directory: {}", bindings.display()))?;

    progress.run_step(
        Command::new(cargo)
            .current_dir(rust_dir)
            .env("ANDROID_HOME", android_sdk)
            .env("ANDROID_SDK_ROOT", android_sdk)
            .env("ANDROID_NDK_HOME", android_ndk)
            .env("NDK_HOME", android_ndk)
            .args(["run", "--manifest-path"])
            .arg(&rust_manifest)
            .args([
                "--features",
                "uniffi-cli",
                "--bin",
                "uniffi-bindgen",
                "--",
                "generate",
            ])
            .arg(&library)
            .args(["--language", "kotlin", "--out-dir"])
            .arg(&bindings),
        "Generate Kotlin bindings",
    )?;

    fs::create_dir_all(jni_lib.parent().context("Invalid JNI path")?)
        .context("Creating JNI library directory")?;

    fs::copy(&library, &jni_lib)
        .with_context(|| format!("Packaging Rust library: {}", jni_lib.display()))?;

    progress.run_step(
        Command::new("./gradlew")
            .current_dir(&android)
            .env("JAVA_HOME", java_home)
            .env("ANDROID_HOME", android_sdk)
            .env("ANDROID_SDK_ROOT", android_sdk)
            .arg("assembleDebug"),
        "Build Android debug APK",
    )?;

    let apk = android.join("app/build/outputs/apk/debug/app-debug.apk");

    if !apk.is_file() {
        bail!("APK not found after Gradle build: {}", apk.display());
    }

    progress.finish(&apk)?;
    Ok(())
}
