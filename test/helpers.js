"use strict";
// Shared by the tests: a temporary folder, the hook run as the process Claude Code starts,
// and the events it is given.
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawn } = require("node:child_process");

const KIT = path.join(__dirname, "..");
const HOOK = path.join(KIT, "hooks", "guard.js");

function tempFolder(t) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "guard-kit-"));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  return dir;
}

/** The hook, as Claude Code starts it. Resolves { code, stdout, stderr, ms }. */
function runHook(stdin, env = {}, script = HOOK, cwd = undefined) {
  return new Promise((resolve, reject) => {
    const started = Date.now();
    const base = { ...process.env };
    for (const name of ["HANGAR_API_URL", "HANGAR_API_TOKEN", "HANGAR_PANE_ID", "HANGAR_PANE_NAME", "HANGAR_KITS_DATA", "CLAUDE_PROJECT_DIR", "CLAUDE_PLUGIN_ROOT"]) delete base[name];
    const child = spawn(process.execPath, [script], { cwd, env: { ...base, ...env }, stdio: ["pipe", "pipe", "pipe"], windowsHide: true });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => (stdout += chunk));
    child.stderr.on("data", (chunk) => (stderr += chunk));
    child.on("error", reject);
    child.on("close", (code) => resolve({ code, stdout, stderr, ms: Date.now() - started }));
    child.stdin.end(stdin);
  });
}

const bashEvent = (command, cwd, extra = {}) => JSON.stringify({
  session_id: "session-1",
  cwd,
  hook_event_name: "PreToolUse",
  tool_name: "Bash",
  tool_input: { command },
  ...extra,
});

const readLog = (dir, name = "refusals.jsonl") => {
  try {
    return fs.readFileSync(path.join(dir, name), "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));
  } catch {
    return [];
  }
};

module.exports = { KIT, HOOK, tempFolder, runHook, bashEvent, readLog };
