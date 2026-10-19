import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import ts from "typescript";

const source = readFileSync(new URL("../src/lib/radial-shortcut.ts", import.meta.url), "utf8");
const js = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
const { radialShortcutError } = await import(`data:text/javascript;base64,${Buffer.from(js).toString("base64")}`);
for (const shortcut of [[], ["KeyQ"], ["ControlLeft", "ShiftLeft", "KeyR"], ["F6"], ["Num3"], ["Slash"]]) {
  assert.equal(radialShortcutError(shortcut), null);
}
for (const shortcut of [["Escape"], ["MetaLeft", "KeyQ"], ["Alt", "Tab"], ["Alt", "F4"], ["KeyQ", "KeyW"], ["ControlLeft"]]) {
  assert.notEqual(radialShortcutError(shortcut), null);
}
assert.notEqual(radialShortcutError(["F8"], [["F8"]]), null);
assert.notEqual(radialShortcutError(["ControlRight", "KeyZ"], [["ControlLeft", "KeyZ"]]), null);
assert.notEqual(radialShortcutError(["ControlLeft", "ShiftLeft", "KeyZ"], [["ControlLeft", "KeyZ"]]), null);
assert.equal(radialShortcutError(["KeyQ"], [["F8"], ["F7"], ["ControlLeft", "KeyZ"]]), null);
console.log("Radial shortcut validation: 16 cases passed.");
