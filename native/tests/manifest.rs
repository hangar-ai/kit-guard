//! The manifest's new field: `native` names the module, and the module is the
//! file `scripts/build-native.sh` puts there. The rest of the manifest is
//! checked by test/manifest.test.js, which runs with the page's tests.

mod common;

use serde_json::Value;

use common::kit;

fn manifest() -> Value {
    let text = std::fs::read_to_string(kit().join("hangar-kit.json")).expect("hangar-kit.json");
    serde_json::from_str(&text).expect("JSON")
}

#[test]
fn the_native_field_names_a_wasm_file_inside_the_kit_beside_the_page() {
    let manifest = manifest();
    assert_eq!(manifest["schema"], 1, "stays at schema 1, for Hangar 1.x");
    let tab = &manifest["tabs"][0];
    assert_eq!(tab["entry"], "tabs/guard/index.html", "the page is kept");

    let native = tab["native"].as_str().expect("a native field");
    assert!(native.to_lowercase().ends_with(".wasm"));
    assert!(
        !native.starts_with(['/', '\\']) && !native.contains(':'),
        "relative"
    );
    assert!(
        native.split(['/', '\\']).all(|part| part != ".."),
        "inside the kit"
    );

    let module = std::fs::read(kit().join(native)).expect("the module is committed");
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
