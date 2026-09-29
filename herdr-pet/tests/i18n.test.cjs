const { test } = require("node:test");
const assert = require("node:assert/strict");
const { PET_TRANSLATIONS, petLanguage, petError } = require("../renderer/i18n.js");

test("both languages have matching, nonempty labels", () => {
  assert.deepEqual(Object.keys(PET_TRANSLATIONS.en).sort(), Object.keys(PET_TRANSLATIONS.zh).sort());
  for (const labels of Object.values(PET_TRANSLATIONS)) {
    for (const value of Object.values(labels)) assert.ok(value.length > 0);
  }
});

test("saved selection overrides system language and English is the default", () => {
  assert.equal(petLanguage("en", "zh-CN"), "en");
  assert.equal(petLanguage("zh", "en-US"), "zh");
  assert.equal(petLanguage(null, "zh-TW"), "zh");
  assert.equal(petLanguage("invalid", "fr-FR"), "en");
});

test("activation errors preserve partial success and translate for English", () => {
  const message = "已切换 agent，但无法显示 Herdr 窗口：macOS 未允许 Herdr 控制 Ghostty";
  assert.equal(petError(message, "en"), "Agent selected, but its window could not be shown: macOS has not allowed Herdr to control Ghostty.");
  assert.equal(petError(message, "zh"), message);
  assert.equal(petError("Connection refused", "en"), "Connection refused");
});
