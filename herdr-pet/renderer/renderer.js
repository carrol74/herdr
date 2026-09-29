const PIXEL = 5;
const GAP = 8;
const MARGIN = 6;
const HOVER_EXIT_DELAY = 160;
const DEFAULT_LAYOUT = { petLeft: 12, petTop: 119 };

const BASE_COLORS = {
  k: "rgb(40 40 46)",
  b: "rgb(70 90 120)",
  g: "rgb(120 140 160)",
  w: "rgb(230 235 245)",
  r: "rgb(220 90 90)",
  y: "rgb(235 205 110)",
  t: "rgb(90 190 170)",
};
let COLORS = { ...BASE_COLORS };
let currentTheme = null;

function applyTheme(theme) {
  const signature = JSON.stringify(theme || null);
  if (signature === currentTheme) return;
  currentTheme = signature;
  for (const role of [...PET_THEME_ROLES, "on-accent", "on-blocked"]) {
    document.documentElement.style.removeProperty(`--pet-${role}`);
  }
  for (const [property, color] of Object.entries(themeVariables(theme))) {
    document.documentElement.style.setProperty(property, color);
  }
  COLORS = themedSpriteColors(BASE_COLORS, theme);
}

let language = petLanguage(localStorage.getItem("herdr-pet-language"), navigator.language);
const t = (key) => PET_TRANSLATIONS[language][key];
const errorLabel = (error) => petError(error, language);

const FRAMES = {
  sleeping: [
    ["....kk....", "...kkk....", "...kkk....", "..kkkkk...", ".kkwwk.kk.", ".kbwk.kbk.", ".kkkkkkk..", "..kk.kk...", "..k...k..."],
    ["....kk....", "...kkk....", "...kkk....", "..kkkkk...", ".kkwwk.kk.", ".kbwk.kbk.", ".kkkkkkk..", "...kk.kk..", "..k...k..."],
  ],
  working: [
    ["....yy....", "...yyy....", "...kkk....", "..kkkkk...", ".kkwwkkk..", ".kbwk.kbk.", ".kkkkkkk..", "..kk.kk...", "..k...k..."],
    ["....yy....", "...yyy....", "...kkk....", "..kkkkk...", ".kkwwkkk..", ".kbwk.kbk.", ".kkkkkkk..", "...kk.kk..", "..k...k..."],
  ],
  attention: [
    ["....rr....", "...rrr....", "...kkk....", "..kkkkk...", ".kkwwkkk..", ".kbwk.kbk.", ".kkkkkkk..", "..kk.kk...", "..k...k..."],
    ["....rr....", "...rrr....", "...kkk....", "..kkkkk...", ".kkwwkkk..", ".kbwk.kbk.", ".kkkkkkk..", ".kk.kk.kk.", ".k.....k.."],
  ],
  celebrate: [
    ["..tt.tt...", "....tt....", "...kkk....", "..kkkkk...", ".kkwwkkk..", ".kbwk.kbk.", ".kkkkkkk..", "..kk.kk...", "..k...k..."],
    [".ttt..ttt.", "....tt....", "...kkk....", "..kkkkk...", ".kkwwkkk..", ".kbwk.kbk.", ".kkkkkkk..", "..kk.kk...", "..k...k..."],
  ],
  neutral: [
    ["....gg....", "...ggg....", "...kkk....", "..kkkkk...", ".kkwwkkk..", ".kbwk.kbk.", ".kkkkkkk..", "..kk.kk...", "..k...k..."],
    ["....gg....", "...ggg....", "...kkk....", "..kkkkk...", ".kkwwkkk..", ".kbwk.kbk.", ".kkkkkkk..", "...kk.kk..", "..k...k..."],
  ],
  offline: [["....kk....", "...kkk....", "...kkk....", "..kkkkk...", ".kkkkkkk..", ".kkkkkkk..", ".kkkkkkk..", "..kk.kk...", "..k...k..."]],
};

const pet = document.getElementById("pet");
const context = pet.getContext("2d");
const bubble = document.getElementById("bubble");
const bubbleContent = document.getElementById("bubble-content");
const offline = document.getElementById("offline");
const attentionCount = document.getElementById("attention-count");
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let state = {
  connection: "offline",
  mood: "offline",
  subject: null,
  agents: [],
  blockedCount: 0,
  session: "default",
  sessions: [],
  offlineReason: null,
};
let layout = { ...DEFAULT_LAYOUT };
let frameIndex = 0;
let hovered = false;
let bubbleHovered = false;
let hideTimer = null;
let dragging = false;
let promptTarget = null;
let actionError = null;
const composer = new PromptState();

function updateLanguage() {
  document.documentElement.lang = language === "zh" ? "zh-CN" : "en";
  bubble.setAttribute("aria-label", t("region"));
  pet.setAttribute("aria-label", t("pet"));
  attentionCount.setAttribute("aria-label", t("attention"));
  offline.textContent = t("offline");
}

function bubbleHeader(title, withBack = false) {
  const header = document.createElement("div");
  header.className = "bubble-header";
  if (withBack) {
    const back = document.createElement("button");
    back.type = "button";
    back.className = "bubble-back";
    back.textContent = "<";
    back.title = t("back");
    back.setAttribute("aria-label", t("back"));
    back.disabled = composer.sending;
    back.addEventListener("click", closePrompt);
    header.append(back);
  }
  header.append(createLine(title, "bubble-title"));
  const menu = document.createElement("button");
  menu.type = "button";
  menu.className = "bubble-menu";
  menu.textContent = "⋯";
  menu.title = t("settings");
  menu.setAttribute("aria-label", t("settings"));
  menu.disabled = composer.sending;
  menu.addEventListener("click", () => {
    const rect = menu.getBoundingClientRect();
    invoke("show_settings_menu", {
      english: language === "en",
      menuX: rect.left,
      menuY: rect.bottom,
    }).catch((error) => {
      actionError = String(error);
      showBubble();
    });
  });
  header.append(menu);
  return header;
}

function draw() {
  const frames = FRAMES[state.mood] || FRAMES.neutral;
  const rows = frames[frameIndex % frames.length];
  context.clearRect(0, 0, pet.width, pet.height);
  rows.forEach((row, y) => {
    [...row].forEach((cell, x) => {
      if (!COLORS[cell]) return;
      context.fillStyle = COLORS[cell];
      context.fillRect(x * PIXEL, y * PIXEL, PIXEL, PIXEL);
    });
  });
  offline.classList.toggle("hidden", state.connection !== "offline");
  attentionCount.classList.toggle("hidden", !state.blockedCount);
  attentionCount.textContent = state.blockedCount > 9 ? "9+" : String(state.blockedCount || "");
}

function applyLayout(next) {
  layout = {
    petLeft: Number.isFinite(next?.petLeft) ? next.petLeft : DEFAULT_LAYOUT.petLeft,
    petTop: Number.isFinite(next?.petTop) ? next.petTop : DEFAULT_LAYOUT.petTop,
  };
  pet.style.left = `${layout.petLeft}px`;
  pet.style.top = `${layout.petTop}px`;
  attentionCount.style.left = `${layout.petLeft + pet.width - 11}px`;
  attentionCount.style.top = `${Math.max(0, layout.petTop - 4)}px`;
  offline.style.left = `${layout.petLeft}px`;
  const below = layout.petTop + pet.height + 3;
  offline.style.top = `${below + 12 <= innerHeight ? below : Math.max(0, layout.petTop - 13)}px`;
  if (bubble.classList.contains("visible")) positionBubble();
}

function elapsedLabel(sinceMs) {
  const seconds = Math.max(0, Math.floor((Date.now() - sinceMs) / 1000));
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m`;
}

function createLine(text, className = "bubble-line") {
  const element = document.createElement("div");
  element.className = className;
  element.textContent = text;
  return element;
}

function renderOverview() {
  const scrollTop = bubbleContent.querySelector(".agent-list")?.scrollTop || 0;
  const focused = document.activeElement?.dataset;
  const focusedPane = focused?.pane;
  const focusedAction = focused?.action;
  if (!state.agents?.length) {
    bubbleContent.replaceChildren(
      bubbleHeader(state.offlineReason ? errorLabel(state.offlineReason) : t("empty")),
      createLine(`session · ${state.session}`),
    );
    bubbleContent.scrollTop = scrollTop;
    return;
  }

  const title = bubbleHeader(
    `${state.session} · ${state.agents.length} agent${state.agents.length === 1 ? "" : "s"}`,
  );
  const rows = state.agents.map((agent) => {
    const row = document.createElement("div");
    row.className = "agent-row";
    if (agent.paneId === state.subject?.paneId) row.classList.add("selected");

    const dot = document.createElement("span");
    dot.className = `status-dot status-${agent.status}`;
    const details = document.createElement("span");
    details.className = "agent-details";
    const heading = createLine(
      `${agent.agent || "agent"} · ${t(agent.status) || agent.status}`,
      "agent-heading",
    );
    const subtitle = document.createElement("div");
    subtitle.className = "agent-subtitle";
    const description = document.createElement("span");
    description.textContent = agent.title || agent.cwd || agent.paneId;
    const elapsed = document.createElement("span");
    elapsed.className = "agent-elapsed";
    elapsed.dataset.since = String(agent.statusSinceMs);
    elapsed.textContent = elapsedLabel(agent.statusSinceMs);
    subtitle.append(description, " · ", elapsed);
    details.append(heading, subtitle);
    const message = document.createElement("button");
    message.type = "button";
    message.dataset.pane = agent.paneId;
    message.dataset.action = "message";
    message.className = "agent-message";
    message.setAttribute("aria-label", `${t("message")} · ${agent.agent || "agent"}`);
    message.title = t("message");
    message.addEventListener("click", (event) => {
      event.stopPropagation();
      promptTarget = { ...agent, session: state.session };
      showBubble();
    });
    const focus = document.createElement("button");
    focus.type = "button";
    focus.className = "agent-focus";
    focus.dataset.pane = agent.paneId;
    focus.dataset.action = "focus";
    focus.title = t("focus");
    focus.append(dot, details);
    row.append(focus, message);
    const focusAgent = () => {
      invoke("focus_agent", { paneId: agent.paneId }).catch((error) => {
        actionError = String(error);
        showBubble();
      });
    };
    focus.addEventListener("click", focusAgent);
    return row;
  });
  const content = [title];
  if (actionError) content.push(createLine(errorLabel(actionError), "action-error"));
  const list = document.createElement("div");
  list.className = "agent-list";
  list.append(...rows);
  content.push(list);
  bubbleContent.replaceChildren(...content);
  list.scrollTop = scrollTop;
  if (focusedPane) {
    [...list.querySelectorAll("button")].find((button) =>
      button.dataset.pane === focusedPane && button.dataset.action === focusedAction
    )?.focus({ preventScroll: true });
  }
}

function renderPrompt() {
  if (document.getElementById("prompt-form")) return;
  const target = promptTarget;
  const form = document.createElement("form");
  form.id = "prompt-form";
  const title = bubbleHeader(`${t("to")} ${promptTarget.agent || "agent"}`, true);
  const input = document.createElement("textarea");
  input.id = "prompt-input";
  input.rows = 4;
  input.maxLength = 4000;
  input.placeholder = t("placeholder");
  input.setAttribute("aria-label", t("input"));
  input.value = composer.read(target);
  const destination = createLine(`${target.session} · ${target.title || target.cwd || target.paneId}`, "prompt-destination");
  destination.title = destination.textContent;
  const feedback = createLine("", "prompt-feedback");
  feedback.setAttribute("role", "status");
  const count = createLine("", "prompt-count");
  const hint = createLine(t("hint"), "prompt-hint");
  const editor = document.createElement("div");
  editor.className = "prompt-editor";
  const submit = document.createElement("button");
  submit.type = "submit";
  submit.className = "prompt-send";
  submit.textContent = t("send");
  editor.append(input, submit);
  form.append(title, destination, editor, count, hint, feedback);
  bubbleContent.replaceChildren(form);

  const update = () => {
    composer.save(target, input.value);
    count.textContent = `${input.value.length} / ${input.maxLength}`;
    submit.disabled = composer.sending || !input.value.trim();
  };
  input.addEventListener("input", update);
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey) && !event.isComposing) {
      event.preventDefault();
      if (!submit.disabled) form.requestSubmit();
    }
  });
  update();
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    const text = input.value.trim();
    if (!composer.begin(target)) return;
    title.querySelector(".bubble-back").disabled = true;
    title.querySelector(".bubble-menu").disabled = true;
    input.readOnly = true;
    submit.disabled = true;
    submit.textContent = t("sending");
    feedback.textContent = t("submitting");
    feedback.classList.remove("action-error");
    invoke("send_prompt", { paneId: target.paneId, text, session: target.session })
      .then(() => {
        composer.finish(true);
        input.value = "";
        feedback.textContent = t("submitted");
      })
      .catch((error) => {
        composer.finish(false);
        feedback.textContent = errorLabel(error);
        feedback.classList.add("action-error");
      })
      .finally(() => {
        title.querySelector(".bubble-back").disabled = false;
        title.querySelector(".bubble-menu").disabled = false;
        input.readOnly = false;
        submit.textContent = t("send");
        update();
        positionBubble();
      });
  });
  invoke("set_prompt_active", { active: true })
    .then(() => requestAnimationFrame(() => input.focus()))
    .catch((error) => {
      feedback.textContent = errorLabel(error);
      feedback.classList.add("action-error");
    });
}

function updateElapsedLabels() {
  for (const element of bubbleContent.querySelectorAll(".agent-elapsed")) {
    element.textContent = elapsedLabel(Number(element.dataset.since));
  }
}

function positionBubble() {
  const petRect = pet.getBoundingClientRect();
  const previousVisibility = bubble.style.visibility;
  bubble.style.visibility = "hidden";
  bubbleContent.style.maxHeight = "none";
  const naturalHeight = bubbleContent.getBoundingClientRect().height;
  const spaceAbove = petRect.top - GAP - MARGIN;
  const spaceBelow = innerHeight - petRect.bottom - GAP - MARGIN;
  const above = spaceAbove >= naturalHeight || (spaceBelow < naturalHeight && spaceAbove >= spaceBelow);
  const availableHeight = Math.max(1, above ? spaceAbove : spaceBelow);
  bubbleContent.style.maxHeight = `${Math.floor(availableHeight)}px`;
  const bubbleRect = bubble.getBoundingClientRect();
  const left = clamp(
    petRect.left + petRect.width / 2 - bubbleRect.width / 2,
    MARGIN,
    Math.max(MARGIN, innerWidth - bubbleRect.width - MARGIN),
  );
  const top = above ? petRect.top - GAP - bubbleRect.height : petRect.bottom + GAP;
  bubble.dataset.side = above ? "above" : "below";
  bubble.style.left = `${Math.round(left)}px`;
  bubble.style.top = `${Math.round(clamp(top, MARGIN, Math.max(MARGIN, innerHeight - bubbleRect.height - MARGIN)))}px`;
  bubble.style.setProperty(
    "--tail-x",
    `${Math.round(clamp(petRect.left + petRect.width / 2 - left, 14, bubbleRect.width - 14))}px`,
  );
  bubble.style.visibility = previousVisibility;
}

function clamp(value, minimum, maximum) {
  return Math.max(minimum, Math.min(value, maximum));
}

function showBubble() {
  if (dragging) return;
  cancelBubbleHide();
  if (promptTarget) {
    renderPrompt();
  } else {
    renderOverview();
  }
  bubble.classList.add("visible");
  positionBubble();
}

function cancelBubbleHide() {
  if (hideTimer !== null) {
    clearTimeout(hideTimer);
    hideTimer = null;
  }
}

function hideBubble(force = false) {
  cancelBubbleHide();
  if (force || (!promptTarget && !hovered && !bubbleHovered)) {
    bubble.classList.remove("visible");
  }
}

function scheduleBubbleHide() {
  cancelBubbleHide();
  if (promptTarget) return;
  hideTimer = setTimeout(() => {
    hideTimer = null;
    hideBubble();
  }, HOVER_EXIT_DELAY);
}

function closePrompt() {
  if (composer.sending) return;
  const wasActive = promptTarget !== null;
  promptTarget = null;
  if (wasActive) bubbleContent.replaceChildren();
  if (wasActive) invoke("set_prompt_active", { active: false });
  if (hovered || bubbleHovered) showBubble();
  else hideBubble(true);
}

function finishDrag() {
  dragging = false;
  pet.classList.remove("dragging");
}

pet.addEventListener("pointerdown", (event) => {
  if (event.button !== 0) return;
  if (composer.sending) return;
  event.preventDefault();
  dragging = true;
  bubbleHovered = false;
  closePrompt();
  hideBubble(true);
  pet.classList.add("dragging");
  invoke("start_drag").catch(finishDrag);
});

pet.addEventListener("pointerup", finishDrag);

pet.addEventListener("dblclick", () => {
  if (composer.sending) return;
  if (state.subject) invoke("focus_agent", { paneId: state.subject.paneId });
});

pet.addEventListener("contextmenu", (event) => event.preventDefault());

pet.addEventListener("mouseenter", () => {
  hovered = true;
  if (!dragging) showBubble();
});

pet.addEventListener("mouseleave", () => {
  hovered = false;
  scheduleBubbleHide();
});

bubble.addEventListener("mouseenter", () => {
  bubbleHovered = true;
  cancelBubbleHide();
});

bubble.addEventListener("mouseleave", () => {
  bubbleHovered = false;
  scheduleBubbleHide();
});

listen("pet-language", (event) => {
  if (composer.sending || !["en", "zh"].includes(event.payload)) return;
  language = event.payload;
  localStorage.setItem("herdr-pet-language", language);
  updateLanguage();
  bubbleContent.replaceChildren();
  if (bubble.classList.contains("visible")) showBubble();
});

window.addEventListener("resize", () => bubble.classList.contains("visible") && positionBubble());
window.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && !event.isComposing) closePrompt();
});

listen("pet-state", (event) => {
  state = event.payload;
  applyTheme(state.theme);
  draw();
  if ((hovered || bubbleHovered) && !promptTarget && !dragging) showBubble();
});

listen("pet-layout", (event) => {
  applyLayout(event.payload);
  finishDrag();
});
listen("pet-drag-ended", finishDrag);
listen("pet-error", (event) => {
  actionError = String(event.payload);
  if (promptTarget) {
    const feedback = document.querySelector(".prompt-feedback");
    feedback.textContent = errorLabel(actionError);
    feedback.classList.add("action-error");
    positionBubble();
    return;
  }
  showBubble();
  setTimeout(() => {
    actionError = null;
    if (hovered || bubbleHovered) showBubble();
    else hideBubble(true);
  }, 5000);
});

Promise.all([
  invoke("set_pet_size", { width: pet.width, height: pet.height }),
  invoke("get_state"),
  invoke("get_layout"),
]).then(([, initial, savedLayout]) => {
  if (initial) state = initial;
  applyTheme(state.theme);
  applyLayout(savedLayout);
  draw();
});

setInterval(() => {
  frameIndex += 1;
  draw();
}, 400);

setInterval(() => {
  if (bubble.classList.contains("visible") && !promptTarget) updateElapsedLabels();
}, 1000);

updateLanguage();
applyLayout(layout);
draw();
