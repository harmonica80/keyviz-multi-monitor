import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import ts from "typescript";

const source = readFileSync(new URL("../src/lib/toolbar-layout.ts", import.meta.url), "utf8");
const js = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
const { visibleUnitCount, totalLength, toolbarLimit, TOOLBAR_MAX_SHARE } = await import(`data:text/javascript;base64,${Buffer.from(js).toString("base64")}`);

assert.equal(TOOLBAR_MAX_SHARE, 0.75);
assert.equal(toolbarLimit(1080), 810);
assert.equal(toolbarLimit(-5), 0);

const opts = { padding: 23, gap: 1, limit: 0, moreSize: 38 };
const sizes = [24, 38, 38, 38, 38, 1, 50, 1, 26, 38, 1, 28, 1, 28];
const dividers = sizes.map((size) => size === 1);

// No limit, or everything fits: all units stay.
assert.equal(visibleUnitCount(sizes, dividers, opts), sizes.length);
const total = totalLength(sizes, opts.padding, opts.gap);
assert.equal(visibleUnitCount(sizes, dividers, { ...opts, limit: total }), sizes.length);

// One pixel too short: the tail collapses into "…" and the "…" button still fits.
const count = visibleUnitCount(sizes, dividers, { ...opts, limit: total - 1 });
assert.ok(count < sizes.length && count >= 1);
assert.ok(totalLength(sizes.slice(0, count), opts.padding, opts.gap) + opts.gap + opts.moreSize <= total - 1);

// The visible part never ends on a divider.
for (let limit = 80; limit < total; limit += 7) {
  const n = visibleUnitCount(sizes, dividers, { ...opts, limit });
  assert.ok(n >= 1 && n <= sizes.length);
  if (n < sizes.length && n > 1) assert.equal(dividers[n - 1], false, `limit ${limit}`);
}

// A tiny limit still keeps the first unit (the drag handle).
assert.equal(visibleUnitCount(sizes, dividers, { ...opts, limit: 10 }), 1);
console.log("Toolbar layout: fit, collapse and divider rules passed.");
