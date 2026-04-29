use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use xluau::{Compiler, package_manager::PackageManager};

fn assert_package_fixture(project: &str, entry_file: &str) {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_dir = repo_root.join("tests").join("package_projects").join(project);
    let temp_root = temp_dir(project);
    copy_dir(&fixture_dir, &temp_root);

    let package_root = repo_root.join("xluau-json");
    let registry_path = repo_root.join("XLpkg").join("index.json");
    let config = format!(
        r#"{{
  "include": ["src/**/*.xl"],
  "outDir": "out",
  "registry": "{}",
  "packages": {{
    "json": "file:{}"
  }}
}}"#,
        escape_json_path(&registry_path),
        escape_json_path(&package_root)
    );
    fs::write(temp_root.join("xluau.config.json"), config).expect("write config");

    let manager = PackageManager::discover(&temp_root).expect("package manager");
    manager.install_all().expect("install packages");

    let source_path = temp_root.join(entry_file);
    let expected_path = fixture_dir.join("expected.luau");
    let expected = fs::read_to_string(&expected_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", expected_path.display()));

    let compiler = Compiler::discover(&temp_root).expect("compiler");
    let artifact = compiler.build_file(&source_path).expect("build artifact");

    assert_eq!(
        expected,
        artifact.luau,
        "build_file output mismatch for fixture {}",
        display_relative(&repo_root, &fixture_dir.join(entry_file))
    );

    let bundle = fs::read_to_string(temp_root.join("packages.luau")).expect("packages bundle");
    assert!(
        bundle.contains("json = _xluau_json"),
        "expected generated package bundle for fixture {} to expose json package",
        display_relative(&repo_root, &fixture_dir.join(entry_file))
    );
}

fn temp_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("xluau_package_fixture_{name}_{nonce}"));
    fs::create_dir_all(&root).expect("temp dir");
    root
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("fixture root");
    for entry in fs::read_dir(from).expect("read fixture dir") {
        let entry = entry.expect("fixture entry");
        let source = entry.path();
        let dest = to.join(entry.file_name());
        if source.is_dir() {
            copy_dir(&source, &dest);
        } else {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).expect("fixture parent");
            }
            fs::copy(&source, &dest).unwrap_or_else(|error| {
                panic!(
                    "failed to copy fixture file {} -> {}: {error}",
                    source.display(),
                    dest.display()
                )
            });
        }
    }
}

fn escape_json_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

fn display_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[test]
fn package_project_json_consumer() {
    assert_package_fixture("json_consumer", "src/main.xl");
}
