use crate::{generator, project, ui};
use anyhow::{bail, Context, Result};
use include_dir::DirEntry;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Default)]
struct SyncSummary {
    added: usize,
    unchanged: usize,
    conflicts: Vec<PathBuf>,
}

fn project_identity(root: &Path) -> Result<(String, String)> {
    let config_path = root.join("anvil.toml");
    let content = fs::read_to_string(&config_path)
        .with_context(|| format!("Reading {}", config_path.display()))?;
    let config: toml::Value =
        toml::from_str(&content).context("Invalid anvil.toml configuration")?;

    let project = config
        .get("project")
        .context("Missing [project] in anvil.toml")?;

    let template = project
        .get("template")
        .and_then(toml::Value::as_str)
        .unwrap_or_default();

    if template != "android-rust" {
        bail!("Unsupported Anvil template: {template}");
    }

    let name = project
        .get("name")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("Missing [project].name in anvil.toml")?;

    let package = project
        .get("package")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("Missing [project].package in anvil.toml")?;

    Ok((name.to_owned(), package.to_owned()))
}

fn sync_entry(
    entry: &DirEntry<'_>,
    root: &Path,
    name: &str,
    package: &str,
    summary: &mut SyncSummary,
) -> Result<()> {
    let relative = entry.path();

    if relative.components().any(|component| {
        matches!(
            component,
            Component::Normal(value) if value == "target" || value == ".git"
        )
    }) {
        return Ok(());
    }

    let destination = generator::destination_path(relative, root, package)?;

    if let Some(directory) = entry.as_dir() {
        for child in directory.entries() {
            sync_entry(child, root, name, package, summary)?;
        }
        return Ok(());
    }

    let file = entry
        .as_file()
        .context("Unsupported entry in embedded Android template")?;

    let desired = match std::str::from_utf8(file.contents()) {
        Ok(content) => generator::render_template(content, name, package)
            .with_context(|| format!("Rendering template file: {}", relative.display()))?
            .into_bytes(),
        Err(_) => file.contents().to_vec(),
    };

    if destination.exists() {
        let current =
            fs::read(&destination).with_context(|| format!("Reading {}", destination.display()))?;

        if current == desired {
            summary.unchanged += 1;
        } else {
            summary.conflicts.push(relative.to_path_buf());
        }

        return Ok(());
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).with_context(|| format!("Creating {}", parent.display()))?;
    }

    fs::write(&destination, desired)
        .with_context(|| format!("Writing {}", destination.display()))?;

    if destination
        .file_name()
        .is_some_and(|file| file == "gradlew")
    {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            fs::set_permissions(&destination, fs::Permissions::from_mode(0o755)).with_context(
                || format!("Setting executable permission: {}", destination.display()),
            )?;
        }
    }

    summary.added += 1;
    Ok(())
}

fn sync_template(root: &Path, name: &str, package: &str) -> Result<SyncSummary> {
    let mut summary = SyncSummary::default();

    for entry in generator::TEMPLATE.entries() {
        sync_entry(entry, root, name, package, &mut summary)
            .context("Synchronizing embedded Anvil template")?;
    }

    Ok(summary)
}

pub fn run(project_dir: &Path) -> Result<()> {
    let root = project::absolute(project_dir)?;

    if !root.is_dir() {
        bail!("Project directory does not exist: {}", root.display());
    }

    let (name, package) = project_identity(&root)?;
    let summary = sync_template(&root, &name, &package)?;

    ui::banner("PROJECT SYNC", "Checking embedded Anvil template updates");
    ui::key_value("Project", &name);
    ui::key_value("Path", &root.display().to_string());

    ui::section("SYNC SUMMARY");
    ui::success(&format!("Added: {}", summary.added));
    ui::info(&format!("Already up to date: {}", summary.unchanged));

    if summary.conflicts.is_empty() {
        ui::success("No conflicts detected. Existing files were preserved.");
    } else {
        ui::warning(&format!(
            "Skipped {} modified existing file(s):",
            summary.conflicts.len()
        ));

        for path in &summary.conflicts {
            ui::warning(&path.display().to_string());
        }

        ui::info("Review these files manually; Anvil did not overwrite them.");
    }

    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    fn test_project() -> PathBuf {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("anvil-sync-test-{}-{id}", std::process::id()));

        fs::create_dir_all(&root).unwrap();
        let config = generator::TEMPLATE
            .get_file("anvil.toml")
            .expect("Embedded template must contain anvil.toml");

        let content = std::str::from_utf8(config.contents()).unwrap();
        let rendered =
            generator::render_template(content, "SyncTest", "com.example.synctest").unwrap();

        fs::write(root.join("anvil.toml"), rendered).unwrap();

        root
    }

    fn remove_test_project(root: &Path) {
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sync_adds_missing_template_files() {
        let root = test_project();

        let summary = sync_template(&root, "SyncTest", "com.example.synctest").unwrap();

        assert!(summary.added > 0);
        assert!(root.join(".gitignore").is_file());
        assert!(summary.conflicts.is_empty());

        remove_test_project(&root);
    }

    #[test]
    fn sync_preserves_modified_existing_files() {
        let root = test_project();
        let custom_content = "# Developer customization\n";
        fs::write(root.join(".gitignore"), custom_content).unwrap();

        let summary = sync_template(&root, "SyncTest", "com.example.synctest").unwrap();

        assert_eq!(
            fs::read_to_string(root.join(".gitignore")).unwrap(),
            custom_content
        );
        assert!(summary.conflicts.contains(&PathBuf::from(".gitignore")));

        remove_test_project(&root);
    }

    #[test]
    fn sync_is_idempotent() {
        let root = test_project();

        let first = sync_template(&root, "SyncTest", "com.example.synctest").unwrap();
        let second = sync_template(&root, "SyncTest", "com.example.synctest").unwrap();

        assert!(first.added > 0);
        assert_eq!(second.added, 0);
        assert!(second.conflicts.is_empty());
        assert_eq!(second.unchanged, first.added + first.unchanged);

        remove_test_project(&root);
    }

    #[test]
    fn sync_restores_deleted_template_files() {
        let root = test_project();
        let gitignore = root.join(".gitignore");

        let first = sync_template(&root, "SyncTest", "com.example.synctest").unwrap();
        assert!(first.added > 0);
        assert!(gitignore.is_file());

        fs::remove_file(&gitignore).unwrap();

        let second = sync_template(&root, "SyncTest", "com.example.synctest").unwrap();

        assert_eq!(second.added, 1);
        assert!(gitignore.is_file());
        assert!(second.conflicts.is_empty());

        let template = generator::TEMPLATE
            .get_file(".gitignore")
            .expect("Embedded template must contain .gitignore");

        assert_eq!(fs::read(&gitignore).unwrap(), template.contents());

        remove_test_project(&root);
    }

    #[test]
    fn rejects_unsupported_project_template() {
        let root = test_project();
        fs::write(
            root.join("anvil.toml"),
            r#"
[project]
name = "SyncTest"
package = "com.example.synctest"
template = "unsupported"
"#,
        )
        .unwrap();

        assert!(project_identity(&root).is_err());

        remove_test_project(&root);
    }
}
