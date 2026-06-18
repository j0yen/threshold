//! AC7: README documents every source, the JSON schema, and the
//! --source-root testing seam.

#[test]
fn readme_documents_all_sources() {
    let readme = std::fs::read_to_string("README.md")
        .expect("README.md must exist");

    let sources = [
        "RecallSource",
        "GossipSource",
        "BuildManifestSource",
        "GitSource",
        "DocketSource",
        "ReviewDueSource",
    ];

    for source in &sources {
        assert!(
            readme.contains(source),
            "README.md must document source '{source}'"
        );
    }
}

#[test]
fn readme_documents_json_schema() {
    let readme = std::fs::read_to_string("README.md")
        .expect("README.md must exist");

    // Must mention JSON schema
    assert!(
        readme.contains("JSON schema") || readme.contains("json schema") || readme.contains("threshold.briefing.v1"),
        "README.md must document the JSON schema"
    );

    // Must include the schema version identifier
    assert!(
        readme.contains("threshold.briefing.v1"),
        "README.md must mention the schema version 'threshold.briefing.v1'"
    );
}

#[test]
fn readme_documents_source_root_seam() {
    let readme = std::fs::read_to_string("README.md")
        .expect("README.md must exist");

    assert!(
        readme.contains("--source-root"),
        "README.md must document the --source-root testing seam"
    );
}
