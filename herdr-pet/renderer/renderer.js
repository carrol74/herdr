const PIXEL = 5;
const GAP = 8;
const MARGIN = 6;

const COLORS = {
  k: "rgb(40 40 46)",
  b: "rgb(70 90 120)",
  g: "rgb(120 140 160)",
  w: "rgb(230 235 245)",
  o: "rgb(240 160 90)",
  r: "rgb(220 90 90)",
  y: "rgb(235 205 110)",
  t: "rgb(90 190 170)",
};

const FRAMES = {
  sleeping: [
    ["....yy....", "...yyy....", "...kkk....", "..kkkkk...", ".kkwwk.kk.", ".kbwk.kbk.", ".kkkkkkk..", "..kk.kk...", "..k...k..."],
    ["....yy....", "...yyy....", "...kkk....", "..kkkkk...", ".kkwwk.kk.", ".kbwk.kbk.", ".kkkkkkk..", "...kk.kk..", "..k...k..."],
  ],
  working: [
    ["....oo....", "...ooo....", "...kkk....", "..kkkkk...", ".kkwwkkk..", ".kbwk.kbk.", ".kkkkkkk..", "..kk.kk...", "..k...k..."],
    ["....oo....", "...ooo....", "...kkk....", "..kkkkk...", ".kkwwkkk..", ".kbwk.kbk.", ".kkkkkkk..", "...kk.kk..", "..k...k..."],
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
const contextMenu = document.getElementById("context-menu");
const quitButton = document.getElementById("quit");
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let state = { connection: "offline", mood: "offline", subject: null, offlineReason: null };
let frameIndex = 0;
let hovered = false;

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
}

function bubbleLines() {
  if (!state.subject) return [state.offlineReason || "没有运行中的 agent"];
  const subject = state.subject;
  return [
    `${subject.agent || "agent"} · ${subject.status}`,
    subject.title,
    subject.cwd,
  ].filter(Boolean);
}

function positionBubble() {
  const petRect = pet.getBoundingClientRect();
  bubbleContent.style.maxHeight = `calc(100vh - ${MARGIN * 2}px)`;
  let bubbleRect = bubble.getBoundingClientRect();
  const spaceAbove = petRect.top - GAP - MARGIN;
  const spaceBelow = innerHeight - petRect.bottom - GAP - MARGIN;
  const above = spaceAbove >= bubbleRect.height || spaceAbove >= spaceBelow;
  const availableHeight = Math.max(1, above ? spaceAbove : spaceBelow);
  bubbleContent.style.maxHeight = `${Math.round(availableHeight)}px`;
  bubbleRect = bubble.getBoundingClientRect();
  const left = clamp(
    petRect.left + petRect.width / 2 - bubbleRect.width / 2,
    MARGIN,
    innerWidth - bubbleRect.width - MARGIN,
  );
  const top = above ? petRect.top - GAP - bubbleRect.height : petRect.bottom + GAP;
  bubble.dataset.side = above ? "above" : "below";
  bubble.style.left = `${Math.round(left)}px`;
  bubble.style.top = `${Math.round(clamp(top, MARGIN, innerHeight - bubbleRect.height - MARGIN))}px`;
  bubble.style.setProperty(
    "--tail-x",
    `${Math.round(clamp(petRect.left + petRect.width / 2 - left, 14, bubbleRect.width - 14))}px`,
  );
}

function clamp(value, minimum, maximum) {
  return Math.max(minimum, Math.min(value, maximum));
}

function showBubble() {
  const lines = bubbleLines();
  bubbleContent.replaceChildren(
    ...lines.map((line, index) => {
      const element = document.createElement("div");
      element.className = index === 0 ? "bubble-title" : "bubble-line";
      element.textContent = line;
      return element;
    }),
  );
  bubble.classList.add("visible");
  positionBubble();
}

function hideBubble() {
  bubble.classList.remove("visible");
}

function finishDrag() {
  pet.classList.remove("dragging");
}

pet.addEventListener("pointerdown", (event) => {
  if (event.button !== 0) return;
  event.preventDefault();
  hideBubble();
  contextMenu.classList.remove("visible");
  pet.classList.add("dragging");
  invoke("start_drag").finally(finishDrag);
});

for (const eventName of ["pointerup", "pointercancel", "lostpointercapture", "mouseleave"]) {
  pet.addEventListener(eventName, finishDrag);
}

pet.addEventListener("mouseenter", () => {
  hovered = true;
  showBubble();
});

pet.addEventListener("mouseleave", () => {
  hovered = false;
  hideBubble();
});

pet.addEventListener("contextmenu", (event) => {
  event.preventDefault();
  hideBubble();
  const width = 150;
  contextMenu.classList.add("visible");
  contextMenu.style.left = `${clamp(event.clientX, MARGIN, innerWidth - width - MARGIN)}px`;
  contextMenu.style.top = `${clamp(event.clientY, MARGIN, innerHeight - contextMenu.offsetHeight - MARGIN)}px`;
});

document.addEventListener("pointerdown", (event) => {
  if (!contextMenu.contains(event.target) && event.target !== pet) {
    contextMenu.classList.remove("visible");
  }
});
quitButton.addEventListener("click", () => invoke("quit"));
window.addEventListener("resize", () => hovered && showBubble());

listen("pet-state", (event) => {
  state = event.payload;
  draw();
  if (hovered) showBubble();
});

invoke("get_state").then((initial) => {
  if (initial) state = initial;
  draw();
});

setInterval(() => {
  frameIndex += 1;
  draw();
}, 400);

draw();
