//! The manifest's tab: its `entry` is the module, and the module is the file
//! `scripts/build-native.sh` puts there. The rest of the manifest is checked by
//! test/manifest.test.js.

mod common;

use serde_json::Value;

use common::kit;

fn manifest() -> Value {
    let text = std::fs::read_to_string(kit().join("hangar-kit.json")).expect("hangar-kit.json");
    serde_json::from_str(&text).expect("JSON")
}

#[test]
fn the_entry_names_a_wasm_file_inside_the_kit() {
    let manifest = manifest();
    assert_eq!(manifest["schema"], 2, "a native tab needs schema 2");
    assert_eq!(manifest["hangar"], ">=2.0.0");
    let tab = &manifest["tabs"][0];
    assert!(
        tab.get("native").is_none(),
        "schema 2 has no `native` field"
    );

    let entry = tab["entry"].as_str().expect("an entry");
    assert_eq!(entry, "tabs/guard.wasm");
    assert!(
        !entry.starts_with(['/', '\\']) && !entry.contains(':'),
        "relative"
    );
    assert!(
        entry.split(['/', '\\']).all(|part| part != ".."),
        "inside the kit"
    );

    let module = std::fs::read(kit().join(entry)).expect("the module is committed");
    assert_eq!(&module[..4], b"\0asm", "WebAssembly");
    assert!(module.len() < 64 * 1024 * 1024);
}

#[test]
fn the_tab_holds_the_two_permissions_it_had_and_asks_for_no_more() {
    let manifest = manifest();
    assert_eq!(
        manifest["tabs"][0]["permissions"],
        serde_json::json!(["data.read", "data.write"])
    );
    assert_eq!(manifest["tabs"][0]["params"], Value::Null);
}
