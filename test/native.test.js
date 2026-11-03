"use strict";
// The committed module, driven through its ABI as Hangar drives it (docs/native/kits-native.md):
// instantiated with the two imports Hangar gives, started with a `Start`, answered call by call
// from a data folder in memory. The Rust tests (native/tests) run the same tab on the host through
// the SDK's harness; this one runs the .wasm that is committed, so that what is built, and what is
// in the repository, is known to start, read the hook's log, show it, and write a pass the hook takes.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { KIT, tempFolder, bashEvent } = require("./helpers.js");
const { decide } = require("../hooks/decide.js");
const store = require("../hooks/store.js");

const MODULE = path.join(KIT, "tabs", "guard.wasm");
const NOW = Date.parse("2026-10-31T18:00:00Z");

/** One instance of the module, as Hangar keeps one per open tab. */
function open({ files = {}, locale = "en", at = NOW, permissions = ["data.read", "data.write"], deny = [] } = {}) {
  let now = at;
  const logs = [];
  let memory;
  const text = (ptr, len) => new TextDecoder().decode(new Uint8Array(memory.buffer, ptr, len));
  const { exports } = new WebAssembly.Instance(new WebAssembly.Module(fs.readFileSync(MODULE)), {
    hangar: { log: (level, ptr, len) => logs.push([level, text(ptr, len)]), now: () => BigInt(now) },
  });
  memory = exports.memory;

  const send = (call, message) => {
    const bytes = new TextEncoder().encode(JSON.stringify(message));
    const ptr = exports.hangar_alloc(bytes.length);
    new Uint8Array(memory.buffer, ptr, bytes.length).set(bytes);
    return answer(call(ptr, bytes.length));
  };
  const answer = (packed) => {
    const ptr = Number(packed >> 32n);
    const len = Number(packed & 0xffffffffn);
    return JSON.parse(text(ptr, len));
  };

  const tab = {
    files,
    calls: [], // every call the module made, in order
    waiting: [], // those not answered yet
    view: null,
    logs,
    subscriptions: null,
    clock: (ms) => (now = ms),
    absorb(output) {
      for (const call of output.calls || []) {
        tab.calls.push(call);
        tab.waiting.push(call);
      }
      if (output.subscriptions) tab.subscriptions = output.subscriptions;
      tab.view = answer(exports.hangar_view());
    },
    update(input) {
      tab.absorb(send(exports.hangar_update, input));
    },
    /** Answers what Hangar answers for the two permissions the tab holds, until nothing waits. */
    settle() {
      while (tab.waiting.length) {
        const call = tab.waiting.shift();
        const { method, params } = call;
        let reply;
        if (method === "tab.setTitle" || method === "tab.setBadge") reply = { result: {} };
        else if (deny.includes(method)) reply = { error: { code: "denied", message: `this tab was not granted ${method}` } };
        else if (method === "data.read") {
          reply = tab.files[params.path] === undefined ? { error: { code: "not_found", message: "no such file" } } : { result: { text: tab.files[params.path] } };
        } else if (method === "data.write") {
          tab.files[params.path] = params.text;
          reply = { result: {} };
        } else reply = { error: { code: "denied", message: `this tab was not granted what ${method} needs` } };
        tab.update({ type: "reply", id: call.id, ...reply });
      }
    },
    timer(id) {
      tab.update({ type: "timer", id });
      tab.settle();
    },
    /** The widgets of the page, depth first. */
    widgets() {
      const found = [];
      const walk = (node) => {
        if (node && typeof node === "object") {
          if (Array.isArray(node)) return node.forEach(walk);
          if (typeof node.type === "string") found.push(node);
          Object.values(node).forEach(walk);
        }
      };
      walk(tab.view.root);
      return found;
    },
    /** Every text the page says: a widget's own, and a row's title and second line. */
    texts() {
      const out = [];
      const walk = (node) => {
        if (node && typeof node === "object") {
          if (Array.isArray(node)) return node.forEach(walk);
          for (const field of ["text", "label", "title", "sub"]) if (typeof node[field] === "string") out.push(node[field]);
          Object.values(node).forEach(walk);
        }
      };
      walk(tab.view.root);
      return out;
    },
    shows: (needle) => tab.texts().some((text) => text.includes(needle)),
    button: (label) => tab.widgets().find((node) => node.type === "button" && node.label === label),
    press(label) {
      const button = tab.button(label);
      assert.ok(button && button.onPress !== undefined, `a button ${label}`);
      tab.update({ type: "ui", view: tab.view.seq, handler: button.onPress });
      tab.settle();
    },
  };

  const start = {
    abi: 1, hangar: "2.0.0", kit: { id: "guard", name: "Guardrails", version: "0.1.1" },
    tab: { id: "guard", params: {}, caller: null }, theme: { scheme: "dark", locale }, permissions, visible: true, utcOffset: 0,
  };
  assert.equal(exports.hangar_abi_version(), 1);
  tab.absorb(send(exports.hangar_init, start));
  tab.settle();
  return tab;
}

const key = "a".repeat(64);
const refusal = (extra = {}) => ({
  type: "refusal", at: NOW - 180000, key, agent: "Ava", agentId: "pane-1", tool: "Bash", command: "git reset --hard", length: 16,
  rule: "git-reset-hard", kind: "any", subject: "", why: "git reset --hard throws away uncommitted work, which git cannot bring back",
  folder: "/work/project", ...extra,
});
const lines = (...events) => events.map((event) => JSON.stringify(event)).join("\n") + "\n";

test("the module starts, names its tab, reads the hook's data and draws the empty list", () => {
  const tab = open();
  assert.deepEqual(tab.calls.map((call) => call.method).slice(0, 2), ["tab.setTitle", "data.read"]);
  assert.deepEqual(tab.calls[0].params, { title: "Guardrails" });
  assert.ok(tab.shows("Nothing refused yet"));
  assert.ok(tab.shows("It is no sandbox"));
  assert.deepEqual(tab.subscriptions.timers, [{ id: "poll", everyMs: 2000 }]);
  assert.ok(tab.subscriptions.events.includes("theme"));
  assert.deepEqual(tab.logs.filter(([level]) => level >= 3), [], "nothing logged as a warning or worse");
});

test("it reads only the hook's files, and its badge and title are the strip's", () => {
  const tab = open({ files: { "refusals.jsonl": lines(refusal()), "errors.jsonl": lines({ at: NOW, message: "x" }) } });
  const methods = new Set(tab.calls.map((call) => call.method));
  assert.deepEqual([...methods].sort(), ["data.read", "data.write", "tab.setBadge", "tab.setTitle"].filter((method) => methods.has(method)));
  assert.equal(methods.has("data.write"), false, "reading writes nothing");
  assert.deepEqual(tab.calls.filter((call) => call.method === "data.read").map((call) => call.params.path).sort(), [`passes/${key}.json`, "errors.jsonl", "refusals.jsonl", "state.json"].sort());
  assert.deepEqual(tab.calls.filter((call) => call.method === "tab.setBadge").pop().params, { state: "failure", text: "1" });
});

test("a refusal is drawn, and one click writes the pass the hook takes", (t) => {
  const tab = open({ files: { "refusals.jsonl": lines(refusal()) } });
  assert.ok(tab.shows("Ava"));
  assert.ok(tab.shows("git reset --hard"));
  assert.ok(tab.shows("3 min ago"));
  assert.ok(tab.button("Allow once"));
  assert.equal(tab.calls.some((call) => call.method === "data.write"), false);

  tab.press("Allow once");
  const writes = tab.calls.filter((call) => call.method === "data.write");
  assert.equal(writes.length, 1);
  assert.equal(writes[0].params.path, `passes/${key}.json`);
  const pass = JSON.parse(writes[0].params.text);
  assert.deepEqual(Object.keys(pass), ["key", "agentId", "agent", "tool", "command", "createdAt", "expiresAt"]);
  assert.equal(pass.expiresAt - pass.createdAt, 120000);
  assert.equal(pass.createdAt, NOW);
  assert.ok(tab.shows("Allowed once, 2:00 left"));
  assert.equal(tab.button("Allow once"), undefined);
  assert.ok(tab.subscriptions.timers.some((timer) => timer.id === "clock"), "the countdown ticks");

  tab.clock(NOW + 10000);
  tab.timer("clock");
  assert.ok(tab.shows("1:50 left"));

  // The same file, in the hook's own folder: taken by the agent it was for, once.
  const dir = path.join(tempFolder(t), "guard");
  fs.mkdirSync(path.join(dir, "passes"), { recursive: true });
  fs.writeFileSync(path.join(dir, "passes", `${key}.json`), writes[0].params.text);
  assert.equal(store.takePass(dir, key, "pane-1", NOW + 1000), true);
  assert.equal(store.takePass(dir, key, "pane-1", NOW + 1000), false);
});

test("from the hook's real refusal to the tab's pass and back to the hook's retry", (t) => {
  const root = tempFolder(t);
  const dir = path.join(root, "guard");
  const project = path.join(root, "project");
  fs.mkdirSync(project);
  const env = { HANGAR_PANE_ID: "pane-1", HANGAR_PANE_NAME: "Ava", CLAUDE_PROJECT_DIR: project };
  const run = (now) => decide(bashEvent("git reset --hard", project), { env, now, context: { dataDir: dir } });
  assert.ok(run(NOW - 10000), "refused");

  const tab = open({ files: { "refusals.jsonl": fs.readFileSync(path.join(dir, "refusals.jsonl"), "utf8") } });
  tab.press("Allow once");
  const write = tab.calls.filter((call) => call.method === "data.write").pop();
  fs.mkdirSync(path.join(dir, "passes"), { recursive: true });
  fs.writeFileSync(path.join(dir, write.params.path), write.params.text);
  assert.equal(run(NOW + 5000), null, "the retry goes through");
  assert.ok(run(NOW + 6000), "once");
});

test("in French, and the language follows Hangar's", () => {
  const tab = open({ files: { "refusals.jsonl": lines(refusal()) }, locale: "fr" });
  assert.deepEqual(tab.calls[0].params, { title: "Garde-fous" });
  assert.ok(tab.shows("Appels refusés"));
  assert.ok(tab.shows("jette le travail non commité"));
  assert.ok(tab.button("Autoriser une fois"));
  const english = open({ files: { "refusals.jsonl": lines(refusal()) } });
  english.update({ type: "event", name: "theme", data: { scheme: "dark", locale: "fr" } });
  english.settle();
  assert.ok(english.shows("Appels refusés"));
  assert.deepEqual(english.calls.filter((call) => call.method === "tab.setTitle").map((call) => call.params.title), ["Guardrails", "Garde-fous"]);
});

test("without data.read the page says so, no badge it cannot know, and the hook goes on refusing", () => {
  const tab = open({ deny: ["data.read"], files: { "refusals.jsonl": lines(refusal()) } });
  assert.ok(tab.shows("This tab may not read the kit’s data"));
  assert.ok(tab.shows("It is no sandbox"), "the limits are still said");
  assert.ok(tab.button("Try again"));
  assert.deepEqual(tab.calls.filter((call) => call.method === "tab.setBadge").pop().params, { state: null });
  assert.equal(tab.calls.some((call) => call.method === "data.write"), false);
});

test("without data.write the row says why nothing was let through", () => {
  const tab = open({ deny: ["data.write"], files: { "refusals.jsonl": lines(refusal()) } });
  tab.press("Allow once");
  assert.ok(tab.shows("This tab may not write passes. Accept data.write in Settings › Kits."));
  assert.ok(tab.button("Allow once"), "and it can be tried again");
});
