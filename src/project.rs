use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

pub fn absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

pub fn package_name(project_dir: &Path) -> Result<String> {
    let project = absolute(project_dir)?;
    let config_path = project.join("anvil.toml");

    let content = std::fs::read_to_string(&config_path)
        .with_context(|| format!("Reading {}", config_path.display()))?;

    let config: toml::Value =
        toml::from_str(&content).context("Invalid anvil.toml configuration")?;

    let package = config
        .get("project")
        .and_then(|value| value.get("package"))
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Missing [project].package in anvil.toml"))?;

    if !package.contains('.') {
        bail!("Invalid Android package name: {package}");
    }

    Ok(package.to_owned())
}

pub fn adb_path() -> PathBuf {
    let sdk = std::env::var_os("ANDROID_HOME").or_else(|| std::env::var_os("ANDROID_SDK_ROOT"));

    if let Some(sdk) = sdk {
        let candidate = PathBuf::from(sdk).join("platform-tools/adb");
        if candidate.is_file() {
            return candidate;
        }
    }

    PathBuf::from("adb")
}
