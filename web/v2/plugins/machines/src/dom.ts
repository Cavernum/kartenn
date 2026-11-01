// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The markup the elements of the plugin draw in their shadow roots: that of the components
// of @kartenn/ui (`k-stack`, `k-skeleton`, `k-inline-alert`, `k-empty-state`, `k-button`,
// `k-form-field`, `k-field-frame`, `k-checkbox`), whose sheet is linked first, and the
// plugin's own (`k-machines-…`, `@layer plugins`) for its tables and pairs. Without a
// framework: plain DOM, written once and relabelled in place.

import { ApiProblem, context, type PluginHandle } from "@kartenn/sdk";

/** What `definePlugin` gives back, once the elements are defined (at the end of index.ts). */
export const defined: { handle?: PluginHandle } = {};

/** A text of the plugin, under its namespace; the key itself before the definition. */
export function t(key: string, params?: Record<string, unknown>): string {
  return defined.handle ? defined.handle.t(key, params) : key;
}

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  text?: string,
  className?: string,
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  if (text !== undefined) element.textContent = text;
  if (className) element.className = className;
  return element;
}

/**
 * The texts of a part drawn once (a form, a heading): each node with its key, set again in
 * another language, in place — what is typed in the form stays.
 */
export class Texts {
  private readonly nodes: [Text, string, Record<string, unknown> | undefined][] = [];

  /** A text node of `key`, in the language of the context, relabelled by `relabel`. */
  text(key: string, params?: Record<string, unknown>): Text {
    const node = document.createTextNode(t(key, params));
    this.nodes.push([node, key, params]);
    return node;
  }

  relabel(): void {
    for (const [node, key, params] of this.nodes) node.textContent = t(key, params);
  }
}

/** A column of `Stack` (@kartenn/ui): its children one under the other, `gap` apart. */
export function stack(gap: "1" | "2" | "3" | "4" | "6", ...children: Node[]): HTMLDivElement {
  const div = el("div", undefined, "k-stack");
  div.dataset["kGap"] = gap;
  div.append(...children);
  return div;
}

/** How long a live region waits, empty, before it says what it holds. */
const SAID_AFTER_MS = 150;

/** The skeleton of `Skeleton` (@kartenn/ui, variant `text`): `rows` lines, said loading. */
export function skeleton(rows = 3): HTMLDivElement {
  const div = el("div", undefined, "k-skeleton");
  div.dataset["variant"] = "text";
  // A live region inserted with its text is often not read: empty first, then filled.
  const said = el("span", "", "k-visually-hidden");
  said.setAttribute("role", "status");
  setTimeout(() => (said.textContent = t("loading")), SAID_AFTER_MS);
  div.append(said);
  for (let row = 1; row <= rows; row++) {
    const bar = el("span", undefined, "k-skeleton__bar k-skeleton__line k-skeleton-fill");
    bar.dataset["width"] = row === rows ? "2" : "0";
    bar.setAttribute("aria-hidden", "true");
    div.append(bar);
  }
  return div;
}

/**
 * A failed read or write in the markup of `InlineAlert` (@kartenn/ui, tone `ko`): what
 * failed, the detail, then on lines of their own the code of the problem (REQ-021-20) and
 * its `Request-Id`, each after its word.
 */
export function failure(title: string, error: unknown): HTMLDivElement {
  const alert = el("div", undefined, "k-inline-alert");
  alert.dataset["tone"] = "ko";
  alert.setAttribute("role", "alert");
  const body = el("div", undefined, "k-inline-alert__body");
  const heading = el("p", undefined, "k-inline-alert__title");
  heading.append(el("span", t("error"), "k-visually-hidden"), title);
  const items = el("ul", undefined, "k-inline-alert__items");
  const line = (...nodes: (Node | string)[]) => {
    const item = el("li", undefined, "k-inline-alert__item");
    item.append(...nodes);
    items.append(item);
  };
  if (error instanceof ApiProblem) {
    line(error.detail ?? error.title);
    // Each rule broken: its sentence, then its code (`machines.name.duplicate`…).
    for (const violation of error.violations) {
      line(violation.detail, " ", el("code", violation.rule));
    }
    // A space between the word and its value: the name read is « Code forbidden ».
    line(t("code"), " ", el("code", error.code));
    if (error.requestId) line(t("request"), " ", el("code", error.requestId));
  } else {
    line(t("network"));
  }
  body.append(heading, items);
  alert.append(body);
  return alert;
}

/** What a failure says, to tell one from another in a signature (a code, a request). */
export function problemOf(error: unknown): string | null {
  if (error === null || error === undefined) return null;
  return error instanceof ApiProblem ? `${error.code} ${error.requestId ?? ""}` : String(error);
}

/** The markup of `EmptyState` (@kartenn/ui): a sentence, and what to do. */
export function empty(title: string, text: string): HTMLDivElement {
  const div = el("div", undefined, "k-empty-state");
  div.append(el("p", title, "k-empty-state__title"), el("p", text, "k-empty-state__text"));
  return div;
}

/** A button in the markup of `Button` (@kartenn/ui). */
export function button(
  label: Node,
  variant: "primary" | "secondary",
  type: "button" | "submit" = "button",
): HTMLButtonElement {
  const element = el("button", undefined, "k-button");
  element.type = type;
  element.dataset["variant"] = variant;
  element.dataset["size"] = "md";
  const span = el("span", undefined, "k-button__label");
  span.append(label);
  element.append(span);
  return element;
}

/** Counts the fields drawn: their identifiers are unique in a shadow root. */
let fields = 0;

/** A labelled field in the markup of `FormField`: the control, its label, its hint. */
export interface Field<C extends HTMLInputElement | HTMLSelectElement> {
  field: HTMLDivElement;
  control: C;
  /** Says a refusal of the field (`aria-invalid`, described by the message), or clears it. */
  refuse(message: string | null): void;
}

function formField<C extends HTMLInputElement | HTMLSelectElement>(
  label: Node,
  control: C,
  options: { required?: boolean; hint?: Node; className?: string; texts: Texts },
): Field<C> {
  const id = `k-machines-field-${++fields}`;
  const field = el("div", undefined, `k-form-field ${options.className ?? ""}`.trim());
  const caption = el("label", undefined, "k-form-field__label");
  caption.htmlFor = id;
  caption.append(label);
  if (options.required) {
    const required = el("span", undefined, "k-form-field__required");
    const star = el("span", "*");
    star.setAttribute("aria-hidden", "true");
    const spoken = el("span", undefined, "k-visually-hidden");
    spoken.append("(", options.texts.text("required"), ")");
    required.append(" ", star, spoken);
    caption.append(required);
    control.required = true;
  }
  const frame = el("div", undefined, "k-field-frame");
  control.id = id;
  control.classList.add("k-field-frame__input");
  frame.append(control);
  // The error is said by a polite region present, empty, before it.
  const live = el("div", undefined, "k-form-field__live");
  live.setAttribute("aria-live", "polite");
  field.append(caption, frame, live);
  const described: string[] = [];
  if (options.hint) {
    const hint = el("p", undefined, "k-form-field__hint");
    hint.id = `${id}-hint`;
    hint.append(options.hint);
    field.append(hint);
    described.push(hint.id);
  }
  const describe = (extra: string[]) => {
    const ids = [...extra, ...described];
    if (ids.length) control.setAttribute("aria-describedby", ids.join(" "));
    else control.removeAttribute("aria-describedby");
  };
  describe([]);
  return {
    field,
    control,
    refuse(message) {
      live.replaceChildren();
      if (message === null) {
        control.removeAttribute("aria-invalid");
        describe([]);
        return;
      }
      const error = el("p", message, "k-form-field__error");
      error.id = `${id}-error`;
      live.append(error);
      control.setAttribute("aria-invalid", "true");
      describe([error.id]);
    },
  };
}

/** A text field (`TextField` in its `FormField`). */
export function textField(
  texts: Texts,
  key: string,
  name: string,
  options: { required?: boolean; mono?: boolean; hint?: string; numeric?: boolean } = {},
): Field<HTMLInputElement> {
  const input = el("input");
  input.name = name;
  input.type = "text";
  // Digits only, without the wheel and arrows of `type=number` changing the value.
  if (options.numeric) {
    input.inputMode = "numeric";
    input.pattern = "[0-9]*";
  }
  input.autocomplete = "off";
  input.spellcheck = false;
  if (options.mono) input.classList.add("k-mono");
  return formField(texts.text(key), input, {
    texts,
    className: "k-text-field",
    ...(options.required ? { required: true } : {}),
    ...(options.hint ? { hint: texts.text(options.hint) } : {}),
  });
}

/**
 * A closed list: a native `<select>` in the frame of a field. The `Select` of @kartenn/ui
 * is a component of Vue (Reka), which a plugin without framework cannot mount.
 */
export function selectField(
  texts: Texts,
  key: string,
  name: string,
  options: { required?: boolean } = {},
): Field<HTMLSelectElement> {
  const select = el("select");
  select.name = name;
  return formField(texts.text(key), select, {
    texts,
    ...(options.required ? { required: true } : {}),
  });
}

/** An option of a closed list. */
export function option(value: string, label: string | Node): HTMLOptionElement {
  const element = el("option");
  element.value = value;
  element.append(label);
  return element;
}

/**
 * The options of a closed list, set again only when they change: the choice kept if it is
 * still offered, else the first. Whether they changed.
 */
export function fill(select: HTMLSelectElement, options: [string, string][]): boolean {
  const values = options.map(([value]) => value).join();
  if (select.dataset["filled"] === values) return false;
  const chosen = select.value;
  select.replaceChildren(...options.map(([value, label]) => option(value, label)));
  select.dataset["filled"] = values;
  select.value = options.some(([value]) => value === chosen) ? chosen : (options[0]?.[0] ?? "");
  return true;
}

/** A checkbox in the markup of `Checkbox` (@kartenn/ui), its label around it. */
export function checkbox(texts: Texts, key: string, name: string): HTMLInputElement {
  const div = el("div", undefined, "k-checkbox");
  div.dataset["size"] = "md";
  const hit = el("label", undefined, "k-checkbox__hit");
  const box = el("span", undefined, "k-checkbox__box");
  const input = el("input", undefined, "k-checkbox__input");
  input.type = "checkbox";
  input.name = name;
  box.append(input);
  const label = el("span", undefined, "k-checkbox__label");
  label.append(texts.text(key));
  hit.append(box, label);
  div.append(hit);
  // The caller places the whole: the input's wrapper.
  (input as HTMLInputElement & { wrapper?: HTMLElement }).wrapper = div;
  return input;
}

/** The wrapper of a checkbox made by `checkbox`. */
export function wrapperOf(input: HTMLInputElement): HTMLElement {
  return (input as HTMLInputElement & { wrapper?: HTMLElement }).wrapper ?? input;
}

/** The dates of the context in a URL of the shell, so a link opened apart keeps them (W9). */
export function hrefOf(path: string, query: Record<string, string> = {}): string {
  const { validAt, asOf } = context.get().time;
  const search = new URLSearchParams(query);
  if (validAt) search.set("valid_at", validAt);
  if (asOf) search.set("as_of", asOf);
  const text = search.toString();
  return text ? `${path}?${text}` : path;
}

/**
 * Whether the page about to be drawn was reached by a link of the plugin: its element then
 * takes the focus on its heading, the link clicked being gone with the former page.
 */
export const arrival = { byLink: false };

/** A link to a page of the shell: the shell follows it without reloading the page. */
export function link(text: string, path: string, mono = false): HTMLAnchorElement {
  const anchor = el("a", text, mono ? "k-machines-link k-mono" : "k-machines-link");
  anchor.href = hrefOf(path);
  anchor.addEventListener("click", () => (arrival.byLink = path.startsWith("/x/machines/")));
  return anchor;
}

/**
 * Goes to `href` as a link of the plugin would: a click on a link in the shadow root of the
 * element, which the shell follows with its router (design 021 « API des plugins », the SDK
 * has no navigation of its own). Without the shell, the browser follows it.
 */
export function followLink(place: Node, href: string): void {
  const anchor = el("a");
  anchor.href = href;
  anchor.hidden = true;
  place.appendChild(anchor);
  anchor.click();
  anchor.remove();
}

/** The pages of the plugin, under `/x/machines/<view>`. */
export const PAGES = {
  machine: (id: string) => `/x/machines/machines/${encodeURIComponent(id)}`,
  cluster: (id: string) => `/x/machines/clusters/${encodeURIComponent(id)}`,
  interface: (id: string) => `/x/machines/interfaces/${encodeURIComponent(id)}`,
};

/** The identifier after `/x/machines/<view>/` in the address of the page, if any. */
export function pageId(view: string): string | null {
  const prefix = `/x/machines/${view}/`;
  const path = location.pathname;
  if (!path.startsWith(prefix)) return null;
  const rest = path.slice(prefix.length).split("/")[0] ?? "";
  return rest ? decodeURIComponent(rest) : null;
}

/** A table: its name, its header row; the plugin's sheet lays it out (`k-machines-table`). */
export function table(
  name: string,
  headers: (string | Node)[],
): { scroll: HTMLDivElement; body: HTMLTableSectionElement } {
  // Long content scrolls in its place, never the page.
  const scroll = el("div", undefined, "k-machines-scroll");
  const element = el("table", undefined, "k-machines-table");
  element.append(el("caption", name, "k-visually-hidden"));
  const head = el("thead");
  const row = el("tr");
  for (const header of headers) {
    const cell = el("th");
    cell.scope = "col";
    cell.append(header);
    row.append(cell);
  }
  head.append(row);
  const body = el("tbody", undefined, "k-machines-rows");
  element.append(head, body);
  scroll.append(element);
  return { scroll, body };
}

/** A row of a table: its first cell a header of the row (what the row is). */
export function row(
  body: HTMLTableSectionElement,
  first: Node,
  ...values: (string | Node)[]
): void {
  const line = el("tr");
  const head = el("th");
  head.scope = "row";
  head.append(first);
  line.append(head);
  for (const value of values) {
    const cell = el("td");
    cell.append(value);
    line.append(cell);
  }
  body.append(line);
}

/** A value in fixed width (MAC, addresses, VID), or the dash of none, said in words. */
export function mono(value: string | null | undefined): Node {
  if (!value) return none();
  return el("span", value, "k-mono");
}

/** The dash of a cell without value, said « none » to a screen reader. */
export function none(): Node {
  const span = el("span");
  const dash = el("span", "—");
  dash.setAttribute("aria-hidden", "true");
  span.append(dash, el("span", t("none"), "k-visually-hidden"));
  return span;
}

/** A list of terms and their values (`k-machines-pairs`). */
export function pairs(entries: [string, Node | string][]): HTMLDListElement {
  const list = el("dl", undefined, "k-machines-pairs");
  for (const [term, value] of entries) {
    const dd = el("dd");
    dd.append(value);
    list.append(el("dt", term, "k-machines-muted"), dd);
  }
  return list;
}

/** A section titled by its heading (`h2`), its content under it. */
export function section(heading: Node, ...content: Node[]): HTMLElement {
  const title = el("h2", undefined, "k-machines-heading");
  title.id = `k-machines-section-${++fields}`;
  title.append(heading);
  const element = el("section", undefined, "k-machines-section");
  element.setAttribute("aria-labelledby", title.id);
  element.append(stack("3", title, ...content));
  return element;
}

/** Where a state is drawn again only when what it says changes (its signature). */
export class Area {
  readonly node: HTMLDivElement = el("div", undefined, "k-machines-area");
  private shown: string | null = null;

  show(signature: unknown, build: () => Node | Node[]): void {
    const text = JSON.stringify([context.get().locale, signature]);
    if (text === this.shown) return;
    this.shown = text;
    // The control that had the focus here, found again in what is drawn (« Afficher la
    // suite »); gone (the last page read), the last row of the table takes it.
    const root = this.node.getRootNode() as Document | ShadowRoot;
    const active = root.activeElement;
    const kept =
      active instanceof HTMLElement && this.node.contains(active) ? active.dataset["kKeep"] : null;
    const built = build();
    this.node.replaceChildren(...(Array.isArray(built) ? built : [built]));
    if (kept === undefined || kept === null) return;
    const again =
      this.node.querySelector<HTMLElement>(`[data-k-keep="${kept}"]`) ??
      this.node.querySelector<HTMLElement>("tbody tr:last-child a");
    again?.focus();
  }
}
