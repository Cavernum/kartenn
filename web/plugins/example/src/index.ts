// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Example plugin (spec 007, task 23; REQ-007-6, 11, 21): Custom Elements without a
// framework, written with the public SDK only. One extension of each point: a view, a tab
// and a panel on the record, a column in the lists, and an action that adds an operation
// to the draft. Each element draws in its own shadow root, styled by `styles.css` through
// `styles()` and the design tokens of the shell.

import {
  ApiProblem,
  api,
  context,
  definePlugin,
  draft,
  type ObjectRef,
  type PluginHandle,
} from "@kartenn/sdk";

/** What `definePlugin` gives back, once the elements are defined (at the end). */
const defined: { handle?: PluginHandle } = {};

/** A text of the plugin, under its namespace; the key itself before the definition. */
function t(key: string, params?: Record<string, unknown>): string {
  return defined.handle ? defined.handle.t(key, params) : key;
}

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  text?: string,
  className?: string,
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  if (text !== undefined) element.textContent = text;
  if (className) element.className = className;
  return element;
}

function detailOf(error: unknown): string {
  if (error instanceof ApiProblem) return error.detail ?? error.title;
  return error instanceof Error ? error.message : String(error);
}

/**
 * The common part of the elements: an open shadow root, the style sheet of the plugin
 * added once, and a redraw whenever the context of the shell changes (language, dates).
 */
abstract class ExampleElement extends HTMLElement {
  /** The object of the record (tab, panel, action) or of the row (column). */
  object: ObjectRef | null = null;
  protected readonly root: ShadowRoot = this.attachShadow({ mode: "open" });
  private styled = false;
  private unsubscribe: (() => void) | null = null;

  connectedCallback(): void {
    if (!this.styled && defined.handle) {
      this.styled = true;
      // Without its sheet the element stays readable: the tokens are inherited anyway.
      defined.handle.styles(this.root, "styles.css").catch(() => undefined);
    }
    this.unsubscribe = context.subscribe(() => this.render());
    this.render();
  }

  disconnectedCallback(): void {
    this.unsubscribe?.();
    this.unsubscribe = null;
  }

  /** Replaces the content of the shadow root, keeping the style sheet. */
  protected draw(...nodes: Node[]): void {
    const sheets = [...this.root.querySelectorAll("link[rel=stylesheet]")];
    this.root.replaceChildren(...sheets, ...nodes);
  }

  protected abstract render(): void;
}

/** The view: what the plugin is, the dates of the context, the size of the draft. */
class Overview extends ExampleElement {
  private stopDraft: (() => void) | null = null;

  override connectedCallback(): void {
    this.stopDraft = draft.subscribe(() => this.render());
    super.connectedCallback();
  }

  override disconnectedCallback(): void {
    this.stopDraft?.();
    this.stopDraft = null;
    super.disconnectedCallback();
  }

  protected render(): void {
    const card = el("section", undefined, "card");
    card.append(
      el("h1", t("view.title")),
      el("p", t("view.body")),
      el("p", t(context.get().time.present ? "view.present" : "view.dated"), "muted"),
      el("p", t("view.draft", { n: draft.operations().length })),
    );
    this.draw(card);
  }
}

/** The tab of a prefix: reads it through the client of the SDK, at the dates of the shell. */
class Details extends ExampleElement {
  private reading = 0;

  protected render(): void {
    const id = this.object?.id;
    if (!id) return;
    const reading = ++this.reading;
    const line = el("p", t("tab.loading"), "muted");
    line.setAttribute("role", "status");
    this.draw(el("section", undefined, "card"));
    this.root.querySelector("section")?.append(line);
    api.get("/v1/prefixes/{id}", { path: { id } }).then(
      ({ data }) => {
        if (reading !== this.reading) return;
        line.className = "";
        line.textContent = t("tab.value", {
          value: data.value,
          domain: data.domain.slug ?? data.domain.id,
        });
      },
      (error: unknown) => {
        if (reading !== this.reading) return;
        line.className = "error";
        line.textContent = t("tab.failed", { detail: detailOf(error) });
      },
    );
  }
}

/** The panel: a short summary of the object. */
class Summary extends ExampleElement {
  protected render(): void {
    const card = el("section", undefined, "card");
    card.append(
      el("h2", t("panel.title")),
      el("p", t("panel.body", { type: this.object?.type ?? "", id: this.object?.id ?? "" })),
    );
    this.draw(card);
  }
}

/** The column of the address lists: the row as the list read it. */
class Usage extends ExampleElement {
  row: Record<string, unknown> | null = null;

  protected render(): void {
    const reserved = this.row?.["usage"] === "reserved";
    this.draw(el("span", t(reserved ? "column.reserved" : "column.other")));
  }
}

/** The action of a prefix: an address to create, added to the draft and not sent. */
class DraftAddress extends ExampleElement {
  protected render(): void {
    if (this.root.querySelector("form")) return;
    const form = el("form");
    const label = el("label", t("action.address"));
    const input = el("input");
    input.name = "address";
    input.required = true;
    input.autocomplete = "off";
    label.append(input);
    const submit = el("button", t("action.submit"));
    submit.type = "submit";
    form.append(label, submit);
    const status = el("p");
    status.setAttribute("role", "status");
    const close = el("button", t("action.close"));
    close.type = "button";
    close.addEventListener("click", () =>
      this.dispatchEvent(new Event("kartenn-close", { bubbles: true, composed: true })),
    );
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      void this.add(input.value.trim(), status);
    });
    this.draw(form, status, close);
  }

  private async add(address: string, status: HTMLElement): Promise<void> {
    const id = this.object?.id;
    if (!id || !address) return;
    try {
      const { data } = await api.get("/v1/prefixes/{id}", { path: { id } });
      draft.add({
        op: "create_address",
        address,
        domain: { id: data.domain.id },
        usage: "reserved",
      });
      status.className = "";
      status.textContent = t("action.added", { n: draft.operations().length });
    } catch (error) {
      status.className = "error";
      status.textContent = t("action.failed", { detail: detailOf(error) });
    }
  }
}

defined.handle = definePlugin({
  name: "example",
  elements: {
    "example-overview": Overview,
    "example-details": Details,
    "example-summary": Summary,
    "example-usage": Usage,
    "example-draft-address": DraftAddress,
  },
});
