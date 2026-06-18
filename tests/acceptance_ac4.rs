//! AC4: Each real SignalSource degrades to an empty contribution (not an
//! error and not a panic) when its backing data is missing or malformed.

use std::path::Path;

use threshold::signal::SignalSource;
use threshold::sources::{
    BuildManifestSource, DocketSource, GitSource, GossipSource, RecallSource, ReviewDueSource,
};

/// Points each source at a nonexistent directory and asserts Ok(vec![]).
fn nonexistent_root() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir creation must succeed");
    // Return the dir (which exists but has none of the expected subdirs)
    dir
}

#[test]
fn recall_source_degrades_on_missing_file() {
    let root = nonexistent_root();
    let src = RecallSource::new(Some(root.path()));
    let result = src.collect().expect("collect must not return Err");
    assert!(result.is_empty(), "recall must return empty vec on missing file");
}

#[test]
fn gossip_source_degrades_on_missing_file() {
    let root = nonexistent_root();
    let src = GossipSource::new(Some(root.path()));
    let result = src.collect().expect("collect must not return Err");
    assert!(result.is_empty(), "gossip must return empty vec on missing file");
}

#[test]
fn build_manifest_source_degrades_on_missing_file() {
    let root = nonexistent_root();
    let src = BuildManifestSource::new(Some(root.path()));
    let result = src.collect().expect("collect must not return Err");
    assert!(result.is_empty(), "build-manifest must return empty vec on missing file");
}

#[test]
fn build_manifest_source_degrades_on_malformed_json() {
    let root = nonexistent_root();
    // Create the manifest path with malformed JSON
    let manifest_dir = root.path().join(".claude/skills/build/state");
    std::fs::create_dir_all(&manifest_dir).expect("create dirs");
    std::fs::write(manifest_dir.join("manifest.json"), b"{ not valid json !!!!")
        .expect("write malformed json");

    let src = BuildManifestSource::new(Some(root.path()));
    let result = src.collect().expect("collect must not return Err");
    assert!(result.is_empty(), "build-manifest must return empty vec on malformed JSON");
}

#[test]
fn git_source_degrades_on_missing_wintermute_dir() {
    let root = nonexistent_root();
    let src = GitSource::new(Some(root.path()));
    let result = src.collect().expect("collect must not return Err");
    assert!(result.is_empty(), "git must return empty vec on missing wintermute dir");
}

#[test]
fn docket_source_degrades_on_missing_file() {
    let root = nonexistent_root();
    let src = DocketSource::new(Some(root.path()));
    let result = src.collect().expect("collect must not return Err");
    assert!(result.is_empty(), "docket must return empty vec on missing file");
}

#[test]
fn review_due_source_degrades_when_flag_absent() {
    let root = nonexistent_root();
    let src = ReviewDueSource::new(Some(root.path()));
    let result = src.collect().expect("collect must not return Err");
    assert!(result.is_empty(), "review-due must return empty vec when flag file absent");
}

#[test]
fn all_sources_degrade_on_nonexistent_path() {
    // Explicit nonexistent path (not just an empty tempdir)
    let nonexistent = Path::new("/tmp/threshold-ac4-nonexistent-xyzzy-12345");

    macro_rules! assert_degrades {
        ($src:expr, $name:expr) => {
            let result = $src.collect().expect(concat!($name, " must not Err"));
            assert!(result.is_empty(), concat!($name, " must degrade to empty on nonexistent path"));
        };
    }

    assert_degrades!(RecallSource::new(Some(nonexistent)), "RecallSource");
    assert_degrades!(GossipSource::new(Some(nonexistent)), "GossipSource");
    assert_degrades!(BuildManifestSource::new(Some(nonexistent)), "BuildManifestSource");
    assert_degrades!(GitSource::new(Some(nonexistent)), "GitSource");
    assert_degrades!(DocketSource::new(Some(nonexistent)), "DocketSource");
    assert_degrades!(ReviewDueSource::new(Some(nonexistent)), "ReviewDueSource");
}
