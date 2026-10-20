import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import ts from "typescript";

const source = readFileSync(new URL("../src/lib/radial-shortcut.ts", import.meta.url), "utf8");
const js = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
const { radialShortcutError, radialToolbarKeyAction, normalizeRadialRightClick } = await import(`data:text/javascript;base64,${Buffer.from(js).toString("base64")}`);
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

let routingChecks = 0;
const event = (code, extra = {}) => ({ code, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, isComposing: false, ...extra });
const check = (event, shortcut, pointer, expected) => {
  assert.equal(radialToolbarKeyAction(event, shortcut, pointer), expected);
  routingChecks++;
};
for (const tool of ["pen", "arrow", "number", "rectangle", "ellipse", "eraser", "select", "text", "line", "check-mark", "cross-mark"]) {
  check(event("KeyQ"), ["KeyQ"], tool === "pointer", "toggle");
}
check(event("KeyQ"), ["KeyQ"], true, null);
check(event("KeyQ"), [], false, null);
check(event("KeyQ", { ctrlKey: true }), ["KeyQ"], false, null);
check(event("KeyQ", { metaKey: true }), ["KeyQ"], false, null);
check(event("KeyQ", { isComposing: true }), ["KeyQ"], false, null);
check(event("KeyR", { ctrlKey: true, shiftKey: true }), ["ControlRight", "ShiftLeft", "KeyR"], false, "toggle");
check(event("Escape"), ["KeyQ"], false, "cancel");
check(event("Escape"), [], false, "cancel");
for (const [code, key] of [["BracketLeft", "LeftBracket"], ["BracketRight", "RightBracket"], ["Backslash", "BackSlash"], ["Semicolon", "SemiColon"], ["Period", "Dot"], ["Backquote", "BackQuote"], ["Digit3", "Num3"], ["Space", " "], ["F6", "F6"]]) {
  check(event(code), [key], false, "toggle");
}
assert.equal(normalizeRadialRightClick(undefined), true);
assert.equal(normalizeRadialRightClick(false), false);
assert.equal(normalizeRadialRightClick(true), true);
console.log(`Radial toolbar routing: ${routingChecks} cases passed; right-click preferences: 3 cases passed.`);
