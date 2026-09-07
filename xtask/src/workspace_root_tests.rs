//! Runtime workspace-root discovery boundaries for clean-archive execution.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::workspace_root_from;

static NEXT_TEMPORARY_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    fn new(label: &str) -> Self {
        for _ in 0..128 {
            let serial = NEXT_TEMPORARY_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "franken_alignment_workspace_root_tests-{label}-{}-{serial}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self { path },
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot create owned temporary directory {path:?}: {error}"),
            }
        }
        panic!("unable to create a unique temporary directory");
    }

    fn write(&self, relative: &str, contents: &[u8]) {
        let path = self.path.join(relative);
        fs::create_dir_all(path.parent().expect("fixture path must have a parent"))
            .expect("create fixture parent");
        fs::write(path, contents).expect("write fixture marker");
    }

    fn workspace_markers(&self) {
        self.write("Cargo.toml", b"[workspace]\nmembers = []\n");
        self.write("xtask/Cargo.toml", b"[package]\nname = \"xtask\"\n");
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path)
            .unwrap_or_else(|error| panic!("cannot remove owned temporary directory: {error}"));
    }
}

#[test]
fn runtime_workspace_root_accepts_root_and_immediate_xtask_directory() {
    let workspace = TemporaryDirectory::new("accepted");
    workspace.workspace_markers();
    let expected = fs::canonicalize(&workspace.path).expect("canonicalize owned fixture root");

    assert_eq!(
        workspace_root_from(&workspace.path).expect("workspace root markers must be accepted"),
        expected
    );
    assert_eq!(
        workspace_root_from(&workspace.path.join("xtask"))
            .expect("immediate xtask directory must resolve its workspace root"),
        expected
    );
}

#[test]
fn runtime_workspace_root_refuses_manifestless_and_missing_starts() {
    let manifestless = TemporaryDirectory::new("manifestless");
    let missing = manifestless.path.join("does-not-exist");

    assert!(
        workspace_root_from(&manifestless.path).is_err(),
        "a manifestless directory must not be treated as a workspace root"
    );
    assert!(
        workspace_root_from(&missing).is_err(),
        "a missing start must not be treated as a workspace root"
    );
}

#[test]
fn runtime_workspace_root_never_searches_upward_from_a_sibling_subdirectory() {
    let workspace = TemporaryDirectory::new("no-upward-search");
    workspace.workspace_markers();
    let sibling = workspace.path.join("sibling");
    fs::create_dir(&sibling).expect("create sibling beneath a valid workspace root");

    assert!(
        workspace_root_from(&sibling).is_err(),
        "a sibling subdirectory must be refused even when its parent is a valid workspace root"
    );
}
