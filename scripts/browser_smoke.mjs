#!/usr/bin/env node
// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later
//
// browser_smoke.mjs — gup-core's browser smoke test (GUP-401, GUP-408).
//
// Usage: node scripts/browser_smoke.mjs <web-dir> <png-out>
//
// Serves <web-dir> on 127.0.0.1, opens its index.html in headless Chromium
// ($GUP_CHROMIUM, default `chromium`) on SwiftShader's software WebGPU
// adapter, and watches the page over the DevTools protocol. Passes only if
// the page logs `GUP PASS` within $GUP_BROWSER_TIMEOUT seconds (default 90)
// and nothing reports an error: console.error, an uncaught exception or
// promise rejection, a browser log entry at error level (a failed load,
// say), or a rendering warning (how Chrome reports WGSL compilation errors
// and uncaptured WebGPU errors). Writes the page's pixels to <png-out>.
//
// SwiftShader is forced, not just allowed: it is the only adapter on a
// GPU-less CI runner, so local runs and CI test the same thing. Headless
// Chromium picks it even on machines with a GPU unless told otherwise.
//
// No dependencies beyond Node 22+ (global WebSocket) and a Chromium build
// that bundles SwiftShader (Chromium, Chrome, Chrome for Testing).

import { spawn } from "node:child_process";
import {
  appendFileSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { dirname, extname, join, normalize } from "node:path";

const [webDir, pngOut] = process.argv.slice(2);
if (!webDir || !pngOut) {
  console.error("usage: browser_smoke.mjs <web-dir> <png-out>");
  process.exit(2);
}
const chromium = process.env.GUP_CHROMIUM || "chromium";
const timeoutMs = 1000 * Number(process.env.GUP_BROWSER_TIMEOUT || 90);

// Chromium flags for WebGPU on SwiftShader's software Vulkan, with or
// without a GPU. --no-sandbox: the page is our own build served from
// localhost, and Ubuntu runners (AppArmor) and Nix's chromium (no setuid
// helper outside NixOS) cannot start the sandbox.
const flags = [
  "--headless=new",
  "--no-sandbox",
  "--no-first-run",
  "--no-default-browser-check",
  "--enable-unsafe-webgpu",
  "--enable-features=Vulkan",
  "--use-vulkan=swiftshader",
  "--use-webgpu-adapter=swiftshader",
  "--use-angle=swiftshader",
  "--disable-vulkan-surface",
  "--remote-debugging-port=0",
];

const types = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".wasm": "application/wasm",
};
const server = createServer((req, res) => {
  const path = normalize(
    decodeURIComponent(new URL(req.url, "http://x").pathname)
  );
  try {
    const body = readFileSync(join(webDir, path === "/" ? "index.html" : path));
    res.writeHead(200, {
      "content-type": types[extname(path)] || "application/octet-stream",
    });
    res.end(body);
  } catch {
    res.writeHead(404);
    res.end();
  }
});
await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
const pageUrl = `http://127.0.0.1:${server.address().port}/index.html`;

const profile = mkdtempSync(join(tmpdir(), "gup-browser-smoke-"));
const browser = spawn(
  chromium,
  [...flags, `--user-data-dir=${profile}`, "about:blank"],
  {
    stdio: ["ignore", "ignore", "pipe"],
    // Its own process group, so the whole browser (and any wrapper's
    // children) can be stopped at once.
    detached: true,
  }
);
let stderr = "";
const errors = [];
let result = null;
let stopping = false;
let pageFailed = false;
let finish;
const done = new Promise(resolve => (finish = resolve));
const timer = setTimeout(() => finish(), timeoutMs);
// A page error stops the run a second later (time to collect what follows):
// a panic in a spawned future can leave the page waiting forever.
const pageError = message => {
  errors.push(message);
  pageFailed = true;
  setTimeout(() => finish(), 1000);
};
browser.on("error", e => {
  errors.push(`cannot start ${chromium}: ${e.message}`);
  finish();
});
browser.on("exit", (code, signal) => {
  if (!result && !stopping)
    errors.push(
      `${chromium} exited (${signal || code}) before the page reported`
    );
  finish();
});
const devtools = await new Promise(resolve => {
  browser.stderr.on("data", chunk => {
    stderr += chunk;
    const m = stderr.match(/DevTools listening on (ws:\/\/\S+)/);
    if (m) resolve(m[1]);
  });
  done.then(() => resolve(null));
});

let version = "unknown browser";
let adapter = "no adapter reported";
let png = null;
if (devtools) {
  try {
    await drive(devtools);
  } catch (e) {
    errors.push(`DevTools protocol: ${e.message}`);
  }
}
clearTimeout(timer);

stopping = true;
try {
  process.kill(-browser.pid, "SIGKILL");
} catch {
  // Already gone.
}
browser.stderr.destroy();
server.close();
rmSync(profile, { recursive: true, force: true, maxRetries: 3 });

if (png) {
  mkdirSync(dirname(pngOut), { recursive: true });
  writeFileSync(pngOut, png);
  console.log(`browser smoke: wrote ${pngOut}`);
}
console.log(`browser smoke: ${version}, ${adapter}`);
if (!result && !pageFailed) {
  // Nothing from the page: the browser's own log is the only clue.
  console.error(stderr.split("\n").slice(-20).join("\n"));
  if (!errors.length)
    errors.push(`no GUP PASS/FAIL within ${timeoutMs / 1000} s`);
} else if (!result) {
  errors.push("the page reported no result");
} else if (!result.startsWith("GUP PASS")) {
  errors.push(result);
}
for (const e of errors) console.error(`browser smoke: FAIL: ${e}`);
if (!errors.length) console.log(`browser smoke: ${result}`);
// On GitHub Actions, record the browser, adapter and outcome on the run page.
if (process.env.GITHUB_STEP_SUMMARY) {
  const outcome = errors.length
    ? errors.map(e => `- FAIL: \`${e.split("\n")[0]}\``).join("\n")
    : `- \`${result}\``;
  appendFileSync(
    process.env.GITHUB_STEP_SUMMARY,
    `### Browser smoke test\n\n- ${version}\n- ${adapter}\n${outcome}\n`
  );
}
process.exit(errors.length ? 1 : 0);

/** Open the page over the DevTools protocol and record what it reports. */
async function drive(url) {
  const ws = new WebSocket(url);
  await new Promise((resolve, reject) => {
    ws.onopen = resolve;
    ws.onerror = () => reject(new Error(`cannot connect to ${url}`));
  });
  let nextId = 0;
  const pending = new Map();
  const send = (method, params = {}, sessionId = undefined) =>
    new Promise((resolve, reject) => {
      const id = ++nextId;
      pending.set(id, { resolve, reject, method });
      ws.send(JSON.stringify({ id, method, params, sessionId }));
    });
  ws.onmessage = ({ data }) => {
    const msg = JSON.parse(data);
    if (msg.id) {
      const p = pending.get(msg.id);
      pending.delete(msg.id);
      if (msg.error) p.reject(new Error(`${p.method}: ${msg.error.message}`));
      else p.resolve(msg.result);
    } else {
      event(msg.method, msg.params);
    }
  };
  ws.onclose = () => finish();

  version = (await send("Browser.getVersion")).product;
  const { targetId } = await send("Target.createTarget", {
    url: "about:blank",
  });
  const { sessionId } = await send("Target.attachToTarget", {
    targetId,
    flatten: true,
  });
  await send("Runtime.enable", {}, sessionId);
  await send("Log.enable", {}, sessionId);
  await send("Page.navigate", { url: pageUrl }, sessionId);
  await done;
  // Errors reported just after the result still count.
  await new Promise(resolve => setTimeout(resolve, 250));
  ws.onclose = null;
  ws.close();
}

function event(method, params) {
  if (method === "Runtime.consoleAPICalled") {
    const text = params.args
      .map(a =>
        a.value !== undefined
          ? String(a.value)
          : a.description || a.unserializableValue
      )
      .join(" ");
    if (text.startsWith("GUPPNG ")) {
      png = Buffer.from(
        text.replace(/^GUPPNG data:image\/png;base64,/, ""),
        "base64"
      );
      return;
    }
    console.log(`console.${params.type}: ${text}`);
    if (text.startsWith("GUP ADAPTER ")) adapter = `adapter ${text.slice(12)}`;
    if (params.type === "error" || params.type === "assert")
      pageError(`console.${params.type}: ${text}`);
    if (/^GUP (PASS|FAIL)/.test(text) && !result) {
      result = text;
      finish();
    }
  } else if (method === "Runtime.exceptionThrown") {
    const d = params.exceptionDetails;
    const text = d.exception?.description || d.text;
    console.log(`uncaught: ${text}`);
    pageError(`uncaught: ${text}`);
  } else if (method === "Log.entryAdded") {
    const { level, source, text, url } = params.entry;
    const line = `log.${level} (${source}): ${text}${url ? ` [${url}]` : ""}`;
    console.log(line);
    // Chrome reports WGSL compilation errors and uncaptured WebGPU errors
    // as "rendering" warnings, so those fail too.
    if (level === "error" || (level === "warning" && source === "rendering")) {
      pageError(line);
    }
  }
}
