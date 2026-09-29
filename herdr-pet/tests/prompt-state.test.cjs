const { test } = require("node:test");
const assert = require("node:assert/strict");
const { PromptState } = require("../renderer/prompt-state.js");

test("drafts stay isolated across panes and sessions, including delimiter names", () => {
  const state = new PromptState();
  const targets = [
    { session: "work", paneId: "a" },
    { session: "work", paneId: "b" },
    { session: "other", paneId: "a" },
    { session: "work:a", paneId: "b" },
    { session: "work", paneId: "a:b" },
  ];
  targets.forEach((target, i) => state.save(target, `中文草稿 ${i}\n第二行`));
  targets.forEach((target, i) => assert.equal(state.read(target), `中文草稿 ${i}\n第二行`));
});

test("failure preserves a draft for explicit retry and blocks duplicate submissions", () => {
  const state = new PromptState();
  const target = { session: "work", paneId: "a" };
  state.save(target, "修复问题");
  assert.equal(state.begin(target), true);
  assert.equal(state.begin(target), false);
  assert.equal(state.sending, true);
  state.finish(false);
  assert.equal(state.sending, false);
  assert.equal(state.read(target), "修复问题");
  assert.equal(state.begin(target), true);
  state.finish(true);
  assert.equal(state.read(target), "");
});

test("success clears only the submitted target; whitespace cannot submit", () => {
  const state = new PromptState();
  const first = { session: "work", paneId: "a" };
  const second = { session: "other", paneId: "a" };
  state.save(first, " \n ");
  assert.equal(state.begin(first), false);
  state.save(first, "第一条");
  state.save(second, "第二条");
  assert.equal(state.begin(first), true);
  assert.equal(state.begin(second), false);
  state.finish(true);
  assert.equal(state.read(first), "");
  assert.equal(state.read(second), "第二条");
});
