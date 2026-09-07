use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::check_metadata_paths;

static NEXT_TEMPORARY_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TemporaryWorkspace {
    path: PathBuf,
}

impl TemporaryWorkspace {
    fn new(label: &str) -> Self {
        let serial = NEXT_TEMPORARY_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "franken_alignment_metadata_path_tests-{label}-{}-{serial}",
            std::process::id()
        ));
        // Refuse a pre-existing path; Drop may remove only our own directory.
        fs::create_dir(&path).unwrap();
        Self { path }
    }

    fn create_file(&self, relative: &str) -> PathBuf {
        let path = self.path.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"reference metadata test\n").unwrap();
        path
    }
}

impl Drop for TemporaryWorkspace {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).unwrap();
    }
}

fn json_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn metadata(workspace_root: &Path, manifest_path: &Path, source_path: &Path) -> Vec<u8> {
    format!(
        r#"{{"workspace_root":"{}","packages":[{{"manifest_path":"{}","targets":[{{"src_path":"{}"}}]}}]}}"#,
        json_path(workspace_root),
        json_path(manifest_path),
        json_path(source_path),
    )
    .into_bytes()
}

fn create_workspace(label: &str) -> (TemporaryWorkspace, PathBuf, PathBuf) {
    let workspace = TemporaryWorkspace::new(label);
    let manifest = workspace.create_file("Cargo.toml");
    fs::write(&manifest, b"[workspace]\nresolver = \"2\"\n").unwrap();
    let source = workspace.create_file("src/lib.rs");
    fs::write(&source, b"pub fn metadata_path_fixture() {}\n").unwrap();
    (workspace, manifest, source)
}

#[cfg(unix)]
fn create_file_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_file_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(not(any(unix, windows)))]
fn create_file_symlink(_target: &Path, _link: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "file symlinks are unsupported on this platform",
    ))
}

#[test]
fn metadata_paths_accept_real_workspace_manifest_and_target() {
    let (workspace, manifest, source) = create_workspace("valid");

    assert_eq!(
        check_metadata_paths(
            &workspace.path,
            &metadata(&workspace.path, &manifest, &source)
        ),
        Ok(())
    );
}

#[test]
fn metadata_paths_refuse_target_symlink_escaping_workspace() {
    let (workspace, manifest, _) = create_workspace("symlink-root");
    let outside = TemporaryWorkspace::new("symlink-outside");
    let outside_source = outside.create_file("outside.rs");
    let escaped_target = workspace.path.join("src/escaped.rs");
    fs::create_dir_all(escaped_target.parent().unwrap()).unwrap();
    create_file_symlink(&outside_source, &escaped_target).unwrap();

    let error = check_metadata_paths(
        &workspace.path,
        &metadata(&workspace.path, &manifest, &escaped_target),
    )
    .unwrap_err();

    assert!(error.starts_with("Metadata file escapes workspace or is not a file:"));
}

#[test]
fn metadata_paths_refuse_missing_target_file() {
    let (workspace, manifest, _) = create_workspace("missing-target");
    let missing_target = workspace.path.join("src/missing.rs");

    let error = check_metadata_paths(
        &workspace.path,
        &metadata(&workspace.path, &manifest, &missing_target),
    )
    .unwrap_err();

    assert!(error.starts_with("Cannot resolve metadata file"));
}

#[test]
fn metadata_paths_refuse_metadata_from_different_workspace() {
    let (workspace, _, _) = create_workspace("expected-root");
    let (other_workspace, other_manifest, other_source) = create_workspace("metadata-root");

    assert_eq!(
        check_metadata_paths(
            &workspace.path,
            &metadata(&other_workspace.path, &other_manifest, &other_source),
        ),
        Err("Metadata belongs to a different workspace".into())
    );
}
