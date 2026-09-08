//! Receipt-pointer and bounded prose-input boundaries.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{current_execution_receipt, read_prose_input};

static NEXT_TEMPORARY_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TemporaryFixture {
    base: PathBuf,
    root: PathBuf,
    outside: PathBuf,
}

impl TemporaryFixture {
    fn new(label: &str) -> Self {
        for _ in 0..128 {
            let serial = NEXT_TEMPORARY_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let base = std::env::temp_dir().join(format!(
                "franken_alignment_prose_input_tests-{label}-{}-{serial}",
                std::process::id()
            ));
            match fs::create_dir(&base) {
                Ok(()) => {
                    let root = base.join("root");
                    let outside = base.join("outside");
                    fs::create_dir(&root).expect("create owned fixture root");
                    fs::create_dir(&outside).expect("create owned fixture outside");
                    return Self {
                        base,
                        root,
                        outside,
                    };
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot create owned temporary directory {base:?}: {error}"),
            }
        }
        panic!("unable to create a unique temporary directory");
    }

    fn write_root(&self, relative: &str, bytes: &[u8]) {
        write_file(&self.root, relative, bytes);
    }

    fn write_outside(&self, relative: &str, bytes: &[u8]) {
        write_file(&self.outside, relative, bytes);
    }
}

impl Drop for TemporaryFixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.base)
            .unwrap_or_else(|error| panic!("cannot remove owned temporary directory: {error}"));
    }
}

fn write_file(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("fixture path must have a parent"))
        .expect("create fixture parent");
    fs::write(path, bytes).expect("write fixture file");
}

#[cfg(unix)]
fn directory_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(unix)]
fn file_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[test]
fn current_execution_receipt_accepts_one_canonical_receipt_pointer() {
    let status = "Current execution evidence: [receipt](artifacts/execution/2026-09-07-reference-epoch-3-receipt.json).";

    assert_eq!(
        current_execution_receipt(status),
        Ok("artifacts/execution/2026-09-07-reference-epoch-3-receipt.json")
    );
}

#[test]
fn current_execution_receipt_refuses_missing_ambiguous_malformed_and_escaping_pointers() {
    for status in [
        "No current execution evidence is declared.",
        "Current execution evidence: [receipt](artifacts/execution/one-receipt.json).\nCurrent execution evidence: [receipt](artifacts/execution/two-receipt.json).",
        "Current execution evidence: [receipt](artifacts/execution/one-receipt.json)",
        "Current execution evidence: [receipt](artifacts/execution/not-a-receipt.txt).",
        "Current execution evidence: [receipt](artifacts/execution/../outside-receipt.json).",
    ] {
        assert!(
            current_execution_receipt(status).is_err(),
            "invalid receipt declaration must be refused: {status}"
        );
    }
}

#[test]
fn current_execution_receipt_ignores_fenced_examples_but_accepts_a_literal_declaration() {
    let fenced_only = "```markdown\nCurrent execution evidence: [receipt](artifacts/execution/example-receipt.json).\n```";
    assert!(
        current_execution_receipt(fenced_only).is_err(),
        "a fenced example must not become the current receipt declaration"
    );

    let with_literal = "```markdown\nCurrent execution evidence: [receipt](artifacts/execution/example-receipt.json).\n```\nCurrent execution evidence: [receipt](artifacts/execution/2026-09-07-reference-epoch-3-receipt.json).";
    assert_eq!(
        current_execution_receipt(with_literal),
        Ok("artifacts/execution/2026-09-07-reference-epoch-3-receipt.json")
    );
}

#[test]
fn review_regression_html_comment_declaration_is_not_current_execution_evidence() {
    let hidden = "<!--\nCurrent execution evidence: [receipt](artifacts/execution/2026-09-07-reference-epoch-3-receipt.json).\n-->";
    assert!(
        current_execution_receipt(hidden).is_err(),
        "an HTML-comment declaration is not rendered current execution evidence"
    );

    let visible = "<!-- historical example -->\nCurrent execution evidence: [receipt](artifacts/execution/2026-09-07-reference-epoch-3-receipt.json).";
    assert_eq!(
        current_execution_receipt(visible),
        Ok("artifacts/execution/2026-09-07-reference-epoch-3-receipt.json"),
        "the paired visible declaration remains the permitted control"
    );
}

#[test]
fn review_regression_indented_declaration_inside_fence_is_not_current_evidence() {
    let indented_code = "    Current execution evidence: [receipt](artifacts/execution/2026-09-07-reference-epoch-3-receipt.json).";
    assert!(
        current_execution_receipt(indented_code).is_err(),
        "an indented code declaration is not current execution evidence"
    );
    let tab_indented_code = "\tCurrent execution evidence: [receipt](artifacts/execution/2026-09-07-reference-epoch-3-receipt.json).";
    assert!(
        current_execution_receipt(tab_indented_code).is_err(),
        "a tab-indented code declaration is not current execution evidence"
    );

    let hidden = "```markdown\n    Current execution evidence: [receipt](artifacts/execution/2026-09-07-reference-epoch-3-receipt.json).\n```";
    assert!(
        current_execution_receipt(hidden).is_err(),
        "an indented declaration inside a fence is not current execution evidence"
    );

    let visible = "```markdown\n    Current execution evidence: [receipt](artifacts/execution/example-receipt.json).\n```\nCurrent execution evidence: [receipt](artifacts/execution/2026-09-07-reference-epoch-3-receipt.json).";
    assert_eq!(
        current_execution_receipt(visible),
        Ok("artifacts/execution/2026-09-07-reference-epoch-3-receipt.json"),
        "the paired literal declaration remains the permitted control"
    );
}

#[test]
fn read_prose_input_returns_a_real_small_contained_file() {
    let fixture = TemporaryFixture::new("small");
    fixture.write_root("docs/input.md", b"real bounded prose\n");

    assert_eq!(
        read_prose_input(&fixture.root, "docs/input.md"),
        Ok(b"real bounded prose\n".to_vec())
    );
}

#[cfg(unix)]
#[test]
fn read_prose_input_refuses_a_symlinked_ancestor() {
    let fixture = TemporaryFixture::new("symlink-ancestor");
    fixture.write_outside("nested/input.md", b"outside bytes\n");
    let link = fixture.root.join("linked");
    directory_symlink(&fixture.outside, &link).expect("create owned ancestor symlink");

    let error = read_prose_input(&fixture.root, "linked/nested/input.md")
        .expect_err("an ancestor symlink must be refused before prose bytes are returned");
    assert!(error.contains("symlink_rejected"), "{error}");
}

#[cfg(unix)]
#[test]
fn read_prose_input_refuses_a_symlinked_leaf() {
    let fixture = TemporaryFixture::new("symlink-leaf");
    fixture.write_outside("outside.md", b"outside bytes\n");
    let link = fixture.root.join("linked.md");
    file_symlink(&fixture.outside.join("outside.md"), &link).expect("create owned leaf symlink");

    let error = read_prose_input(&fixture.root, "linked.md")
        .expect_err("a leaf symlink must be refused before prose bytes are returned");
    assert!(error.contains("symlink_rejected"), "{error}");
}

#[test]
fn read_prose_input_refuses_an_oversized_file() {
    let fixture = TemporaryFixture::new("oversized");
    let bytes = vec![b'x'; 4 * 1024 * 1024 + 1];
    fixture.write_root("docs/oversized.md", &bytes);

    let error = read_prose_input(&fixture.root, "docs/oversized.md")
        .expect_err("an input over the fixed four-mebibyte cap must be refused");
    assert!(
        error.contains("Prose input exceeds 4194304 bytes"),
        "{error}"
    );
}
