use crate::{project, ui};
use anyhow::Result;
use std::env;
use std::path::Path;
use std::process::Command;

struct Check {
    name: &'static str,
    passed: bool,
    detail: String,
}

fn command_version(command: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(command).args(args).output().ok()?;

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

fn check_command(name: &'static str, command: &str, args: &[&str]) -> Check {
    match command_version(command, args) {
        Some(version) => Check {
            name,
            passed: true,
            detail: version,
        },
        None => Check {
            name,
            passed: false,
            detail: format!("{command} not found or unavailable"),
        },
    }
}

fn find_java() -> String {
    if let Ok(java_home) = env::var("JAVA_HOME") {
        let java = Path::new(&java_home).join("bin/java");
        if java.is_file() {
            return java.display().to_string();
        }
    }

    "java".into()
}

fn check_java() -> Check {
    let command = find_java();

    match command_version(&command, &["-version"]) {
        Some(version) => Check {
            name: "Java",
            passed: true,
            detail: format!("{version} ({command})"),
        },
        None => Check {
            name: "Java",
            passed: false,
            detail: format!("{command} not found or unavailable"),
        },
    }
}

fn check_path(name: &'static str, path: Option<String>) -> Check {
    match path {
        Some(value) if Path::new(&value).is_dir() => Check {
            name,
            passed: true,
            detail: value,
        },
        _ => Check {
            name,
            passed: false,
            detail: "Not found".into(),
        },
    }
}

fn find_ndk() -> Option<String> {
    if let Ok(path) = env::var("ANDROID_NDK_HOME") {
        if Path::new(&path).is_dir() {
            return Some(path);
        }
    }

    let sdk = env::var("ANDROID_HOME")
        .or_else(|_| env::var("ANDROID_SDK_ROOT"))
        .ok()?;

    let mut versions: Vec<_> = std::fs::read_dir(Path::new(&sdk).join("ndk"))
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();

    versions.sort();
    versions.last().map(|path| path.display().to_string())
}

fn connected_devices() -> Vec<(String, String)> {
    let output = match Command::new(project::adb_path())
        .args(["devices", "-l"])
        .output()
    {
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

            if id.is_empty() {
                return None;
            }

            Some((id.to_string(), state.to_string()))
        })
        .collect()
}

pub fn run() -> Result<()> {
    ui::banner(
        "ENVIRONMENT DOCTOR",
        "Checking your Android development toolchain",
    );

    let sdk = env::var("ANDROID_HOME")
        .or_else(|_| env::var("ANDROID_SDK_ROOT"))
        .ok();

    let checks = vec![
        check_command("Rust", "rustc", &["--version"]),
        check_command("Cargo", "cargo", &["--version"]),
        check_java(),
        check_path("Android SDK", sdk.clone()),
        check_path("Android NDK", find_ndk()),
        check_command(
            "ADB",
            project::adb_path().to_str().unwrap_or("adb"),
            &["version"],
        ),
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

    if let Some(sdk) = &sdk {
        let platforms = Path::new(sdk).join("platforms");

        let count = std::fs::read_dir(platforms)
            .map(|entries| entries.filter_map(|entry| entry.ok()).count())
            .unwrap_or(0);

        ui::key_value("Platforms", &format!("{count} installed"));
    }

    ui::section("CONNECTED DEVICES");

    let devices = connected_devices();

    if devices.is_empty() {
        ui::warning("No Android devices connected");
    } else {
        for (id, state) in &devices {
            match state.as_str() {
                "device" => ui::status(true, id, "ready"),
                "unauthorized" => {
                    ui::warning(&format!("{id} — authorize USB debugging"));
                }
                "offline" => ui::warning(&format!("{id} — device offline")),
                _ => ui::warning(&format!("{id} — state: {state}")),
            }
        }
    }

    ui::section("READINESS");

    let environment_ready = passed == checks.len();

    ui::status(
        environment_ready,
        "Environment",
        if environment_ready {
            "READY"
        } else {
            "NEEDS ATTENTION"
        },
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
