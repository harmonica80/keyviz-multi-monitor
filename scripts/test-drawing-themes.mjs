import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import ts from "typescript";

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
const themes = JSON.parse(read("src/lib/drawing-themes.json"));
const source = read("src/lib/drawing-themes.ts").replace(
  'import themes from "./drawing-themes.json";', `const themes = ${JSON.stringify(themes)};`,
);
const js = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
const { normalizeDrawingTheme, drawingThemeVariables } = await import(`data:text/javascript;base64,${Buffer.from(js).toString("base64")}`);
const colorJs = ts.transpileModule(read("src/lib/radial-colors.ts"), { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
const { DEFAULT_RADIAL_COLORS, normalizeRadialColors } = await import(`data:text/javascript;base64,${Buffer.from(colorJs).toString("base64")}`);
assert.deepEqual(DEFAULT_RADIAL_COLORS, ["#ef2b2d", "#2d37d6"]);
for (const value of [null, undefined, {}, [], ["invalid", "#fff"], [1, false]]) {
  assert.deepEqual(normalizeRadialColors(value), DEFAULT_RADIAL_COLORS);
}
assert.deepEqual(normalizeRadialColors(["#ABCDEF", "#000000"]), ["#abcdef", "#000000"]);
assert.deepEqual(normalizeRadialColors(["bad", "#123456"]), ["#ef2b2d", "#123456"]);
assert.deepEqual(themes.map(({ id }) => id), ["pearl", "graphite", "ocean", "butter", "orange", "sand"]);
for (const theme of themes) {
  assert.equal(normalizeDrawingTheme(theme.id), theme.id);
  const vars = drawingThemeVariables(theme.id);
  assert.equal(vars["--tool-fill"], theme.fill);
  assert.equal(vars["--tool-accent"], theme.accent);
  assert.notEqual(theme.fill, theme.ink);
  assert.notEqual(theme.accent, theme.selectedInk);
  for (const key of ["fill", "ink", "accent", "selectedInk", "divider"]) {
    assert.match(theme[key], /^#[\da-f]{6}$/);
  }
}
for (const old of [undefined, null, "", "unknown", "original", "gray", "colorful", "blue", "classic", "minimal"]) {
  assert.equal(normalizeDrawingTheme(old), "pearl");
  assert.deepEqual(drawingThemeVariables(old), drawingThemeVariables("pearl"));
}
assert.equal(themes[0].fill, "#fcfbfc");
assert.equal(themes[3].fill, "#fcfbfc");
assert.equal(themes[4].fill, "#fce4ec");

// Exercise the real store initializer, reset, migration and merge with platform adapters stubbed.
let storeSource = read("src/stores/drawing_toolbar.ts").replace(/^import[\s\S]*?;\r?\n/gm, "");
storeSource = `
const DRAWING_TOOLS = [{id: "pointer"}, {id: "pen"}, {id: "text"}];
const normalizeDrawingTheme = ${normalizeDrawingTheme.toString()};
const DEFAULT_RADIAL_COLORS = ${JSON.stringify(DEFAULT_RADIAL_COLORS)};
const normalizeRadialColors = ${normalizeRadialColors.toString()};
const themes = ${JSON.stringify(themes)};
const getCurrentWindow = () => ({label: "settings"});
const createJSONStorage = () => ({});
const tauriStorage = {};
const radialShortcutError = () => null;
const normalizeRadialRightClick = (value) => value !== false;
let options;
const persist = (config, settings) => { options = settings; return config; };
const createSyncedStore = (_name, initializer, wrap) => {
  let state;
  const set = (patch) => { state = {...state, ...(typeof patch === "function" ? patch(state) : patch)}; };
  state = wrap(initializer)(set);
  return () => ({getState: () => state, options});
};
${storeSource}`;
const storeJs = ts.transpileModule(storeSource, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
const { useDrawingToolbar: store } = await import(`data:text/javascript;base64,${Buffer.from(storeJs).toString("base64")}`);
assert.equal(store.getState().radialStyle, "pearl");
assert.deepEqual(store.getState().radialColors, DEFAULT_RADIAL_COLORS);
store.getState().setRadialColor(0, "#ABCDEF");
store.getState().setRadialColor(1, "#000000");
assert.deepEqual(store.getState().radialColors, ["#abcdef", "#000000"]);
store.getState().setRadialStyle("ocean");
assert.equal(store.getState().radialStyle, "ocean");
store.getState().resetToolbar();
assert.equal(store.getState().radialStyle, "pearl");
assert.deepEqual(store.getState().radialColors, DEFAULT_RADIAL_COLORS);
const oldState = {radialStyle: "classic", items: [{id: "pen", visible: false}, {id: "pointer", visible: true}], side: "left", radialShortcut: ["KeyR"], hoverEdit: {pen: false}};
const migrated = store.options.migrate(oldState);
assert.equal(migrated.radialStyle, "pearl");
assert.deepEqual(migrated.radialColors, DEFAULT_RADIAL_COLORS);
assert.equal(migrated.items[0].id, "pen");
assert.equal(migrated.items[0].visible, false);
assert.equal(migrated.side, "left");
assert.deepEqual(migrated.radialShortcut, ["KeyR"]);
assert.deepEqual(migrated.hoverEdit, {pen: false});
assert.equal(store.options.merge({...oldState, radialStyle: "orange"}, store.getState()).radialStyle, "orange");
assert.deepEqual(store.options.merge({...oldState, radialColors: ["#123456", "#654321"]}, store.getState()).radialColors, ["#123456", "#654321"]);
console.log("Drawing themes: six palettes, exact fills, legacy fallback, store reset/migration/merge passed.");
