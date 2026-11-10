use crate::ui;
use anyhow::{bail, Context, Result};
use include_dir::{include_dir, Dir, DirEntry};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(crate) static TEMPLATE: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/templates/android-rust");

fn valid_project_name(name: &str) -> bool {
    let mut chars = name.chars();

    matches!(chars.next(), Some(c) if c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn package_name(name: &str) -> String {
    let mut slug = name.replace('-', "_").to_ascii_lowercase();

    if slug.starts_with(|c: char| c.is_ascii_digit()) {
        slug = format!("app_{slug}");
    }

    format!("com.example.{slug}")
}

pub(crate) fn render_template(content: &str, name: &str, package: &str) -> Result<String> {
    let rendered = content
        .replace("{{project_name}}", name)
        .replace("{{package_name}}", package);

    if rendered.contains("{{") || rendered.contains("}}") {
        bail!("Unresolved template placeholder: {rendered}");
    }

    Ok(rendered)
}

fn validate_generated_project(root: &Path) -> Result<()> {
    const REQUIRED_FILES: &[&str] = &[
        ".gitignore",
        "anvil.toml",
        "android/gradlew",
        "android/settings.gradle.kts",
        "android/app/build.gradle.kts",
        "android/app/src/main/AndroidManifest.xml",
        "android/app/src/main/java",
        "rust/app_core/Cargo.toml",
        "rust/app_core/src/lib.rs",
        "rust/app_core/uniffi.toml",
    ];

    for relative in REQUIRED_FILES {
        let path = root.join(relative);
        if !path.exists() {
            bail!(
                "Generated project is incomplete: missing {}",
                path.display()
            );
        }
    }

    Ok(())
}

pub(crate) fn destination_path(
    relative: &Path,
    destination: &Path,
    package: &str,
) -> Result<PathBuf> {
    let mut path = destination.to_path_buf();

    for component in relative.components() {
        let Component::Normal(name) = component else {
            bail!("Invalid embedded template path: {}", relative.display());
        };

        let name = name
            .to_string_lossy()
            .replace("{{package_path}}", &package.replace('.', "/"));

        path.push(name);
    }

    Ok(path)
}

fn copy_entry(entry: &DirEntry<'_>, destination: &Path, name: &str, package: &str) -> Result<()> {
    let relative = entry.path();

    if relative.components().any(|component| {
        matches!(component, Component::Normal(value)
            if value == "target" || value == ".git")
    }) {
        return Ok(());
    }

    let output_path = destination_path(relative, destination, package)?;

    if let Some(directory) = entry.as_dir() {
        fs::create_dir_all(&output_path)
            .with_context(|| format!("Creating {}", output_path.display()))?;

        for child in directory.entries() {
            copy_entry(child, destination, name, package)?;
        }

        return Ok(());
    }

    let file = entry
        .as_file()
        .context("Unsupported entry in embedded Android template")?;

    let output = match std::str::from_utf8(file.contents()) {
        Ok(content) => render_template(content, name, package)
            .with_context(|| format!("Rendering template file: {}", relative.display()))?
            .into_bytes(),
        Err(_) => file.contents().to_vec(),
    };

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("Creating {}", parent.display()))?;
    }

    fs::write(&output_path, output)
        .with_context(|| format!("Writing {}", output_path.display()))?;

    if output_path
        .file_name()
        .is_some_and(|file| file == "gradlew")
    {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&output_path, fs::Permissions::from_mode(0o755)).with_context(
                || format!("Setting executable permission: {}", output_path.display()),
            )?;
        }
    }

    Ok(())
}

pub fn run(name: &str) -> Result<()> {
    let project_path = Path::new(name);

    if name.trim().is_empty()
        || !valid_project_name(name)
        || project_path.components().count() != 1
        || !matches!(project_path.components().next(), Some(Component::Normal(_)))
    {
        bail!(
            "Invalid project name: use ASCII letters, numbers, '_' or '-', starting with a letter or number"
        );
    }

    let destination = PathBuf::from(name);

    if destination.exists() {
        bail!("Destination already exists: {}", destination.display());
    }

    let staging = PathBuf::from(format!(".{name}.anvil-tmp"));

    if staging.exists() {
        bail!(
            "Temporary generation directory already exists: {}",
            staging.display()
        );
    }

    let package = package_name(name);

    fs::create_dir_all(&staging)
        .with_context(|| format!("Creating temporary directory: {}", staging.display()))?;

    for entry in TEMPLATE.entries() {
        if let Err(error) = copy_entry(entry, &staging, name, &package) {
            let _ = fs::remove_dir_all(&staging);
            return Err(error).context("Generating Android project from embedded template");
        }
    }

    if let Err(error) = validate_generated_project(&staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error).context("Validating generated Android project");
    }

    if let Err(error) = fs::rename(&staging, &destination) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error)
            .with_context(|| format!("Finalizing generated project at {}", destination.display()));
    }

    ui::banner("PROJECT CREATED", "Your native Android project is ready");
    ui::success("Project generated successfully");
    ui::key_value("Name", name);
    ui::key_value("Package", &package);
    ui::key_value("Path", &destination.display().to_string());

    ui::section("NEXT STEP");
    println!("    anvil doctor");
    println!("    anvil build --project-dir {name}");
    println!();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_project_names() {
        for name in ["MyApp", "my_app", "app-2", "2App"] {
            assert!(valid_project_name(name), "Expected valid: {name}");
        }

        for name in [
            "",
            "../escape",
            "My App",
            "-app",
            "app/name",
            ".hidden",
            "app.name",
        ] {
            assert!(!valid_project_name(name), "Expected invalid: {name}");
        }
    }

    #[test]
    fn generates_package_names() {
        assert_eq!(package_name("MyApp"), "com.example.myapp");
        assert_eq!(package_name("Todo-List"), "com.example.todo_list");
        assert_eq!(package_name("my_app"), "com.example.my_app");
        assert_eq!(package_name("2App"), "com.example.app_2app");
    }

    #[test]
    fn renders_all_supported_placeholders() {
        let rendered = render_template(
            "name={{project_name}} package={{package_name}}",
            "MyApp",
            "com.example.myapp",
        )
        .unwrap();

        assert_eq!(rendered, "name=MyApp package=com.example.myapp");
    }

    #[test]
    fn rejects_unknown_placeholders() {
        assert!(render_template("value={{unknown}}", "MyApp", "com.example.myapp").is_err());
    }

    #[test]
    fn replaces_package_path_in_template_paths() {
        let result = destination_path(
            Path::new("android/app/src/main/java/{{package_path}}/MainActivity.kt"),
            Path::new("/tmp/MyApp"),
            "com.example.myapp",
        )
        .unwrap();

        assert_eq!(
            result,
            PathBuf::from("/tmp/MyApp/android/app/src/main/java/com/example/myapp/MainActivity.kt")
        );
    }
}
