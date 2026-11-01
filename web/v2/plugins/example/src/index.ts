// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Example plugin on the SDK 1.0 (spec 021, task 19; REQ-021-25, 27; REQ-007-21): Custom
// Elements without a framework, written with the public SDK only. One extension of each
// point: a view, a tab and a panel on the record, a column in the lists, an action that adds
// an operation to the draft, and a tile of the home page. Each element draws in its own
// shadow root, with the sheet of @kartenn/ui then its own, both by `handle.styles()`, and
// the markup of the components of the library (`k-button`, `k-skeleton`,
// `k-inline-alert`…). It reads through the shared cache (`queryClient`, `reads`): an
// observer holds each query, and the cache reads it again when a write or an event of the
// stream invalidates it — the element draws the new state, it invalidates nothing itself.

import {
  ApiProblem,
  context,
  definePlugin,
  draft,
  InfiniteQueryObserver,
  QueryObserver,
  queryClient,
  reads,
  stats,
  type components,
  type ObjectRef,
  type PluginHandle,
} from "@kartenn/sdk";

type Prefix = components["schemas"]["Prefix"];
type Address = components["schemas"]["Address"];

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

/** A column of `Stack` (@kartenn/ui): its children one under the other, `gap` apart. */
function stack(gap: "1" | "2" | "3" | "4" | "6", ...children: Node[]): HTMLDivElement {
  const div = el("div", undefined, "k-stack");
  div.dataset["kGap"] = gap;
  div.append(...children);
  return div;
}

/** How long a live region waits, empty, before it says what it holds. */
const SAID_AFTER_MS = 150;

/** The skeleton of `Skeleton` (@kartenn/ui, variant `text`): `rows` lines, said loading. */
function skeleton(rows = 3): HTMLDivElement {
  const div = el("div", undefined, "k-skeleton");
  div.dataset["variant"] = "text";
  // A live region inserted with its text is often not read: empty first, then filled, as
  // the library's `Skeleton` does.
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
function failure(title: string, error: unknown): HTMLDivElement {
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
    line(t("code"), el("code", error.code));
    if (error.requestId) line(t("request"), el("code", error.requestId));
  } else {
    line(t("network"));
  }
  body.append(heading, items);
  alert.append(body);
  return alert;
}

/** A term and its value, in a list of definitions. */
function pair(list: HTMLDListElement, term: string, value: string, mono = false): void {
  list.append(el("dt", term, "k-example-muted"), el("dd", value, mono ? "k-mono" : undefined));
}

/**
 * The common part of the elements: an open shadow root, the sheet of @kartenn/ui then the
 * plugin's linked once, and nothing drawn before them (no shift once they apply). Once
 * connected, the element follows the context of the shell (language, dates) and its
 * queries; disconnected, it lets go of them all.
 */
abstract class ExampleElement extends HTMLElement {
  /** The object of the record (tab, panel, action) or of the row (column). */
  object: ObjectRef | null = null;
  private readonly root: ShadowRoot = this.attachShadow({ mode: "open" });
  /**
   * What the element draws, beside its sheets and never them: a `<link>` moved in the tree
   * would be read again, its rules gone meanwhile.
   */
  private readonly content: HTMLDivElement = this.root.appendChild(
    el("div", undefined, "k-example-content"),
  );
  private sheets: Promise<unknown> | null = null;
  private stops: (() => void)[] = [];
  /** Counts the connections: a later one, or a disconnection, voids a pending start. */
  private connection = 0;

  connectedCallback(): void {
    const handle = defined.handle;
    // Without its sheets the element stays readable: the tokens are inherited anyway.
    this.sheets ??= handle
      ? Promise.allSettled([
          handle.styles(this.root, "@kartenn/ui/style.css"),
          handle.styles(this.root, "styles.css"),
        ])
      : Promise.resolve();
    const connection = ++this.connection;
    void this.sheets.then(() => {
      if (connection !== this.connection) return;
      const changed = () => {
        this.retime();
        this.render();
      };
      this.stops.push(context.subscribe(changed), ...this.follow());
      changed();
    });
  }

  disconnectedCallback(): void {
    this.connection++;
    for (const stop of this.stops.splice(0)) stop();
  }

  /** The queries the element follows while connected: the functions that let them go. */
  protected follow(): (() => void)[] {
    return [];
  }

  /**
   * The context changed (or the element connects): the dates of the shell are in the keys
   * of the cache, so another date is another query, which the observers move to.
   */
  protected retime(): void {}

  /** What is drawn, as `show` was told it: the same again draws nothing. */
  private shown: string | null = null;

  /**
   * Replaces what the element shows when `shown` (what the nodes say: state, values,
   * language) changed; its style sheets stay where they are. An observer tells each state of
   * its query (reading, read): redrawn only when what it says changes, a reader's place in
   * the content and an alert already read are left alone.
   */
  protected show(shown: string, build: () => Node[]): void {
    if (shown === this.shown) return;
    this.shown = shown;
    this.content.replaceChildren(...build());
  }

  protected abstract render(): void;
}

/**
 * The view: what the plugin is, the dates of the context, and the first page of the
 * prefixes, followed in the shared cache — a prefix created elsewhere (this tab, another,
 * another user) shows up here once the stream has invalidated the list.
 */
class Overview extends ExampleElement {
  private readonly list = new InfiniteQueryObserver(queryClient, reads.list("/v1/prefixes"));

  protected override follow(): (() => void)[] {
    return [this.list.subscribe(() => this.render())];
  }

  protected override retime(): void {
    this.list.setOptions(reads.list("/v1/prefixes"));
  }

  /** When the list was read, the one text that changes at each read: set in place. */
  private readAt: HTMLParagraphElement | null = null;

  protected render(): void {
    const { time, locale } = context.get();
    const result = this.list.getCurrentResult();
    const page = result.data?.pages[0]?.data;
    const items = (page?.items ?? []) as Prefix[];
    this.show(
      JSON.stringify([
        locale,
        time.present,
        page === undefined,
        result.isError ? problemOf(result.error) : null,
        items.map((prefix) => [prefix.value, domainOf(prefix)]),
        Boolean(page?.links.next),
      ]),
      () => {
        const heading = el("h2", t("view.prefixes"), "k-example-heading");
        heading.id = "k-example-prefixes";
        const section = el("section", undefined, "k-example-section");
        section.setAttribute("aria-labelledby", heading.id);
        section.append(
          stack(
            "3",
            stack("1", heading, el("p", t("view.how"), "k-example-muted k-example-prose")),
            this.prefixes(),
          ),
        );
        return [
          stack(
            "6",
            stack(
              "2",
              el("h1", t("view.title"), "k-example-title"),
              el("p", t("view.body"), "k-example-prose"),
              el("p", t(time.present ? "view.present" : "view.dated"), "k-example-muted"),
            ),
            section,
          ),
        ];
      },
    );
    if (this.readAt && result.dataUpdatedAt > 0) {
      const read = new Date(result.dataUpdatedAt).toISOString();
      this.readAt.textContent = t("view.readAt", { date: defined.handle?.format.dateTime(read) });
    }
  }

  /** The state of the list: loading, failed, empty, or its first page. */
  private prefixes(): Node {
    this.readAt = null;
    const result = this.list.getCurrentResult();
    const page = result.data?.pages[0]?.data;
    if (!page) return result.isError ? failure(t("failed"), result.error) : skeleton();
    if (page.items.length === 0) {
      const empty = el("div", undefined, "k-empty-state");
      empty.append(
        el("p", t("view.empty"), "k-empty-state__title"),
        el("p", t("view.emptyText"), "k-empty-state__text"),
      );
      return empty;
    }
    // One grid for every row (`subgrid`): the domains line up whatever the prefixes.
    const list = el("ul", undefined, "k-example-list");
    for (const prefix of page.items as Prefix[]) {
      const item = el("li", undefined, "k-example-row");
      const domain = el("span", undefined, "k-example-muted");
      domain.append(el("span", `${t("tab.domain")} `, "k-visually-hidden"), domainOf(prefix));
      item.append(el("span", prefix.value, "k-mono"), domain);
      list.append(item);
    }
    this.readAt = el("p", undefined, "k-example-muted");
    const notes = [this.readAt];
    if (page.links.next) notes.push(el("p", t("view.more"), "k-example-muted"));
    // A read again that failed keeps the list it had, with the failure above it.
    return result.isError
      ? stack("2", failure(t("failed"), result.error), list, ...notes)
      : stack("2", list, ...notes);
  }
}

/** The domain of a prefix as shown: its slug, else its identifier. */
function domainOf(prefix: Prefix): string {
  return prefix.domain.slug ?? prefix.domain.id;
}

/** What a failure says, to tell one from another (a code, a request). */
function problemOf(error: unknown): string {
  return error instanceof ApiProblem ? `${error.code} ${error.requestId ?? ""}` : String(error);
}

/** The observer of the object of an element, from its first connection. */
function objectObserver(object: ObjectRef | null) {
  return object
    ? new QueryObserver(queryClient, reads.object<Prefix>(object.type, object.id))
    : null;
}

/** The tab of a prefix: the prefix as the shared cache holds it, at the dates of the shell. */
class Details extends ExampleElement {
  // Not `prefix`: a member of every `Element` (its namespace prefix).
  private observer: ReturnType<typeof objectObserver> = null;

  protected override follow(): (() => void)[] {
    this.observer ??= objectObserver(this.object);
    return this.observer ? [this.observer.subscribe(() => this.render())] : [];
  }

  protected override retime(): void {
    if (this.object) {
      this.observer?.setOptions(reads.object<Prefix>(this.object.type, this.object.id));
    }
  }

  protected render(): void {
    if (!this.observer) return;
    const result = this.observer.getCurrentResult();
    const prefix = result.data?.data;
    const failed = result.isError ? problemOf(result.error) : null;
    this.show(
      JSON.stringify([context.get().locale, failed, prefix && [prefix.value, domainOf(prefix)]]),
      () => {
        if (!prefix) return [failed ? failure(t("failed"), result.error) : skeleton(2)];
        const list = el("dl", undefined, "k-example-counts");
        pair(list, t("tab.prefix"), prefix.value, true);
        pair(list, t("tab.domain"), domainOf(prefix));
        return [failed ? stack("2", failure(t("failed"), result.error), list) : list];
      },
    );
  }
}

/** The panel: a short summary of the object. */
class Summary extends ExampleElement {
  protected render(): void {
    const { type = "", id = "" } = this.object ?? {};
    this.show(JSON.stringify([context.get().locale, type, id]), () => {
      const list = el("dl", undefined, "k-example-counts");
      pair(list, t("panel.type"), type);
      pair(list, t("panel.id"), id, true);
      return [stack("2", el("h2", t("panel.title"), "k-example-heading"), list)];
    });
  }
}

/** The column of the address lists: the row as the list read it. */
class Usage extends ExampleElement {
  row: Partial<Address> | null = null;

  protected render(): void {
    const key = this.row?.usage === "reserved" ? "column.reserved" : "column.other";
    this.show(JSON.stringify([context.get().locale, key]), () => [el("span", t(key))]);
  }
}

/** The action of a prefix: an address to create, added to the draft and not sent. */
class DraftAddress extends ExampleElement {
  protected render(): void {
    // Drawn once: a change of language leaves what is typed alone.
    this.show("form", () => {
      // The markup of `TextField` in its `FormField`: label, required mark, hint.
      const field = el("div", undefined, "k-form-field k-text-field");
      const label = el("label", t("action.address"), "k-form-field__label");
      label.htmlFor = "k-example-address";
      const required = el("span", undefined, "k-form-field__required");
      const star = el("span", "*");
      star.setAttribute("aria-hidden", "true");
      required.append(" ", star, el("span", t("required"), "k-visually-hidden"));
      label.append(required);
      const frame = el("div", undefined, "k-field-frame");
      const input = el("input", undefined, "k-field-frame__input k-mono");
      input.id = label.htmlFor;
      input.name = "address";
      input.required = true;
      input.autocomplete = "off";
      input.spellcheck = false;
      const hint = el("p", t("action.hint"), "k-form-field__hint");
      hint.id = "k-example-address-hint";
      input.setAttribute("aria-describedby", hint.id);
      frame.append(input);
      field.append(label, frame, hint);
      // The outcome: said by a live region there from the start; a failure as an alert, tied
      // to the field.
      const said = el("p");
      said.setAttribute("role", "status");
      const failed = el("div");
      failed.id = "k-example-address-failed";
      const submit = button(t("action.submit"), "primary");
      submit.type = "submit";
      const close = button(t("action.close"), "secondary");
      close.addEventListener("click", () =>
        this.dispatchEvent(new Event("kartenn-close", { bubbles: true, composed: true })),
      );
      const actions = el("div", undefined, "k-example-actions");
      actions.append(submit, close);
      const form = el("form", undefined, "k-stack");
      form.dataset["kGap"] = "4";
      form.append(field, said, failed, actions);
      form.addEventListener("submit", (event) => {
        event.preventDefault();
        void this.add(input, submit, said, failed);
      });
      return [form];
    });
  }

  private async add(
    input: HTMLInputElement,
    submit: HTMLButtonElement,
    said: HTMLElement,
    failed: HTMLElement,
  ): Promise<void> {
    const object = this.object;
    const address = input.value.trim();
    if (!object || !address || submit.getAttribute("aria-busy") === "true") return;
    submit.setAttribute("aria-busy", "true");
    said.textContent = "";
    failed.replaceChildren();
    input.removeAttribute("aria-invalid");
    input.setAttribute("aria-describedby", "k-example-address-hint");
    const refused = (error: unknown) => {
      failed.replaceChildren(failure(t("action.failed"), error));
      input.setAttribute("aria-invalid", "true");
      input.setAttribute("aria-describedby", `k-example-address-hint ${failed.id}`);
    };
    try {
      // The domain of the prefix, from the shared cache when it holds it.
      const { data } = await queryClient.fetchQuery(reads.object<Prefix>(object.type, object.id));
      const outcome = await draft.add({
        op: "create_address",
        address,
        domain: { id: data.domain.id },
        usage: "reserved",
      });
      if (outcome.kind === "saved") {
        said.textContent = t("action.added", { n: outcome.draft.operation_count });
      } else {
        refused(outcome.problem);
      }
    } catch (error) {
      refused(error);
    } finally {
      submit.removeAttribute("aria-busy");
    }
  }
}

/** A button in the markup of `Button` (@kartenn/ui). */
function button(label: string, variant: "primary" | "secondary"): HTMLButtonElement {
  const element = el("button", undefined, "k-button");
  element.type = "button";
  element.dataset["variant"] = variant;
  element.dataset["size"] = "md";
  element.append(el("span", label, "k-button__label"));
  return element;
}

/** The tile of the home page: counters of the referential, read again on `stats`. */
class Counts extends ExampleElement {
  private readonly counts = new QueryObserver(queryClient, stats.query());

  protected override follow(): (() => void)[] {
    return [this.counts.subscribe(() => this.render())];
  }

  protected render(): void {
    const result = this.counts.getCurrentResult();
    const objects = result.data?.data.objects;
    const count = (type: string) => defined.handle?.format.number(objects?.[type] ?? 0) ?? "";
    const failed = result.isError ? problemOf(result.error) : null;
    const counts = objects ? [count("core.prefix"), count("core.address")] : null;
    this.show(JSON.stringify([context.get().locale, failed, counts]), () => {
      if (!counts) return [failed ? failure(t("failed"), result.error) : skeleton(2)];
      const list = el("dl", undefined, "k-example-counts");
      pair(list, t("widget.prefixes"), counts[0]!, true);
      pair(list, t("widget.addresses"), counts[1]!, true);
      return [list];
    });
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
    "example-counts": Counts,
  },
});
