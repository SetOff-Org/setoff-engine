import init, { net } from "./pkg/setoff_engine_wasm.js";

const EXAMPLES = {
  corridor: [
    ["anchor-ng", "anchor-us", "USDC", "450"], ["anchor-us", "anchor-ng", "USDC", "300"],
    ["anchor-ng", "anchor-ke", "USDC", "250"], ["anchor-ke", "anchor-ng", "USDC", "110"],
    ["anchor-us", "anchor-ke", "USDC", "35"], ["anchor-ng", "anchor-eu", "EURC", "70"],
    ["anchor-eu", "anchor-ng", "EURC", "65"],
  ],
  cycle: [["A", "B", "USDC", "500"], ["B", "C", "USDC", "500"], ["C", "A", "USDC", "500"]],
  merchant: [["alice", "shop", "USDC", "40"], ["bob", "shop", "USDC", "25"], ["carol", "shop", "USDC", "60"], ["dan", "shop", "USDC", "15"]],
  random: null, // generated
};

const rowsEl = document.getElementById("rows");
const resultsEl = document.getElementById("results");
const emptyEl = document.getElementById("empty");
const strategyEl = document.getElementById("strategy");
let rows = [];
let ready = false;

// Amounts are integers in each asset's smallest unit. By default they are
// shown as typed; a preset that knows an asset's decimals (the mainnet day:
// USDC in stroops, 7) sets them here so the page shows real USDC.
let decimals = {};
const fmt = (s, asset) => {
  const v = BigInt(s), d = decimals[asset] || 0;
  if (!d) return v.toLocaleString("en-US");
  const neg = v < 0n, a = neg ? -v : v, base = 10n ** BigInt(d);
  const frac = (a % base).toString().padStart(d, "0").slice(0, 2);
  return `${neg ? "-" : ""}${(a / base).toLocaleString("en-US")}.${frac}`;
};
const clean = (s) => String(s).trim().replace(/[,\s_]/g, "");

function randomRows(n) {
  const people = ["anchor-ng", "anchor-us", "anchor-ke", "anchor-eu", "psp-a", "psp-b", "wallet-x"];
  const out = [];
  for (let i = 0; i < n; i++) {
    const d = people[Math.floor(Math.random() * people.length)];
    let c = d;
    while (c === d) c = people[Math.floor(Math.random() * people.length)];
    out.push([d, c, "USDC", String(10 + Math.floor(Math.random() * 990))]);
  }
  return out;
}

function load(list, units = {}) {
  decimals = units;
  rows = list.map(([debtor, creditor, asset, amount]) => ({ debtor, creditor, asset, amount }));
  render();
}

function render() {
  rowsEl.replaceChildren();
  emptyEl.hidden = rows.length > 0;
  rows.forEach((r, i) => {
    const tr = document.createElement("tr");
    tr.dataset.index = i;
    for (const field of ["debtor", "creditor", "asset", "amount"]) {
      const td = document.createElement("td");
      const input = document.createElement("input");
      input.value = r[field];
      input.setAttribute("aria-label", `${field} of obligation ${i + 1}`);
      input.placeholder = field[0].toUpperCase() + field.slice(1);
      if (field === "amount") { input.className = "num"; input.inputMode = "numeric"; }
      if (field === "asset") input.maxLength = 12;
      input.addEventListener("input", () => { r[field] = input.value; schedule(); });
      td.append(input);
      tr.append(td);
    }
    const td = document.createElement("td");
    const rm = document.createElement("button");
    rm.type = "button"; rm.className = "remove"; rm.textContent = "×";
    rm.setAttribute("aria-label", `Remove obligation ${i + 1}`);
    rm.addEventListener("click", () => { rows.splice(i, 1); render(); });
    td.append(rm);
    tr.append(td);
    rowsEl.append(tr);
  });
  compute();
}

let timer;
function schedule() { clearTimeout(timer); timer = setTimeout(compute, 150); }

function showRowErrors(errors) {
  rowsEl.querySelectorAll("tr.err").forEach((e) => e.remove());
  rowsEl.querySelectorAll("tr.bad").forEach((e) => e.classList.remove("bad"));
  for (const { index, message } of errors) {
    const tr = rowsEl.querySelector(`tr[data-index="${index}"]`);
    if (!tr) continue;
    tr.classList.add("bad");
    const er = document.createElement("tr");
    er.className = "err";
    const td = document.createElement("td");
    td.colSpan = 5;
    td.textContent = explain(message);
    er.append(td);
    tr.after(er);
  }
}

// Turn engine messages into plain language.
function explain(m) {
  if (/same/.test(m)) return "The debtor and creditor are the same participant; nobody owes themselves.";
  if (/positive/.test(m)) return "The amount must be more than zero.";
  if (/canonical|integer|invalid/i.test(m)) return "The amount must be a whole number, like 250.";
  if (/empty/.test(m)) return "Fill in every field: debtor, creditor, asset and amount.";
  return m;
}

function compute() {
  if (!ready) return;
  showRowErrors([]);
  if (rows.length === 0) {
    resultsEl.innerHTML = `<p class="muted">Results appear here as soon as there is an obligation.</p>`;
    return;
  }
  const obligations = rows.map((r, i) => ({
    id: String(i + 1), debtor: r.debtor.trim(), creditor: r.creditor.trim(), asset: r.asset.trim(), amount: clean(r.amount),
  }));
  const out = JSON.parse(net(JSON.stringify(obligations), strategyEl.value));
  if (!out.ok) {
    const rowErrors = out.errors.filter((e) => e.index >= 0);
    showRowErrors(rowErrors);
    resultsEl.innerHTML = "";
    const box = document.createElement("div");
    box.className = "error-box";
    box.textContent = rowErrors.length
      ? `${rowErrors.length === 1 ? "One obligation needs" : `${rowErrors.length} obligations need`} fixing before the window can net. The problem is shown under each row.`
      : `The obligations couldn't be read: ${out.errors[0].message}`;
    resultsEl.append(box);
    return;
  }
  renderResult(out.netting, obligations.length);
}

function renderResult(n, count) {
  const pct = (g, s) => (g === 0n ? 0 : Number(((g - s) * 10000n) / g) / 100);
  const transfers = n.transfers.length;
  const participants = new Set(n.positions.map((p) => p.participant)).size;
  // Amounts in different assets are never added together: savings are per asset.
  const saved = n.assets
    .map((a) => `<b>${pct(BigInt(a.gross), BigInt(a.settled)).toFixed(1)}%</b><span>${esc(a.asset)} liquidity saved</span>`)
    .join("");
  let html = `<div class="summary">
    <div class="stat">${saved}</div>
    <div class="stat"><b>${count} → ${transfers}</b><span>payments become transfers</span></div>
    <div class="stat"><b>${participants}</b><span>participant${participants === 1 ? "" : "s"} with a net position</span></div>
  </div>`;
  for (const a of n.assets) {
    const g = BigInt(a.gross), s = BigInt(a.settled);
    const w = g === 0n ? 0 : Number((s * 1000n) / g) / 10;
    html += `<div class="asset"><h3>${esc(a.asset)}</h3>
      <div class="bars">
        <div class="bar"><span>Owed ${fmt(g, a.asset)}</span><i style="width:100%"></i></div>
        <div class="bar"><span>Moves ${fmt(s, a.asset)}</span><i class="settled" style="width:${Math.max(w, s > 0n ? 1 : 0)}%"></i></div>
      </div></div>`;
  }
  html += `<h3>Settlement plan</h3><ol class="plan">`;
  html += transfers === 0
    ? `<li class="none">Nothing moves: every position nets to zero.</li>`
    : n.transfers.map((t) => `<li><b>${esc(t.from)}</b> pays <b>${esc(t.to)}</b> ${fmt(t.amount, t.asset)} ${esc(t.asset)}</li>`).join("");
  html += `</ol>`;
  if (n.positions.length) {
    html += `<h3>Net positions</h3><table class="positions"><thead><tr><th>Participant</th><th>Asset</th><th class="num">Net</th></tr></thead><tbody>`;
    html += n.positions.map((p) => {
      const v = BigInt(p.net);
      return `<tr><td>${esc(p.participant)}</td><td>${esc(p.asset)}</td><td class="num ${v > 0n ? "pos" : "neg"}">${v > 0n ? "+" : ""}${fmt(p.net, p.asset)}</td></tr>`;
    }).join("");
    html += `</tbody></table>`;
  }
  resultsEl.innerHTML = html;
}

function esc(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}

// Scenario in the URL, so a link reopens the same window.
function encode() { return btoa(unescape(encodeURIComponent(JSON.stringify(rows.map((r) => [r.debtor, r.creditor, r.asset, r.amount]))))); }
function decode(h) { try { return JSON.parse(decodeURIComponent(escape(atob(h)))); } catch { return null; } }

const mainnetNote = document.getElementById("mainnet-note");
document.querySelectorAll("[data-example]").forEach((b) => b.addEventListener("click", () => {
  mainnetNote.hidden = true;
  csvErrors.hidden = true;
  load(b.dataset.example === "random" ? randomRows(40) : EXAMPLES[b.dataset.example]);
}));

// CSV in the format `setoff net` and the engine's read_csv take:
// id,debtor,creditor,asset,amount. Quoted fields are allowed. Returns the
// rows and one message per line that could not be read; good lines load.
const CSV_HEADER = ["id", "debtor", "creditor", "asset", "amount"];
function parseCSV(text) {
  const records = [];
  let field = "", record = [], quoted = false, line = 1, start = 1;
  const endField = () => { record.push(field); field = ""; };
  const endRecord = () => { endField(); records.push({ line: start, fields: record }); record = []; start = line; };
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (quoted) {
      if (ch === '"' && text[i + 1] === '"') { field += '"'; i++; }
      else if (ch === '"') quoted = false;
      else { if (ch === "\n") line++; field += ch; }
    } else if (ch === '"') quoted = true;
    else if (ch === ",") endField();
    else if (ch === "\n") { line++; endRecord(); }
    else if (ch !== "\r") field += ch;
  }
  if (field !== "" || record.length) endRecord();
  const rows = [], errors = [];
  const [head, ...body] = records.filter((r) => r.fields.some((f) => f.trim() !== ""));
  if (!head || head.fields.map((f) => f.trim()).join(",") !== CSV_HEADER.join(",")) {
    errors.push(`line 1: the header must be ${CSV_HEADER.join(",")}`);
    return { rows, errors };
  }
  for (const { line: n, fields } of body) {
    if (fields.length !== CSV_HEADER.length) { errors.push(`line ${n}: expected ${CSV_HEADER.length} fields, found ${fields.length}`); continue; }
    const [, debtor, creditor, asset, amount] = fields.map((f) => f.trim());
    if (!/^(0|[1-9][0-9]*)$/.test(amount)) { errors.push(`line ${n}: amount ${JSON.stringify(amount)} is not a whole number`); continue; }
    rows.push([debtor, creditor, asset, amount]);
  }
  return { rows, errors };
}

const csvErrors = document.getElementById("csv-errors");
function showCSVErrors(errors, name) {
  csvErrors.hidden = errors.length === 0;
  if (!errors.length) return;
  const shown = errors.slice(0, 20).map((e) => `<li>${esc(e)}</li>`).join("");
  const more = errors.length > 20 ? `<li>…and ${errors.length - 20} more</li>` : "";
  csvErrors.innerHTML = `<strong>${errors.length} line${errors.length === 1 ? "" : "s"} of ${esc(name)} could not be read and ${errors.length === 1 ? "was" : "were"} skipped:</strong><ul>${shown}${more}</ul>`;
}

document.getElementById("csv-file").addEventListener("change", async (e) => {
  const file = e.target.files[0];
  e.target.value = ""; // the same file can be chosen again after a fix
  if (!file) return;
  mainnetNote.hidden = true;
  try {
    const { rows: list, errors } = parseCSV(await file.text());
    showCSVErrors(errors, file.name);
    if (list.length) {
      load(list);
      history.replaceState(null, "", location.pathname);
    } else if (!errors.length) {
      showCSVErrors(["the file has a header but no obligations"], file.name);
    }
  } catch (err) {
    showCSVErrors([`could not read the file: ${err.message}`], file.name);
  }
});

// One UTC day of real USDC payments between institutions on Stellar mainnet,
// built by tools/mainnet_day.py. Amounts stay exact, in stroops (10^-7 USDC).
const MAINNET = { file: "samples/mainnet-day.csv", day: "2026-10-21" };
const mainnetBtn = document.getElementById("mainnet");
mainnetBtn.addEventListener("click", async () => {
  mainnetBtn.disabled = true;
  mainnetNote.hidden = false;
  csvErrors.hidden = true;
  mainnetNote.textContent = "Loading the day's payments…";
  try {
    const res = await fetch(MAINNET.file);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const { rows: list, errors } = parseCSV(await res.text());
    if (errors.length) throw new Error(errors[0]);
    load(list, { USDC: 7 });
    mainnetNote.innerHTML = `${list.length.toLocaleString("en-US")} real USDC payments made on ${MAINNET.day} (UTC) between institutions
      that <a href="https://stellar.expert/directory">stellar.expert's directory</a> lists as exchanges, anchors, custodians or wallets,
      grouped by institution. The table holds amounts in stroops (10<sup>−7</sup> USDC), exactly as recorded on chain; results are shown in USDC.
      <a href="https://github.com/SetOff-Org/setoff-engine/blob/main/docs/mainnet-day.md">How this was built</a>.`;
    history.replaceState(null, "", location.pathname);
  } catch (e) {
    mainnetNote.textContent = `The mainnet sample couldn't be loaded (${e.message}). Check your connection and try again.`;
  } finally {
    mainnetBtn.disabled = false;
  }
});
document.getElementById("add").addEventListener("click", () => {
  rows.push({ debtor: "", creditor: "", asset: rows.at(-1)?.asset || "USDC", amount: "" });
  render();
  rowsEl.querySelector("tr:last-child input")?.focus();
});
document.getElementById("clear").addEventListener("click", () => { rows = []; render(); });
strategyEl.addEventListener("change", compute);
document.getElementById("share").addEventListener("click", async () => {
  const url = `${location.origin}${location.pathname}#s=${encode()}`;
  history.replaceState(null, "", url);
  const status = document.getElementById("share-status");
  try { await navigator.clipboard.writeText(url); status.textContent = "Link copied."; }
  catch { status.textContent = "Link is in the address bar."; }
  setTimeout(() => (status.textContent = ""), 3000);
});

try {
  await init();
  ready = true;
} catch (e) {
  resultsEl.innerHTML = `<div class="error-box">The netting engine couldn't load, so nothing can be computed. Your browser may block WebAssembly; try reloading, or a current Chrome, Firefox or Safari.</div>`;
}
const shared = location.hash.startsWith("#s=") ? decode(location.hash.slice(3)) : null;
load(shared && Array.isArray(shared) ? shared : EXAMPLES.corridor);
