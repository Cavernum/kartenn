// Test plugin of the journeys of the new shell (spec 021, task 16): one extension of each
// point, the tile of the home page included, written without a framework against the SDK
// the shell maps (@kartenn/sdk 1.0). Its view `broken` throws when connected: its error
// stays in its place. Its views propose `g n` (free) and `g p` (the shell's, refused).
import { definePlugin } from "@kartenn/sdk";

let plugin;
const t = (key, params) => (plugin ? plugin.t(key, params) : key);

function paragraph(text) {
  const p = document.createElement("p");
  p.textContent = text;
  return p;
}

class Overview extends HTMLElement {
  connectedCallback() {
    const title = document.createElement("h1");
    title.textContent = t("view.title");
    const outcome = paragraph(t("view.trying"));
    this.replaceChildren(title, outcome);
    // A plugin may not reach another origin (CSP connect-src 'self', REQ-007-9).
    fetch("https://third-party.invalid/beacon").then(
      () => (outcome.textContent = t("view.reached")),
      () => (outcome.textContent = t("view.blocked")),
    );
  }
}

class Broken extends HTMLElement {
  connectedCallback() {
    throw new Error("e2e plugin element throws when connected");
  }
}

class Admin extends HTMLElement {
  connectedCallback() {
    const title = document.createElement("h1");
    title.textContent = t("nav.admin");
    this.replaceChildren(title);
  }
}

class Tab extends HTMLElement {
  connectedCallback() {
    this.replaceChildren(paragraph(t("tab.body", { type: this.object.type, id: this.object.id })));
  }
}

class Summary extends HTMLElement {
  connectedCallback() {
    const title = document.createElement("h2");
    title.textContent = t("panel.title");
    this.replaceChildren(title, paragraph(this.object.id));
  }
}

class Flag extends HTMLElement {
  connectedCallback() {
    this.textContent = "•";
  }
}

class Review extends HTMLElement {
  connectedCallback() {
    const close = document.createElement("button");
    close.type = "button";
    close.textContent = t("action.close");
    close.addEventListener("click", () =>
      this.dispatchEvent(new Event("kartenn-close", { bubbles: true, composed: true })),
    );
    this.replaceChildren(paragraph(t("action.body", { id: this.object.id })), close);
  }
}

class Home extends HTMLElement {
  connectedCallback() {
    this.replaceChildren(paragraph(t("widget.body")));
  }
}

plugin = definePlugin({
  name: "notes",
  elements: {
    "notes-overview": Overview,
    "notes-broken": Broken,
    "notes-admin": Admin,
    "notes-tab": Tab,
    "notes-summary": Summary,
    "notes-flag": Flag,
    "notes-review": Review,
    "notes-home": Home,
  },
});
