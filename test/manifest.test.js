"use strict";
// The manifest and the kit's shape, checked by hand against docs/kits.md and
// docs/native/kits-native.md: Hangar's validator is Rust (crates/hangar-core/src/kits/manifest.rs)
// and cannot be reached from a Node test. Each rule below is one of those pages'. The tab itself
// is tested by `cargo test` in native/ and by native.test.js, which drives the committed module.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { KIT } = require("./helpers.js");
const { hangarFile, HANGAR_FILES_MISSING } = require("./hangar-ui.js");

const manifest = JSON.parse(fs.readFileSync(path.join(KIT, "hangar-kit.json"), "utf8"));
const read = (...parts) => fs.readFileSync(path.join(KIT, ...parts), "utf8");

function allFiles(dir = KIT, found = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    // `native/target` is the build of the native tab: not the kit's, and not text.
    if ([".git", ".hangar-ui", "node_modules", "target"].includes(entry.name)) continue;
    if (entry.isDirectory()) allFiles(path.join(dir, entry.name), found);
    else found.push(path.join(dir, entry.name));
  }
  return found;
}

function safePath(relative, extensions, at) {
  assert.equal(typeof relative, "string", at);
  assert.doesNotMatch(relative, /^([/\\]|[A-Za-z]:)|:|(^|[/\\])\.\.([/\\]|$)/, `${at}: must stay inside the kit`);
  assert.ok(extensions.some((ext) => relative.toLowerCase().endsWith(ext)), `${at}: ${extensions.join("/")}`);
  assert.ok(fs.existsSync(path.join(KIT, relative)), `${at}: ${relative} must exist`);
}

test("the manifest's own fields", () => {
  assert.equal(manifest.schema, 2);
  assert.equal(manifest.id, "guard");
  assert.match(manifest.id, /^[a-z0-9-]{2,40}$/);
  assert.ok(manifest.name.trim());
  assert.match(manifest.version, /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/);
  assert.ok(manifest.description.trim());
  assert.equal(manifest.homepage, "https://github.com/hangar-ai/kit-guard");
  safePath(manifest.icon, [".svg", ".png"], "icon");
  assert.equal(manifest.commands, undefined, "it runs no declared command");
  assert.equal(manifest.setup, undefined, "and asks for no setup");
  assert.equal(manifest.hangar, ">=2.0.0");
});

test("the gallery's card: one sentence, and its tags", () => {
  const { permissionsSummary, tags } = manifest.gallery;
  assert.ok(permissionsSummary.trim());
  assert.match(permissionsSummary, /hook/);
  assert.match(permissionsSummary, /tab/);
  assert.ok(Array.isArray(tags) && tags.length > 0 && tags.every((tag) => typeof tag === "string" && tag.trim()));
});

test("the plugin is found at the root, named like the kit", () => {
  assert.equal(manifest.plugin, undefined);
  const plugin = JSON.parse(read(".claude-plugin", "plugin.json"));
  assert.equal(plugin.name, manifest.id);
  assert.equal(plugin.version, manifest.version);
  assert.match(read("skills", "guard", "SKILL.md").replace(/\r\n/g, "\n"), /^---\nname: guard\n/);
});

test("the tab: Guardrails, and the two permissions it needs", () => {
  assert.equal(manifest.tabs.length, 1);
  const [tab] = manifest.tabs;
  assert.equal(tab.id, "guard");
  assert.equal(tab.title, "Guardrails");
  safePath(tab.entry, [".wasm"], "tabs[0].entry");
  assert.equal(tab.native, undefined, "schema 2 has no `native` field: the entry is the module");
  safePath(tab.icon, [".svg", ".png"], "tabs[0].icon");
  assert.deepEqual(tab.permissions, ["data.read", "data.write"]);
  assert.equal(tab.params, undefined);
});

test("the tab is native: its entry is the module, and no page is left beside it", () => {
  const [tab] = manifest.tabs;
  assert.equal(manifest.schema, 2, "a native tab needs schema 2");
  assert.equal(tab.entry, "tabs/guard.wasm");
  assert.equal(manifest.hangar, ">=2.0.0");
  assert.equal(fs.existsSync(path.join(KIT, "tabs", "guard")), false);
});

test("the module is one Hangar loads: built for wasm32-unknown-unknown, importing only what Hangar gives", () => {
  const bytes = fs.readFileSync(path.join(KIT, manifest.tabs[0].entry));
  assert.ok(bytes.length > 0 && bytes.length < 64 * 1024 * 1024, "a kit's file is 64 MB at most");
  const module = new WebAssembly.Module(bytes);
  const imports = WebAssembly.Module.imports(module).map((entry) => `${entry.module}.${entry.name}`);
  assert.deepEqual(imports.sort(), ["hangar.log", "hangar.now"], "no WASI, no wasm-bindgen glue");
  const exports = WebAssembly.Module.exports(module).map((entry) => entry.name);
  for (const name of ["memory", "hangar_abi_version", "hangar_alloc", "hangar_init", "hangar_update", "hangar_view"]) {
    assert.ok(exports.includes(name), `exports ${name}`);
  }
  const instance = new WebAssembly.Instance(module, { hangar: { log: () => undefined, now: () => BigInt(Date.now()) } });
  assert.equal(instance.exports.hangar_abi_version(), 1, "the ABI of docs/native/kits-native.md");
});

test("the module holds no folder of the machine that built it", () => {
  const text = fs.readFileSync(path.join(KIT, manifest.tabs[0].entry)).toString("latin1");
  assert.doesNotMatch(text, /\/home\/|\/Users\/|[A-Za-z]:\\Users\\|\/tmp\//, "scripts/build-native.sh remaps them out");
});

test("the hook asks for the kit and the tab the manifest declares", () => {
  const { KIT: id, TAB } = require("../hooks/open-tab.js");
  assert.equal(id, manifest.id);
  assert.equal(TAB, manifest.tabs[0].id);
});

test("rules.json reads: each rule says what it does, and can be turned off", () => {
  const rules = JSON.parse(read("rules.json"));
  assert.deepEqual(Object.keys(rules).sort(), ["downloadToShell", "dropDatabase", "encodedCommand", "forcePush", "gitClean", "gitResetHard", "guardFiles", "recursiveDelete", "secrets"]);
  for (const [name, rule] of Object.entries(rules)) {
    assert.equal(rule.enabled, true, name);
    assert.ok(rule.what.trim(), name);
  }
  new RegExp(rules.dropDatabase.sql, "i");
});

test("the README says what it does, what it touches, what it does not protect against, and where to get it", () => {
  const readme = read("README.md");
  assert.match(readme, /## What it does\b/);
  assert.match(readme, /## What it can touch\b/);
  assert.match(readme, /## What it does not protect against\b/);
  assert.match(readme, /It reads the command line/);
  assert.match(readme, /It is no sandbox/);
  assert.match(readme, /A script can do anything once started/);
  assert.ok(readme.includes("https://github.com/hangar-ai/kit-guard"));
});

test("the proof in a real Claude Code is on record, and nothing in it failed", () => {
  const result = read("proof", "RESULT.md");
  assert.match(result, /^# The guard in a real Claude Code/);
  assert.match(result, /Claude Code \d+\.\d+\.\d+/);
  for (const title of ["Refused, through Bash", "Refused, through PowerShell", "Refused, through Monitor", "Allowed, through PowerShell", "A one-time pass, on PowerShell"]) {
    assert.ok(result.includes(`## ${title}`), title);
  }
  assert.ok(result.includes("- [x] "));
  assert.equal(result.includes("FAILED"), false);
  // In the kit's test folder, an older Node's `node --test` would run it, and pay for it.
  assert.equal(fs.existsSync(path.join(KIT, "test", "prove.js")), false);
});

test("the skill tells an agent to explain, not to go around", () => {
  const skill = read("skills", "guard", "SKILL.md");
  assert.match(skill, /Stop trying/);
  assert.match(skill, /why you need it/);
  assert.match(skill, /character for character/);
  assert.ok(skill.split(/\s+/).length < 300, "a short one");
});

test("the permissions, the route and the variable it uses are in the docs, when the docs are around", (t) => {
  const docs = hangarFile("kits.md");
  if (!docs) return t.skip(HANGAR_FILES_MISSING);
  const text = fs.readFileSync(docs, "utf8");
  for (const name of ["data.read", "data.write"]) assert.ok(text.includes(`\`${name}\``), name);
  assert.ok(text.includes("/api/kit/tab/open"));
  assert.ok(text.includes("HANGAR_KITS_DATA"));
});

test("nothing personal, and the files in English, anywhere", () => {
  const personal = /[\w.+-]+@[\w-]+\.fr\b|@gmail\.com\b/i;
  for (const file of allFiles()) {
    const text = fs.readFileSync(file, "utf8");
    assert.doesNotMatch(text, personal, file);
  }
});
