use crate::{project, ui};
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

pub fn run(project_dir: &Path, verbose: bool) -> Result<()> {
    let project = project::absolute(project_dir)?;
    let apk = project.join("android/app/build/outputs/apk/debug/app-debug.apk");

    ui::banner(
        "ANDROID INSTALL",
        "Deploy your application to a connected device",
    );
    ui::key_value("Project", &project.display().to_string());

    if !apk.is_file() {
        bail!(
            "Debug APK not found: {}. Run `anvil build` first.",
            apk.display()
        );
    }

    let adb = project::adb_path();

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

    for (id, _) in authorized {
        ui::status(true, id, "authorized");
    }

    ui::section("INSTALLATION");

    let output = ui::run_step_capture(
        Command::new(adb).arg("install").arg("-r").arg(&apk),
        "Install application on device",
        verbose,
    )?;

    if !output.status.success() {
        let detail = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        if detail.contains("INSTALL_FAILED_USER_RESTRICTED") {
            bail!(
                "Android blocked the installation because it was canceled or restricted by the device.\n\
                 Unlock the phone, approve the installation prompt, and check the device's USB installation permission."
            );
        }

        if detail.contains("INSTALL_FAILED_INSUFFICIENT_STORAGE") {
            bail!("Installation failed: the Android device does not have enough free storage.");
        }

        if detail.contains("INSTALL_FAILED_UPDATE_INCOMPATIBLE") {
            bail!(
                "Installation failed because the installed app has a different signing key. \
                 Uninstall the existing app before installing this build (this removes its local app data)."
            );
        }

        if detail.contains("INSTALL_FAILED_VERSION_DOWNGRADE") {
            bail!(
                "Installation failed because this APK has a lower version code than the installed app. \
                 Increase the app version or uninstall the existing app."
            );
        }

        if detail.contains("INSTALL_FAILED_NO_MATCHING_ABIS") {
            bail!(
                "Installation failed: this APK does not contain a CPU architecture supported by the device."
            );
        }

        bail!(
            "Application installation failed ({}). Re-run with `--verbose` for complete ADB output.",
            output.status
        );
    }

    ui::success("Installation completed successfully");
    println!();

    Ok(())
}
