//! The crawler must not open an ignored tree at all — no entries, no descent.
//!
//! The fixture lives in `/Users/Shared` because the default rules already cover
//! the usual scratch locations (`/tmp`, `/private/var/folders`), which would
//! make this test pass for the wrong reason.

use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(format!("/Users/Shared/fsearch-ignore-test-{}", std::process::id()))
}

/// Every name the crawler saw, at any depth.
fn names(listings: &[fsearch::walk::Listing]) -> Vec<String> {
    let mut out = Vec::new();
    for l in listings {
        for e in &l.ents {
            let off = e.name_off as usize;
            let name = &l.names[off..off + e.name_len as usize];
            out.push(String::from_utf8_lossy(name).into_owned());
        }
    }
    out
}

#[test]
fn crawl_leaves_ignored_subtrees_alone() {
    let root = fixture();
    let _ = std::fs::remove_dir_all(&root);
    let home = root.join("home");
    std::fs::create_dir_all(home.join("Library/Caches/com.foo")).unwrap();
    std::fs::create_dir_all(home.join("Projects/app")).unwrap();
    std::fs::write(home.join("Projects/app/main.rs"), b"fn main() {}").unwrap();
    std::fs::write(home.join("Projects/app/.DS_Store"), b"junk").unwrap();
    std::fs::write(home.join("Library/Caches/com.foo/blob"), b"junk").unwrap();

    fsearch::ignore::init(&home, &root); // no `ignore` file: defaults only
    let seen = names(&fsearch::walk::scan(home.as_os_str().as_bytes(), 2));

    assert!(seen.iter().any(|n| n == "main.rs"), "kept files must be indexed: {seen:?}");
    assert!(seen.iter().any(|n| n == "app"), "kept folders must be listed: {seen:?}");
    assert!(!seen.iter().any(|n| n == "com.foo"), "ignored folder listed: {seen:?}");
    assert!(!seen.iter().any(|n| n == "blob"), "descended into an ignored folder: {seen:?}");
    assert!(!seen.iter().any(|n| n == ".DS_Store"), "ignored name indexed: {seen:?}");

    let _ = std::fs::remove_dir_all(&root);
}
