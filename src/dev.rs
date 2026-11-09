use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::{build, install, logs, project, ui};

pub fn run(project_dir: &Path, verbose: bool) -> Result<()> {
    let package = project::package_name(project_dir)?;

    ui::banner(
        "ANDROID DEVELOPMENT",
        "Build, deploy, launch and inspect your application",
    );
    ui::key_value("Package", &package);
    ui::key_value(
        "Project",
        &project::absolute(project_dir)?.display().to_string(),
    );

    ui::section("1 / BUILD");
    build::run(project_dir, verbose)?;

    ui::section("2 / INSTALL");
    install::run(project_dir, verbose)?;

    ui::section("3 / LAUNCH");
    ui::run_step(
        Command::new(project::adb_path()).args(["shell", "monkey", "-p", &package, "1"]),
        "Launch Android application",
        verbose,
    )
    .context("Unable to launch Android application")?;

    ui::success("Application launch command completed");

    thread::sleep(Duration::from_secs(2));

    ui::section("4 / LOG STREAM");
    ui::info("Streaming application logs — press Ctrl+C to stop");
    logs::run(project_dir)
}
