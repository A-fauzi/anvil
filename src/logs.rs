use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

use crate::{project, ui};

pub fn run(project_dir: &Path) -> Result<()> {
    let package = project::package_name(project_dir)?;
    let adb = project::adb_path();

    ui::banner("ANDROID LOGS", "Live application process output");
    ui::key_value("Package", &package);

    ui::section("DEVICE DETECTION");

    let devices = Command::new(&adb)
        .arg("devices")
        .output()
        .context("Unable to execute adb. Check Android platform-tools installation.")?;

    if !devices.status.success() {
        let detail = String::from_utf8_lossy(&devices.stderr);
        bail!("ADB device detection failed: {}", detail.trim());
    }

    let output = String::from_utf8_lossy(&devices.stdout);

    let connected: Vec<_> = output
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            Some((fields.next()?.to_owned(), fields.next()?.to_owned()))
        })
        .collect();

    let authorized: Vec<_> = connected
        .iter()
        .filter(|(_, state)| state == "device")
        .collect();

    if authorized.is_empty() {
        for (id, state) in &connected {
            ui::warning(&format!("{id} — {state}"));
        }

        if connected.is_empty() {
            bail!("No Android device detected. Connect a device and verify with `adb devices`.");
        }

        bail!(
            "No authorized Android device detected. Unlock your device and approve the USB debugging prompt."
        );
    }

    for (id, _) in &authorized {
        ui::status(true, id, "authorized");
    }

    ui::section("APPLICATION PROCESS");

    let process = Command::new(&adb)
        .args(["shell", "pidof", "-s", &package])
        .output()
        .context("Unable to query Android application process through adb.")?;

    let detail = String::from_utf8_lossy(&process.stderr);
    let pid = String::from_utf8_lossy(&process.stdout).trim().to_owned();

    if pid.is_empty() {
        if !detail.trim().is_empty() {
            bail!("Unable to query application process: {}", detail.trim());
        }

        bail!(
            "Application is not running: {}
Launch it on the device first, or run `anvil dev`.",
            package
        );
    }

    ui::status(true, "Process", &format!("PID {pid}"));

    ui::section("LIVE LOG STREAM");
    ui::key_value("Filter", &format!("PID {pid}"));
    ui::key_value("Format", "Threadtime + colored priorities");
    ui::info("Press Ctrl+C to stop the log stream");
    println!();

    let status = Command::new(&adb)
        .args(["logcat", "--pid", &pid, "--format=threadtime,color"])
        .status()
        .context("Unable to start adb logcat. Check your device connection.")?;

    if !status.success() {
        bail!("ADB logcat exited with status: {status}");
    }

    Ok(())
}
