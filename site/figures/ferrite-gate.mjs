// Fig: the consent gate as a punch-card verifier.
// The task's predicted fingerprint is a template plate with a hole for every
// capability the task may use. Each planned agent action arrives as a card
// punched in one column. Feeler pins drop: a pin that passes the template and
// the card's hole touches the contact strip (predicted, it runs). A pin the
// template stops means the action was not predicted: a stop gate rises and the
// card is held until you approve or deny it. The js.execute column is solid on
// every template, because js.execute can never be predicted.
import { emit, k, tools } from "./common.mjs";

const WIDTH = 640;
const HEIGHT = 360;

export const CAPS = ["web.read", "web.navigate", "web.interact", "web.download", "scoped.read", "clipboard.read", "clipboard.write", "js.execute"];
const COLS = [-21, -15, -9, -3, 3, 9, 15, 21];
const ALLOWED = [0, 1, 2]; // the task: "find the cheapest flight to Lisbon"
const JS = 7;

const PLATE = { x: -138, y: -58, w: 276, d: 116, r: 9 };
const PLATE_H = 12;
const FOOT_H = 5;

const RAIL_X0 = -98;
const RAIL_X1 = 98;
const RAILS = [-22, 22];
const RAIL_H = 3;
const CAR_Z = RAIL_H; // carriage bars sit on the rails
const CAR_TOP = 6;
const CARD_Z = CAR_TOP;
const CARD_T = 0.8;
const CARD_TOP = CARD_Z + CARD_T;
const START = -76;
const END = 76;
const GATE_X = 42;
const WAIT = GATE_X - 18; // carriage front face meets the gate
const GATE_H = 16;

const POST_Y = 38;
const TPL_Z = 12.8;
const TPL_T = 1.2;
const TPL_TOP = TPL_Z + TPL_T;
const BLOCK_Z = 26;
const BLOCK_TOP = 44;
const BEAM_TOP = 50;
const PIN_R = 1.3;
const PIN_L = 27;
const PIN_REST = BLOCK_TOP - 1 - PIN_L; // bottom of a pin at rest: 16
const PIN_THROUGH = 0.6; // touches the contact strip

const P = k.fitProjection(
  [...k.boxCorners(PLATE, -PLATE_H - FOOT_H, 0), [0, -POST_Y - 6, BEAM_TOP + 4], [0, POST_Y + 6, BEAM_TOP + 4], [48, -38, 40]],
  WIDTH,
  HEIGHT,
  { pad: 18, azimuth: Number(process.env.AZ || 58) },
);
const T = tools(P);
const { at, S, L, D } = T;

const hole = (x, y, r, z) => `<path class="fig-hole" d="${k.ring(x, y, r, z, P, 18)}"/>`;

function base() {
  const out = [T.basePlate(PLATE, PLATE_H, FOOT_H, { footInset: 22 })];
  out.push(T.ruler({ x: -124, y: 45, w: 150, d: 7, r: 1 }, 0));
  out.push(T.labelPlate({ x: 70, y: 37, w: 42, d: 14, r: 1.2 }, 0));
  // Contact strip under the reader: a pin that reaches it means "predicted".
  const strip = { x: -4.5, y: -27, w: 9, d: 54, r: 1.2 };
  out.push(S(k.slabOf(strip, 0, 0.6, P, 3), { tone: "lo", crease: "none" }));
  out.push(`<g class="fig-contacts">${COLS.map((y, i) => `<path class="iso-line" data-tone="faint" data-col="${i}" d="${k.ring(0, y, 1.9, 0.6, P, 16)}"/>`).join("")}</g>`);
  // Slot the stop gate rises from.
  out.push(L(k.planOutline({ x: GATE_X - 0.6, y: -28, w: 5.2, d: 56, r: 1 }, 0, P, 3), { tone: "lo" }));
  out.push(`<path class="fig-hole" d="${k.onTop(k.ringOf({ x: GATE_X - 0.2, y: -27.6, w: 4.4, d: 55.2, r: 0.8 }, 3), 0, P, true)}"/>`);
  return out.join("");
}

function rails() {
  const out = [];
  for (const y of RAILS) {
    out.push(S(k.slabOf({ x: RAIL_X0, y: y - 2.5, w: RAIL_X1 - RAIL_X0, d: 5, r: 1 }, 0, RAIL_H, P, 3, 0.4), { tone: "mid" }));
    out.push(D([RAIL_X0 + 8, -50, 50, RAIL_X1 - 8].map((x) => at([x, y, RAIL_H])), { size: 0.4 }));
  }
  for (const x of [RAIL_X0 - 5, RAIL_X1]) {
    out.push(S(k.slabOf({ x, y: -27, w: 5, d: 54, r: 1.2 }, 0, 7, P, 3, 0.5), { tone: "mid" }));
    out.push(D([at([x + 2.5, -21, 7]), at([x + 2.5, 21, 7])], { size: 0.45 }));
  }
  return out.join("");
}

// A stack of cards in a tray with three walls (hopper on the left, out tray on the right).
function trayWalls(x0, open) {
  const out = [];
  const W = 28;
  const plan = { x: x0, y: -30, w: W, d: 60, r: 2 };
  out.push(T.halo(plan, 0, 2));
  out.push(S(k.slabOf(plan, 0, 2, P, 4, 0.5), { tone: "mid" }));
  const back = open === "right" ? { x: x0, y: -30, w: 2.4, d: 60, r: 0.8 } : { x: x0 + W - 2.4, y: -30, w: 2.4, d: 60, r: 0.8 };
  return { plan, out, back };
}

function cardStack(x0, count, idPrefix) {
  const out = [];
  for (let i = 0; i < count; i++) {
    const z = 2 + i * 1.6;
    const plan = { x: x0 + 2, y: -26, w: 24, d: 52, r: 1.6 };
    out.push(`<g data-${idPrefix}="${i}">${S(k.slabOf(plan, z, CARD_T, P, 4), { tone: "hi", crease: "lo" })}</g>`);
  }
  return out.join("");
}

function hopper() {
  const x0 = -134;
  const { out, back } = trayWalls(x0, "right");
  // Back wall (far, -x side) then the side walls; the stack sits between them.
  out.push(S(k.slabOf(back, 2, 18, P, 3, 0.4), { tone: "mid" }));
  out.push(S(k.slabOf({ x: x0, y: -30, w: 28, d: 2.4, r: 0.8 }, 2, 18, P, 3, 0.4), { tone: "mid" }));
  out.push(`<g data-part="hopper-stack">${cardStack(x0, 8, "hop")}</g>`);
  out.push(S(k.slabOf({ x: x0, y: 27.6, w: 28, d: 2.4, r: 0.8 }, 2, 18, P, 3, 0.4), { tone: "mid" }));
  out.push(D([at([x0 + 6, 28.8, 20]), at([x0 + 22, 28.8, 20])], { size: 0.45 }));
  return out.join("");
}

function outTray() {
  const x0 = 106;
  const { out, back } = trayWalls(x0, "left");
  out.push(S(k.slabOf({ x: x0, y: -30, w: 28, d: 2.4, r: 0.8 }, 2, 10, P, 3, 0.4), { tone: "mid" }));
  out.push(`<g data-part="out-stack">${cardStack(x0, 8, "out")}</g>`);
  out.push(S(k.slabOf(back, 2, 10, P, 3, 0.4), { tone: "mid" }));
  out.push(S(k.slabOf({ x: x0, y: 27.6, w: 28, d: 2.4, r: 0.8 }, 2, 10, P, 3, 0.4), { tone: "mid" }));
  out.push(D([at([x0 + 6, 28.8, 12]), at([x0 + 22, 28.8, 12])], { size: 0.45 }));
  return out.join("");
}

// The carriage, built at x = 0 and moved with translateAlong.
function carriage() {
  const out = [T.halo({ x: -16, y: -24, w: 32, d: 48, r: 3 }, CAR_Z, 0.1, 4)];
  for (const y of RAILS) {
    out.push(S(k.slabOf({ x: -18, y: y - 3.5, w: 36, d: 7, r: 1.4 }, CAR_Z, CAR_TOP - CAR_Z, P, 3, 0.5), { tone: "mid" }));
    out.push(D([at([-13, y, CAR_TOP]), at([13, y, CAR_TOP])], { size: 0.42 }));
  }
  for (const x of [-18, 14]) {
    out.push(S(k.slabOf({ x, y: -18.5, w: 4, d: 37, r: 0.8 }, CAR_Z + 0.5, CAR_TOP - CAR_Z - 0.5, P, 3), { tone: "mid", crease: "lo" }));
  }
  // Handle on the near bar.
  out.push(S(k.slabOf({ x: 8, y: 25.5, w: 4, d: 3, r: 0.8 }, CAR_Z + 0.6, 2.4, P, 2), { tone: "mid", crease: "none" }));
  out.push(S(k.cylinder(10, 29.6, 2.6, CAR_Z, 4.6, P, 20), { tone: "mid" }));
  out.push(L(k.knurl(10, 29.6, 2.6, CAR_Z, CAR_Z + 4.6, 18, P), { tone: "lo" }));
  out.push(`<g data-part="card"></g>`);
  return out.join("");
}

// One card, punched in column `col`, drawn at the carriage origin.
export function cardSvg(col) {
  const plan = { x: -12, y: -26, w: 24, d: 52, r: 1.6 };
  const out = [S(k.slabOf(plan, CARD_Z, CARD_T, P, 4), { tone: "hi", crease: "lo", lit: true })];
  // Printed column marks and the punched hole.
  out.push(D(COLS.filter((_, i) => i !== col).map((y) => at([0, y, CARD_TOP])), { size: 0.38, tone: "lo" }));
  out.push(hole(0, COLS[col], 1.6, CARD_TOP));
  out.push(L(k.ring(0, COLS[col], 1.6, CARD_TOP, P, 18), { tone: "hi" }));
  // Print: three rule lines near the far edge stand for the action's text.
  [-6, -3, 0].forEach((dx, i) => {
    const x = -8 + i * 3;
    out.push(L(k.lineOnTop([x, -23.5], [x, -23.5 + [14, 10, 6][i]], CARD_TOP, P), { tone: "lo", free: true }));
  });
  out.push(L(k.lineOnTop([8, 24], [8, 12], CARD_TOP, P), { tone: "lo", free: true }));
  return out.join("");
}

function backPost() {
  return post(-POST_Y);
}
function post(y) {
  const out = [];
  out.push(S(k.cylinder(0, y, 5.6, 0, 3, P, 28), { tone: "lo" }));
  out.push(S(k.cylinder(0, y, 3.6, 3, BLOCK_TOP - 3, P, 28), { tone: "mid", crease: "none" }));
  out.push(L([12, 30].map((z) => k.sideArc(0, y, 3.6, z, P, 16)).join(""), { tone: "faint" }));
  return out.join("");
}

function template() {
  const out = [];
  const plan = { x: -3.8, y: -35, w: 7.6, d: 70, r: 1.2 };
  out.push(S(k.slabOf(plan, TPL_Z, TPL_T, P, 3, 0.4), { tone: "mid" }));
  COLS.forEach((y, i) => {
    if (ALLOWED.includes(i)) {
      out.push(hole(0, y, 1.8, TPL_TOP));
      out.push(L(k.ring(0, y, 1.8, TPL_TOP, P, 18), { tone: "mid" }));
    } else if (i === JS) {
      out.push(L(k.ring(0, y, 1.8, TPL_TOP, P, 18), { tone: "red" }));
      out.push(L(k.lineOnTop([-1.27, y - 1.27], [1.27, y + 1.27], TPL_TOP, P) + k.lineOnTop([-1.27, y + 1.27], [1.27, y - 1.27], TPL_TOP, P), { tone: "red" }));
    } else {
      out.push(D([at([0, y, TPL_TOP])], { size: 0.38, tone: "lo" }));
    }
  });
  out.push(D([at([0, -31.5, TPL_TOP]), at([0, 31.5, TPL_TOP])], { size: 0.42 }));
  return out.join("");
}

function head() {
  const out = [];
  const block = { x: -5, y: -25, w: 10, d: 50, r: 1.2 };
  out.push(S(k.slabOf(block, BLOCK_Z, BLOCK_TOP - BLOCK_Z, P, 3, 0.5), { tone: "mid" }));
  out.push(L(k.sideSeam(block, BLOCK_Z + 4, P, 3), { tone: "faint" }));
  out.push(`<g class="fig-inds">${COLS.map((y, i) => { const [sx, sy] = at([5, y, BLOCK_Z + 9]); return `<circle class="fig-ind" data-ind="${i}" cx="${sx.toFixed(1)}" cy="${sy.toFixed(1)}" r="1.25"/>`; }).join("")}</g>`);
  return out.join("");
}

function beam() {
  const out = [];
  const plan = { x: -7, y: -POST_Y - 4, w: 14, d: 2 * POST_Y + 8, r: 2 };
  out.push(T.halo(plan, BLOCK_TOP, 0.1, 6));
  out.push(S(k.slabOf(plan, BLOCK_TOP, BEAM_TOP - BLOCK_TOP, P, 4, 0.8), { tone: "mid" }));
  out.push(L(k.planOutline(k.insetPlan(plan, 2.4), BEAM_TOP, P, 4), { tone: "faint" }));
  out.push(D([-POST_Y, POST_Y].map((y) => at([0, y, BEAM_TOP])), { size: 0.5 }));
  out.push(L([-POST_Y, POST_Y].map((y) => k.ring(0, y, 2, BEAM_TOP, P, 16)).join(""), { tone: "lo" }));
  // Knurled adjuster for the pin head.
  out.push(S(k.cylinder(0, -12, 4, BEAM_TOP, 3, P, 28), { tone: "mid" }));
  out.push(L(k.knurl(0, -12, 4, BEAM_TOP, BEAM_TOP + 3, 28, P), { tone: "lo" }));
  out.push(L(k.lineOnTop([0, -12], [3, -12], BEAM_TOP + 3, P), { tone: "hi" }));
  return out.join("");
}

function lampPost() {
  const out = [];
  const [x, y] = [GATE_X + 2, -40];
  out.push(S(k.cylinder(x, y, 5, 0, 2.4, P, 24), { tone: "lo" }));
  out.push(S(k.cylinder(x, y, 2.2, 2.4, 26, P, 20), { tone: "mid", crease: "none" }));
  out.push(S(k.cylinder(x, y, 4.4, 28.4, 6, P, 28, 0.5), { tone: "mid" }));
  out.push(L(k.sideArc(x, y, 4.4, 31.4, P, 18), { tone: "faint" }));
  out.push(`<path class="fig-lamp" data-part="lamp" d="${k.cylinder(x, y, 3.4, 34.4, 3, P, 24).fill}"/>`);
  out.push(L(k.ring(x, y, 3.4, 37.4, P, 24), { tone: "mid" }));
  out.push(D([at([x, y, 37.4])], { size: 0.5, tone: "hi" }));
  return out.join("");
}

const svg = k.figureSvg({
  width: WIDTH,
  height: HEIGHT,
  label:
    "A punch-card verifier. A template plate under a row of eight feeler pins has holes only for the capabilities the task was predicted to need. A carriage brings each planned action, a card punched in one column, under the pins. A pin that passes the template and the card touches the contact strip and the card runs. A pin the template stops is a deviation: a stop gate rises and the card waits until you approve or deny it.",
  body: [
    base(),
    hopper(),
    rails(),
    backPost(),
    `<g data-part="pins-low"></g>`,
    `<g data-part="slot-a"><g data-part="carriage">${carriage()}</g></g>`,
    `<g data-part="pins-mid"></g>`,
    template(),
    `<g data-part="pins-high"></g>`,
    head(),
    post(POST_Y),
    beam(),
    lampPost(),
    `<g data-part="slot-b"></g>`,
    `<g data-part="gate"></g>`,
    outTray(),
  ],
});

const live = String.raw`
const P = D.P, COLS = D.COLS, CAPS = D.CAPS, ALLOWED = D.ALLOWED;
const stage = root.querySelector(".iso-stage");
const svg = stage.querySelector("svg");
const q = (name) => svg.querySelector('[data-part="' + name + '"]');
const car = q("carriage"), cardSlot = q("card"), slotA = q("slot-a"), slotB = q("slot-b");
const low = q("pins-low"), mid = q("pins-mid"), high = q("pins-high"), gate = q("gate"), lamp = q("lamp");
const hop = [...svg.querySelectorAll("[data-hop]")], outs = [...svg.querySelectorAll("[data-out]")];
const contacts = [...svg.querySelectorAll("[data-col]")];
const inds = [...svg.querySelectorAll("[data-ind]")];
const readout = root.querySelector("[data-readout]");
const still = matchMedia("(prefers-reduced-motion: reduce)");
const S = (paths, style) => k.solidSvg(paths, style);
const SCRIPT = D.SCRIPT;

let cx = D.START, target = D.START, gateH = 0, gateWant = 0, cardAlpha = 0;
const pins = COLS.map(() => ({ at: D.PIN_REST, rate: 0, want: D.PIN_REST }));
let phase = "load", clock = 0, step = 0, col = SCRIPT[0][0], decision = SCRIPT[0][1], ran = 0, held = 0;
let frame = 0, last = 0, visible = false, autoUntil = 0, queued = null, now = 0;
const say = (text) => { if (readout.textContent !== text) readout.textContent = text; };
const describe = (text) => stage.setAttribute("aria-valuetext", text);

function loadCard() {
  if (queued !== null) { col = queued; decision = "ask"; queued = null; }
  else { col = SCRIPT[step % SCRIPT.length][0]; decision = SCRIPT[step % SCRIPT.length][1]; step++; }
  cardSlot.innerHTML = D.CARDS[col];
  cardAlpha = 0;
  const left = 8 - (step % 8);
  hop.forEach((el, i) => el.style.opacity = i < left ? 1 : 0);
}
function pinTargets(probe) {
  COLS.forEach((_, i) => {
    let want = D.PIN_REST;
    if (probe) {
      if (!ALLOWED.includes(i)) want = D.TPL_TOP;          // stopped by the template
      else if (i === col) want = D.PIN_THROUGH;             // through template and card
      else want = D.CARD_TOP;                               // stopped by the card
    }
    pins[i].want = want;
  });
}
const allowed = () => ALLOWED.includes(col);
function drawPins() {
  let a = "", b = "", c = "";
  const order = COLS.map((y, i) => i);
  for (const i of order) {
    const y = COLS[i], bottom = pins[i].at, top = bottom + D.PIN_L;
    const tone = { tone: i === col && bottom < D.CARD_TOP - 0.2 ? "hi" : "mid", crease: "none" };
    if (bottom < D.CARD_Z - 0.01) a += S(k.cylinder(0, y, D.PIN_R, bottom, Math.min(D.CARD_Z, top) - bottom, P, 16), tone);
    const m0 = Math.max(bottom, D.CARD_TOP), m1 = Math.min(D.TPL_Z, top);
    if (m1 > m0 + 0.01) b += S(k.cylinder(0, y, D.PIN_R, m0, m1 - m0, P, 16), tone);
    const h0 = Math.max(bottom, D.TPL_TOP), h1 = Math.min(D.BLOCK_Z + 0.4, top);
    if (h1 > h0 + 0.01) c += S(k.cylinder(0, y, D.PIN_R, h0, h1 - h0, P, 16), tone);
  }
  low.innerHTML = a; mid.innerHTML = b; high.innerHTML = c;
  contacts.forEach((el, i) => el.setAttribute("data-tone", pins[i].at < D.PIN_THROUGH + 0.3 ? "green" : "faint"));
  inds.forEach((el, i) => { const through = pins[i].at < D.PIN_THROUGH + 0.3; const stopped = i === col && pins[i].want === D.TPL_TOP && pins[i].at < D.PIN_REST - 1; el.setAttribute("data-state", through ? "go" : stopped ? "stop" : ""); });
}
function drawGate() {
  if (gateH < 0.25) { gate.innerHTML = ""; return; }
  gate.innerHTML = S(k.slabOf({ x: D.GATE_X, y: -27, w: 4, d: 54, r: 0.8 }, 0, gateH, P, 3, 0.4), { tone: "mid" })
    + k.dotsSvg([k.iso([D.GATE_X + 2, -20, gateH], P), k.iso([D.GATE_X + 2, 20, gateH], P)], { size: 0.42 });
}
function draw() {
  car.setAttribute("transform", k.translateAlong(P, [cx, 0, 0]));
  const wantB = cx >= 20;
  if (wantB && car.parentNode !== slotB) slotB.appendChild(car);
  if (!wantB && car.parentNode !== slotA) slotA.appendChild(car);
  cardSlot.style.opacity = cardAlpha.toFixed(3);
  drawPins();
  drawGate();
  lamp.setAttribute("data-on", phase === "hold" || phase === "toGate" ? "1" : "0");
}
const name = () => CAPS[col];
function enter(next) { phase = next; clock = 0; }
function decide(yes) {
  if (phase !== "hold") return;
  if (yes) { enter("approved"); gateWant = 0; describe(name() + ": you approved it, it runs"); }
  else { enter("denied"); target = D.START; describe(name() + ": you denied it, nothing ran"); }
  run();
}
function tick(t) {
  now = t;
  const dt = last ? Math.min((t - last) / 1000, 1 / 30) : 1 / 60;
  last = t;
  const calm = still.matches;
  clock += dt;
  const ease = (v, w, tau) => calm ? w : v + (w - v) * (1 - Math.exp(-dt / tau));
  switch (phase) {
    case "load":
      cardAlpha = ease(cardAlpha, 1, 0.18);
      say("card " + ((ran + held) % 99 + 1) + " · " + name() + " · fed");
      if (clock > 0.7) { cardAlpha = 1; target = 0; enter("feed"); }
      break;
    case "feed":
      say(name() + " · to the reader");
      if (Math.abs(cx - target) < 0.05) { cx = target; pinTargets(true); enter("probe"); }
      break;
    case "probe":
      say(allowed() ? name() + " · in the fingerprint" : name() + " · not predicted · deviation");
      if (clock > 1.5) { pinTargets(false); enter(allowed() ? "lift" : "liftHold"); }
      break;
    case "lift":
      if (clock > 0.5) { target = D.END; enter("out"); describe(name() + ": predicted, it runs"); }
      break;
    case "liftHold":
      if (clock > 0.4) { gateWant = D.GATE_H; target = D.WAIT; enter("toGate"); }
      break;
    case "toGate":
      say(name() + " · held at the gate");
      if (Math.abs(cx - target) < 0.05 && Math.abs(gateH - gateWant) < 0.05) { cx = target; enter("hold"); held++; describe(name() + ": held, waiting for you. Enter approves, Escape denies"); }
      break;
    case "hold":
      say(name() + " · waiting for you");
      if (clock > (now > autoUntil ? 2.4 : 4.5)) decide(decision === "yes");
      break;
    case "approved":
      say(name() + " · you approved · runs");
      if (gateH < 0.25) { target = D.END; enter("out"); }
      break;
    case "denied":
      say(name() + " · denied · nothing ran");
      cardAlpha = ease(cardAlpha, 0, 0.35);
      if (Math.abs(cx - target) < 0.05) { cx = target; loadCard(); enter("load"); }
      break;
    case "out":
      if (allowed() || clock > 0) say(name() + " · ran");
      if (Math.abs(cx - target) < 0.05) {
        cx = target; ran++;
        outs.forEach((el, i) => el.style.opacity = i < Math.min(8, ran % 9) ? 1 : 0);
        cardAlpha = 0; target = D.START; enter("back");
      }
      break;
    case "back":
      say("ran " + ran + " · held " + held);
      if (Math.abs(cx - target) < 0.05) { cx = target; loadCard(); enter("load"); }
      break;
  }
  cx = ease(cx, target, 0.32);
  if (Math.abs(cx - target) < 0.04) cx = target;
  gateH = ease(gateH, gateWant, 0.22);
  if (Math.abs(gateH - gateWant) < 0.04) gateH = gateWant;
  for (const p of pins) {
    if (calm) { p.at = p.want; p.rate = 0; continue; }
    for (let s = 0; s < 4; s++) { p.rate += ((p.want - p.at) * 260 - p.rate * 15) * dt / 4; p.at += p.rate * dt / 4; }
    if (p.want > p.at && p.want - p.at < 0.004 && Math.abs(p.rate) < 0.01) { p.at = p.want; p.rate = 0; }
    // A pin can't sink into what stopped it.
    if (p.at < p.want - 0.001 && p.want < D.PIN_REST && p.rate < 0) { p.at = p.want; p.rate = -p.rate * 0.25; }
  }
  draw();
  frame = visible ? requestAnimationFrame(tick) : 0;
}
const run = () => { if (!frame && visible) { last = 0; frame = requestAnimationFrame(tick); } };
new IntersectionObserver((entries) => { visible = entries[entries.length - 1].isIntersecting; if (visible) run(); }, { rootMargin: "120px 0px" }).observe(stage);
const hush = () => { autoUntil = performance.now() + 6000; };
stage.addEventListener("click", () => { hush(); decide(true); });
stage.addEventListener("keydown", (event) => {
  if (event.key >= "1" && event.key <= "8") { hush(); queued = Number(event.key) - 1; say("next card · " + CAPS[queued]); event.preventDefault(); run(); return; }
  if (event.key === "Enter" || event.key === " ") { hush(); decide(true); event.preventDefault(); }
  if (event.key === "Escape" || event.key === "Backspace") { hush(); decide(false); event.preventDefault(); }
});
outs.forEach((el) => el.style.opacity = 0);
loadCard();
draw();
`;

const cards = CAPS.map((_, i) => cardSvg(i));

emit({
  name: "ferrite-gate",
  title: "Consent gate",
  svg,
  plate: {
    fig: "Fig 1",
    title: "Consent gate",
    hint: "Predicted runs · not predicted waits",
    readout: "card 1 · web.navigate · fed",
    aria: 'role="group" aria-label="Punch-card verifier: keys 1 to 8 punch the next card in that column, Enter approves a held card, Escape denies it"',
    keys: [
      { mark: "lit", label: "The card: one planned action, punched in its capability's column" },
      { mark: "raised", label: "Template: holes only where the task's fingerprint allows" },
      { mark: "red", label: "js.execute: solid on every template, so always held" },
    ],
    caption:
      "Task: “find the cheapest flight to Lisbon”, predicted to need web.read, web.navigate and web.interact. A pin that drops through the template and the card touches the contact strip, and the action runs. A pin the template stops means the action was not predicted, so the gate rises and nothing real happens until you decide.",
  },
  live,
  data: {
    P,
    COLS,
    CAPS,
    ALLOWED,
    START,
    END,
    WAIT,
    GATE_X,
    GATE_H,
    PIN_REST,
    PIN_L,
    PIN_R,
    PIN_THROUGH,
    CARD_Z,
    CARD_TOP,
    TPL_Z,
    TPL_TOP,
    BLOCK_Z,
    CARDS: cards,
    // [column, what "you" decide on the tour if it is held]
    SCRIPT: [[1, "yes"], [7, "no"], [0, "yes"], [6, "yes"], [2, "yes"], [3, "no"]],
  },
  css: `
.fig-hole{fill:var(--anatomy-shade-0);stroke:none}
.iso[data-theme="light"] .fig-hole,.fig-light .fig-hole{fill:var(--anatomy-shade-0)}
.fig-ind{fill:var(--anatomy-shade-0);stroke:var(--anatomy-lo);stroke-width:var(--anatomy-weight);transition:fill .3s}
.fig-ind[data-state="go"]{fill:var(--fig-go,#46d39a)}
.fig-ind[data-state="stop"]{fill:var(--fig-warn,#f5a524)}
.fig-lamp{fill:var(--anatomy-shade-1);stroke:var(--anatomy-mid);stroke-width:var(--anatomy-weight);transition:fill .35s var(--anatomy-ease)}
.fig-lamp[data-on="1"]{fill:var(--fig-warn,#f5a524)}
`,
});
