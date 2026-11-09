use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::{project, ui};

pub fn run(project_dir: &Path) -> Result<()> {
    let project = project::absolute(project_dir)?;

    if !project.join("anvil.toml").is_file() {
        anyhow::bail!(
            "Anvil project config not found: {}",
            project.join("anvil.toml").display()
        );
    }

    ui::banner("PROJECT CLEANUP", "Remove generated build artifacts");
    ui::key_value("Project", &project.display().to_string());

    let directories = [
        project.join("rust/app_core/target"),
        project.join("android/.gradle"),
        project.join("android/app/build"),
        project.join("uniffi/kotlin"),
    ];

    ui::section("BUILD DIRECTORIES");

    let mut removed = 0usize;

    for path in directories {
        if path.exists() {
            fs::remove_dir_all(&path).with_context(|| format!("Removing {}", path.display()))?;
            ui::success(&format!("Removed {}", path.display()));
            removed += 1;
        }
    }

    let native_library = project.join("android/app/src/main/jniLibs/arm64-v8a/libapp_core.so");

    ui::section("NATIVE LIBRARIES");

    if native_library.is_file() {
        fs::remove_file(&native_library)
            .with_context(|| format!("Removing {}", native_library.display()))?;
        ui::success(&format!("Removed {}", native_library.display()));
        removed += 1;
    } else {
        ui::info("No packaged native library found");
    }

    ui::section("CLEANUP SUMMARY");
    ui::success(&format!("{removed} artifact(s) removed"));
    ui::key_value("Project", "Source files preserved");
    println!();

    Ok(())
}
