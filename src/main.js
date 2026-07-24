import { formatHeld, verdictText, holderDetail } from "./verdict.js";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const sheet = document.getElementById("sheet");
const title = document.getElementById("title");
const sub = document.getElementById("sub");
const list = document.getElementById("holders");
const power = document.getElementById("power");
const checked = document.getElementById("checked");

function dlog(msg) {
  try {
    invoke("js_log", { msg: String(msg) });
  } catch (_) {
    /* the log is a convenience, never a reason to break the popover */
  }
}

function render(verdict) {
  const words = verdictText(verdict);
  sheet.dataset.mood = verdict.mood;
  title.textContent = words.title;
  sub.textContent = words.sub;
  power.textContent = verdict.on_battery ? "On battery" : "Plugged in";
  checked.textContent = "Checked just now";

  list.replaceChildren();
  for (const holder of verdict.holders) {
    list.append(row(holder));
  }

  fit();
}

// The window is sized by what is in it, not by a row-height guessed in Rust:
// the same list rendered at a different text size would then be clipped or
// float in empty space.
function fit() {
  requestAnimationFrame(() => {
    const height = Math.ceil(sheet.getBoundingClientRect().height);
    invoke("fit_popover", { height }).catch((e) => dlog(`resize failed: ${e}`));
  });
}

// App names come from other people's software, so they go in as text and never
// as markup.
function row(holder) {
  const li = document.createElement("li");

  const who = document.createElement("div");
  who.className = "who";
  const name = document.createElement("b");
  name.textContent = holder.app;
  const detail = document.createElement("span");
  detail.textContent = holderDetail(holder);
  who.append(name, detail);

  const held = document.createElement("span");
  held.className = "held";
  held.textContent = formatHeld(holder.held);

  const close = document.createElement("button");
  close.className = "quit";
  close.textContent = "×";
  close.title = `Quit ${holder.app}`;
  close.setAttribute("aria-label", `Quit ${holder.app}`);
  close.addEventListener("click", async () => {
    close.disabled = true;
    try {
      await invoke("close_holder", { pid: holder.pid });
    } catch (e) {
      // The app was already gone, or it is not ours to close. Say so where the
      // name was, rather than leaving a dead cross.
      close.remove();
      detail.textContent = String(e);
    }
  });

  li.append(who, held, close);
  return li;
}

listen("verdict-changed", (event) => render(event.payload));

invoke("get_verdict")
  .then(render)
  .catch((e) => dlog(`first read failed: ${e}`));
