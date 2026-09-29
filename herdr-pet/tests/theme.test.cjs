const { test } = require("node:test");
const assert = require("node:assert/strict");
const { themeVariables, themedSpriteColors } = require("../renderer/theme.js");

test("theme variables preserve light colors and readable accent labels", () => {
  const variables = themeVariables({ colors: { background: "#eff1f5", text: "#4c4f69", accent: "#f5c2e7", blocked: "#111111" } });
  assert.equal(variables["--pet-background"], "#eff1f5");
  assert.equal(variables["--pet-text"], "#4c4f69");
  assert.equal(variables["--pet-on-accent"], "#000000");
  assert.equal(variables["--pet-on-blocked"], "#ffffff");
});

test("unavailable or invalid colors do not introduce stale theme variables", () => {
  assert.deepEqual(themeVariables(null), {});
  assert.deepEqual(themeVariables({ colors: { accent: "url(example)", text: null, background: "#xyzxyz", unexpected: "#ffffff" } }), {});
});

test("sprite state colors follow semantic roles and retain unavailable defaults", () => {
  const base = { k: "#111111", w: "#eeeeee", b: "#222222", r: "#ff0000", y: "#ffff00", t: "#00ffff" };
  const themed = themedSpriteColors(base, { colors: { text: "#eeeeee", background: "#111111", working: "#ddccbb", blocked: "#cc1122", done: "#aabbcc" } });
  assert.equal(themed.k, "#eeeeee");
  assert.equal(themed.y, "#ddccbb");
  assert.equal(themed.r, "#cc1122");
  assert.equal(themed.t, "#aabbcc");
  assert.equal(themed.b, base.b);
  assert.deepEqual(themedSpriteColors(base, null), base);
});
