//! Private fixture roots with room for real Unix socket addresses. RCH's
//! source-scoped TMPDIR is too deep for the unchanged native path limits.
use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Component, Path, PathBuf};

pub(crate) fn create(name: String) -> PathBuf {
    let mut parts = Path::new(&name).components();
    assert!(matches!(parts.next(), Some(Component::Normal(_))) && parts.next().is_none());
    let path = Path::new("/tmp").join(name);
    DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .unwrap_or_else(|error| {
            panic!("create private socket fixture {}: {error}", path.display())
        });
    path
}
