use crate::{environment::Environment, ui};
use anyhow::Result;
use std::{path::Path, process::Command};

struct Check {
    name: &'static str,
    passed: bool,
    detail: String,
}

fn command_version(path: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new(path).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let output = if stdout.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };

    Some(output.lines().next().unwrap_or("").to_string())
}

fn check_command(name: &'static str, path: Option<&Path>, args: &[&str]) -> Check {
    match path.and_then(|path| command_version(path, args).map(|version| (path, version))) {
        Some((path, version)) => Check {
            name,
            passed: true,
            detail: format!("{version} ({})", path.display()),
        },
        None => Check {
            name,
            passed: false,
            detail: "Not found or unavailable".into(),
        },
    }
}

fn check_path(name: &'static str, path: Option<&Path>) -> Check {
    match path.filter(|path| path.is_dir()) {
        Some(path) => Check {
            name,
            passed: true,
            detail: path.display().to_string(),
        },
        None => Check {
            name,
            passed: false,
            detail: "Not found".into(),
        },
    }
}

fn check_flag(name: &'static str, passed: bool, detail: &'static str) -> Check {
    Check {
        name,
        passed,
        detail: if passed { detail } else { "Not available" }.into(),
    }
}

fn connected_devices(adb: Option<&Path>) -> Vec<(String, String)> {
    let Some(adb) = adb else {
        return Vec::new();
    };

    let output = match Command::new(adb).args(["devices", "-l"]).output() {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let id = fields.next()?;
            let state = fields.next()?;
            (!id.is_empty()).then(|| (id.to_string(), state.to_string()))
        })
        .collect()
}

pub fn run() -> Result<()> {
    ui::banner(
        "ENVIRONMENT DOCTOR",
        "Checking your Android development toolchain",
    );

    let env = Environment::detect();

    let checks = vec![
        check_command("Rust", env.rustc.as_deref(), &["--version"]),
        check_command("Cargo", env.cargo.as_deref(), &["--version"]),
        check_command("Java", env.java.as_deref(), &["-version"]),
        check_path("Android SDK", env.android_sdk.as_deref()),
        check_path("Android NDK", env.android_ndk.as_deref()),
        check_command("ADB", env.adb.as_deref(), &["version"]),
        check_command("SDK Manager", env.sdkmanager.as_deref(), &["--version"]),
        check_flag(
            "Rust Android target",
            env.android_target,
            "aarch64-linux-android installed",
        ),
        check_flag("cargo-ndk", env.cargo_ndk, "cargo-ndk available"),
    ];

    ui::section("TOOLCHAIN");

    let mut passed = 0;
    for check in &checks {
        if check.passed {
            passed += 1;
        }
        ui::status(check.passed, check.name, &check.detail);
    }

    ui::section("BUILD SYSTEM");
    ui::info("Gradle uses the project's Gradle Wrapper");

    if let Some(sdk) = &env.android_sdk {
        let count = std::fs::read_dir(sdk.join("platforms"))
            .map(|entries| entries.filter_map(|entry| entry.ok()).count())
            .unwrap_or(0);

        ui::key_value("Platforms", &format!("{count} installed"));
    }

    ui::section("CONNECTED DEVICES");
    let devices = connected_devices(env.adb.as_deref());

    if devices.is_empty() {
        ui::warning("No Android devices connected");
    } else {
        for (id, state) in &devices {
            match state.as_str() {
                "device" => ui::status(true, id, "ready"),
                "unauthorized" => {
                    ui::warning(&format!("{id} — authorize USB debugging"));
                }
                "offline" => {
                    ui::warning(&format!("{id} — device offline"));
                }
                _ => ui::warning(&format!("{id} — state: {state}")),
            }
        }
    }

    ui::section("READINESS");

    let ready = passed == checks.len();
    ui::status(
        ready,
        "Environment",
        if ready { "READY" } else { "NEEDS ATTENTION" },
    );

    let ready_devices = devices.iter().any(|(_, state)| state == "device");
    ui::status(
        ready_devices,
        "Deployment",
        if ready_devices {
            "READY"
        } else {
            "WAITING FOR DEVICE"
        },
    );

    println!();
    ui::key_value(
        "Toolchain",
        &format!("{passed}/{} checks passed", checks.len()),
    );
    println!();

    Ok(())
}
