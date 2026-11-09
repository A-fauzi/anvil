use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Default)]
pub struct Environment {
    pub rustc: Option<PathBuf>,
    pub cargo: Option<PathBuf>,
    pub java: Option<PathBuf>,
    pub android_sdk: Option<PathBuf>,
    pub android_ndk: Option<PathBuf>,
    pub adb: Option<PathBuf>,
    pub sdkmanager: Option<PathBuf>,
    pub cargo_ndk: bool,
    pub android_target: bool,
}

fn executable(path: &Path, args: &[&str]) -> bool {
    path.is_file()
        && Command::new(path)
            .args(args)
            .output()
            .is_ok_and(|output| output.status.success())
}

fn find_in_path(name: &str, args: &[&str]) -> Option<PathBuf> {
    env::split_paths(&env::var_os("PATH")?).find_map(|dir| {
        let path = dir.join(name);
        executable(&path, args).then_some(path)
    })
}

fn find_java() -> Option<PathBuf> {
    if let Some(home) = env::var_os("JAVA_HOME") {
        let path = PathBuf::from(home).join("bin/java");
        if executable(&path, &["-version"]) {
            return Some(path);
        }
    }
    find_in_path("java", &["-version"])
}

fn find_sdk() -> Option<PathBuf> {
    ["ANDROID_HOME", "ANDROID_SDK_ROOT"]
        .iter()
        .filter_map(env::var_os)
        .map(PathBuf::from)
        .find(|path| path.is_dir())
}

fn is_valid_ndk(path: &Path) -> bool {
    if !path.join("source.properties").is_file() {
        return false;
    }

    let Ok(hosts) = fs::read_dir(path.join("toolchains/llvm/prebuilt")) else {
        return false;
    };

    hosts
        .filter_map(Result::ok)
        .any(|host| host.path().join("bin/clang").is_file())
}

fn find_ndk(sdk: Option<&Path>) -> Option<PathBuf> {
    for key in ["ANDROID_NDK_HOME", "NDK_HOME"] {
        if let Some(value) = env::var_os(key) {
            let path = PathBuf::from(value);
            if is_valid_ndk(&path) {
                return Some(path);
            }
        }
    }

    let mut versions: Vec<_> = fs::read_dir(sdk?.join("ndk"))
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_valid_ndk(path))
        .collect();

    versions.sort_by_key(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .split('.')
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    });

    versions.pop()
}

fn find_adb(sdk: Option<&Path>) -> Option<PathBuf> {
    if let Some(sdk) = sdk {
        let path = sdk.join("platform-tools/adb");
        if executable(&path, &["version"]) {
            return Some(path);
        }
    }
    find_in_path("adb", &["version"])
}

pub fn resolve_adb() -> Option<PathBuf> {
    let sdk = find_sdk();
    find_adb(sdk.as_deref())
}

fn find_sdkmanager(sdk: Option<&Path>) -> Option<PathBuf> {
    if let Some(sdk) = sdk {
        let path = sdk.join("cmdline-tools/latest/bin/sdkmanager");
        if executable(&path, &["--version"]) {
            return Some(path);
        }
    }
    find_in_path("sdkmanager", &["--version"])
}

fn has_android_target(rustup: Option<&Path>) -> bool {
    let Some(rustup) = rustup else {
        return false;
    };

    Command::new(rustup)
        .args(["target", "list", "--installed"])
        .output()
        .is_ok_and(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .any(|line| line.trim() == "aarch64-linux-android")
        })
}

impl Environment {
    pub fn detect() -> Self {
        let android_sdk = find_sdk();
        let rustc = find_in_path("rustc", &["--version"]);
        let cargo = find_in_path("cargo", &["--version"]);
        let rustup = find_in_path("rustup", &["--version"]);

        let cargo_ndk = cargo
            .as_ref()
            .is_some_and(|path| executable(path, &["ndk", "--version"]));

        Self {
            rustc,
            cargo,
            java: find_java(),
            android_ndk: find_ndk(android_sdk.as_deref()),
            adb: find_adb(android_sdk.as_deref()),
            sdkmanager: find_sdkmanager(android_sdk.as_deref()),
            cargo_ndk,
            android_target: has_android_target(rustup.as_deref()),
            android_sdk,
        }
    }
}
