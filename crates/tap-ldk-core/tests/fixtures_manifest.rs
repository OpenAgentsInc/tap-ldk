use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;

const REPO_ROOT_FROM_CORE: &str = "../..";

#[test]
fn fixture_manifest_loads_and_has_provenance() {
    let manifest = load_manifest();
    let fixtures = manifest["fixtures"]
        .as_array()
        .expect("fixtures must be an array");

    assert!(!fixtures.is_empty(), "fixtures manifest cannot be empty");

    let mut ids = BTreeSet::new();
    for fixture in fixtures {
        let id = required_str(fixture, "id");
        assert!(ids.insert(id.to_owned()), "duplicate fixture id: {id}");

        let path = required_str(fixture, "path");
        let absolute_path = repo_root().join(path);
        assert!(absolute_path.exists(), "fixture path missing: {path}");

        let source = fixture["source"]
            .as_object()
            .expect("fixture source must be an object");
        for field in ["upstream", "local_path", "commit", "notes"] {
            let value = source
                .get(field)
                .and_then(Value::as_str)
                .unwrap_or_default();
            assert!(
                !value.trim().is_empty(),
                "fixture {id} missing source.{field}"
            );
        }
    }
}

#[test]
fn required_fixture_categories_are_reachable() {
    let manifest = load_manifest();
    let fixtures = manifest["fixtures"]
        .as_array()
        .expect("fixtures must be an array");
    let categories = fixtures
        .iter()
        .map(|fixture| required_str(fixture, "category"))
        .collect::<BTreeSet<_>>();

    for required in [
        "address",
        "channel_trace",
        "ms_smt",
        "proof",
        "virtual_psbt",
        "vm",
    ] {
        assert!(
            categories.contains(required),
            "missing required fixture category: {required}"
        );
    }
}

#[test]
fn imported_fixture_json_files_are_valid_json() {
    let manifest = load_manifest();
    let fixtures = manifest["fixtures"]
        .as_array()
        .expect("fixtures must be an array");

    for fixture in fixtures {
        let path = required_str(fixture, "path");
        let raw = fs::read_to_string(repo_root().join(path))
            .unwrap_or_else(|err| panic!("failed to read {path}: {err}"));
        serde_json::from_str::<Value>(&raw)
            .unwrap_or_else(|err| panic!("fixture is not valid JSON: {path}: {err}"));
    }
}

fn load_manifest() -> Value {
    let raw = fs::read_to_string(repo_root().join("fixtures/manifest.json"))
        .expect("fixtures manifest is readable");
    serde_json::from_str(&raw).expect("fixtures manifest is valid JSON")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(REPO_ROOT_FROM_CORE)
}

fn required_str<'a>(value: &'a Value, field: &str) -> &'a str {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing string field: {field}"))
}
