// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Plugin of the VLAN module (spec 008, REQ-008-24; design « Interface »): Custom Elements
// without a framework, written with the public SDK only, like the example plugin. The
// views of the groups (with their occupancy and VLANs) and of the VLANs, the VLAN tab of a
// network, the VLAN column of the prefix lists, and the action that creates a VLAN for a
// network (its VID proposed as the next free one of the chosen group). Writes go through
// the routes of the module and the changesets of the core, under the rights of the user;
// they are offered only at the present (W9).

import {
  ApiProblem,
  api,
  context,
  definePlugin,
  type components,
  type ObjectRef,
  type PluginHandle,
} from "@kartenn/sdk";

type Group = components["schemas"]["VlanGroup"];
type Vlan = components["schemas"]["Vlan"];

/** What `definePlugin` gives back, once the elements are defined (at the end). */
const defined: { handle?: PluginHandle } = {};

/** The property of the core that carries the VLAN of a network (spec 026). */
const PROPERTY = "core.vlan";

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

/** Whether the shell looks at the present: writes are offered only then (W9). */
function present(): boolean {
  return context.get().time.present;
}

/** A labelled field: `<label>text <input|select></label>`. */
function field<E extends HTMLInputElement | HTMLSelectElement>(
  text: string,
  control: E,
): {
  label: HTMLLabelElement;
  control: E;
} {
  const label = el("label", text);
  label.append(control);
  return { label, control };
}

function input(name: string, type = "text"): HTMLInputElement {
  const element = el("input");
  element.name = name;
  element.type = type;
  element.autocomplete = "off";
  return element;
}

function select(name: string): HTMLSelectElement {
  const element = el("select");
  element.name = name;
  return element;
}

function option(value: string, text: string): HTMLOptionElement {
  const element = el("option", text);
  element.value = value;
  return element;
}

function statusLine(): HTMLParagraphElement {
  const line = el("p");
  line.setAttribute("role", "status");
  return line;
}

function say(line: HTMLElement, text: string, error = false): void {
  line.className = error ? "error" : "";
  line.textContent = text;
}

// ------------------------------------------------------------------------- reads

/** Every item of a list of the API, page after page. */
async function all<T>(pages: AsyncIterable<{ items: T[] }>): Promise<T[]> {
  const items: T[] = [];
  for await (const page of pages) items.push(...page.items);
  return items;
}

function groups(): Promise<Group[]> {
  return all(api.pages("/v1/vlan-groups"));
}

function vlansOf(group?: string): Promise<Vlan[]> {
  return all(api.pages("/v1/vlans", group ? { query: { group } } : {}));
}

/** The VLANs already read, by identifier: a column reads the same VLAN on many rows. */
const vlanCache = new Map<string, Promise<Vlan>>();

function vlan(id: string): Promise<Vlan> {
  let found = vlanCache.get(id);
  if (!found) {
    found = api.get("/v1/vlans/{id}", { path: { id } }).then(({ data }) => data);
    found.catch(() => vlanCache.delete(id));
    vlanCache.set(id, found);
  }
  return found;
}

/** The identifier a reference value carries: its text, or the `id` of an object. */
function idOfValue(value: unknown): string | null {
  if (typeof value === "string") return value;
  if (value && typeof value === "object" && "id" in value) {
    const id = (value as { id: unknown }).id;
    return typeof id === "string" ? id : null;
  }
  return null;
}

/**
 * At most this many reads of the column at once: a list shows dozens of rows, and one read
 * per row all at once overflows the server's shedding limit (REQ-006-30) and the reader's
 * quota, refusing the other requests of the page with them.
 */
export const COLUMN_READS = 4;
let reading = 0;
const queued: (() => void)[] = [];

/** Runs `read` within the limit; `null`, without reading, if its row is gone by its turn. */
export async function limited<T>(live: () => boolean, read: () => Promise<T>): Promise<T | null> {
  while (reading >= COLUMN_READS) await new Promise<void>((resolve) => queued.push(resolve));
  reading++;
  try {
    return live() ? await read() : null;
  } finally {
    reading--;
    queued.shift()?.();
  }
}

/** The VLAN of a network at the dates of the shell, if it has one. */
async function vlanOfNetwork(prefix: string): Promise<Vlan | null> {
  const { data } = await api.get("/v1/prefixes/{id}/properties", { path: { id: prefix } });
  const id = idOfValue((data.properties as Record<string, unknown>)[PROPERTY]);
  return id ? vlan(id) : null;
}

let declaration: Promise<string> | null = null;

/** The identifier of the declaration of `core.vlan`, read once. */
function propertyId(): Promise<string> {
  declaration ??= (async () => {
    for (const item of await all(api.pages("/v1/property-declarations"))) {
      if (item.key === PROPERTY) return item.id;
    }
    throw new Error(PROPERTY);
  })();
  declaration.catch(() => (declaration = null));
  return declaration;
}

/** `VLAN 100 · servers`. */
function named(vlan: Vlan): string {
  return t("field.named", { vid: vlan.vid, name: vlan.name });
}

// ------------------------------------------------------------------------ writes

/** Sets the VLAN of a network, or removes it with `vlan` null, in one changeset. */
async function setNetworkVlan(prefix: string, vlan: string | null, motive: string): Promise<void> {
  const property = { id: await propertyId() };
  const object = { id: prefix };
  const operation = vlan
    ? {
        op: "assign_property" as const,
        kind: "prefix" as const,
        object,
        property,
        values: [vlan],
      }
    : { op: "unassign_property" as const, kind: "prefix" as const, object, property };
  await api.post("/v1/changesets", { body: { motive, operations: [operation] } });
}

// ---------------------------------------------------------------------- elements

/**
 * The common part of the elements: an open shadow root, the style sheet of the plugin
 * added once, and a redraw whenever the context of the shell changes (language, dates).
 */
abstract class VlanElement extends HTMLElement {
  /** The object of the record (tab, action) or of the row (column). */
  object: ObjectRef | null = null;
  protected readonly root: ShadowRoot = this.attachShadow({ mode: "open" });
  private styled = false;
  private unsubscribe: (() => void) | null = null;
  /** Bumped at each render: an answer of an older render is dropped. */
  protected generation = 0;

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
    // A row scrolled out of the list: its pending reads are dropped.
    this.generation++;
  }

  /** Replaces the content of the shadow root, keeping the style sheet. */
  protected draw(...nodes: Node[]): void {
    const sheets = [...this.root.querySelectorAll("link[rel=stylesheet]")];
    this.root.replaceChildren(...sheets, ...nodes);
  }

  protected abstract render(): void;
}

/** A table with its caption and header row. */
function table(
  caption: string,
  headers: string[],
): {
  table: HTMLTableElement;
  body: HTMLTableSectionElement;
} {
  const element = el("table");
  element.append(el("caption", caption));
  const head = el("thead");
  const row = el("tr");
  for (const header of headers) {
    const cell = el("th", header);
    cell.scope = "col";
    row.append(cell);
  }
  head.append(row);
  const body = el("tbody");
  element.append(head, body);
  return { table: element, body };
}

function cells(row: HTMLTableRowElement, ...values: (string | Node)[]): void {
  for (const value of values) {
    const cell = el("td");
    cell.append(value);
    row.append(cell);
  }
}

/** The VLANs of a group as a table. */
function vlanTable(caption: string, vlans: Vlan[], groupSlug?: (id: string) => string) {
  const headers = [t("field.vid"), t("field.name"), t("field.status")];
  if (groupSlug) headers.push(t("field.group"));
  const { table: element, body } = table(caption, headers);
  for (const item of [...vlans].sort((a, b) => a.vid - b.vid)) {
    const row = el("tr");
    cells(row, String(item.vid), item.name, t(`status.${item.status}`));
    if (groupSlug) cells(row, groupSlug(item.group.id));
    body.append(row);
  }
  return element;
}

/** The view of the groups: list, occupancy and VLANs of a group, creation of a group. */
class GroupsView extends VlanElement {
  protected render(): void {
    const generation = ++this.generation;
    const card = el("section", undefined, "card");
    card.append(el("h1", t("groups.title")));
    const status = statusLine();
    say(status, t("loading"));
    card.append(status);
    this.draw(card);
    Promise.all([groups(), present() ? all(api.pages("/v1/entities")) : Promise.resolve([])]).then(
      ([found, entities]) => {
        if (generation !== this.generation) return;
        say(status, found.length ? "" : t("groups.none"));
        const detail = el("section");
        if (found.length) {
          const { table: list, body } = table(t("groups.caption"), [
            t("groups.slug"),
            t("groups.range"),
            t("groups.details"),
          ]);
          for (const group of found) {
            const row = el("tr");
            const show = el("button", t("groups.show", { slug: group.slug }));
            show.type = "button";
            show.addEventListener("click", () => void this.details(group, detail));
            cells(
              row,
              group.slug,
              t("groups.vids", { min: group.vid_min, max: group.vid_max }),
              show,
            );
            body.append(row);
          }
          card.append(list);
        }
        card.append(detail);
        if (present()) card.append(this.creation(entities));
      },
      (error: unknown) => {
        if (generation === this.generation)
          say(status, t("failed", { detail: detailOf(error) }), true);
      },
    );
  }

  /** The occupancy and the VLANs of a group. */
  private async details(group: Group, into: HTMLElement): Promise<void> {
    const status = statusLine();
    into.replaceChildren(el("h2", t("groups.of", { slug: group.slug })), status);
    say(status, t("loading"));
    try {
      const [{ data: occupancy }, vlans] = await Promise.all([
        api.get("/v1/vlan-groups/{id}/occupancy", { path: { id: group.id } }),
        vlansOf(group.id),
      ]);
      say(
        status,
        t("groups.occupancy", {
          used: occupancy.used,
          reserved: occupancy.reserved,
          free: occupancy.free,
        }),
      );
      into.append(vlanTable(t("groups.vlans", { slug: group.slug }), vlans));
    } catch (error) {
      say(status, t("failed", { detail: detailOf(error) }), true);
    }
  }

  /** The form that creates a group anchored on an entity. */
  private creation(entities: { id: string; slug: string }[]): HTMLElement {
    const section = el("section");
    section.append(el("h2", t("create.group")));
    const form = el("form");
    const slug = field(t("groups.slug"), input("slug"));
    slug.control.required = true;
    const entity = field(t("create.entity"), select("entity"));
    for (const item of entities) entity.control.append(option(item.id, item.slug));
    const min = field(t("create.min"), input("vid_min", "number"));
    const max = field(t("create.max"), input("vid_max", "number"));
    min.control.value = "1";
    max.control.value = "4094";
    for (const vid of [min.control, max.control]) {
      vid.min = "1";
      vid.max = "4094";
      vid.required = true;
    }
    const motive = field(t("motive"), input("motive"));
    motive.control.required = true;
    const submit = el("button", t("create.submit"));
    submit.type = "submit";
    form.append(slug.label, entity.label, min.label, max.label, motive.label, submit);
    const status = statusLine();
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      void (async () => {
        try {
          await api.post("/v1/vlan-groups", {
            body: {
              motive: motive.control.value.trim(),
              slug: slug.control.value.trim(),
              anchor: { entity: { id: entity.control.value } },
              vid_min: Number(min.control.value),
              vid_max: Number(max.control.value),
            },
          });
          this.render();
        } catch (error) {
          say(status, t("failed", { detail: detailOf(error) }), true);
        }
      })();
    });
    section.append(form, status);
    return section;
  }
}

/** The view of every VLAN the user may read, with their group. */
class VlansView extends VlanElement {
  protected render(): void {
    const generation = ++this.generation;
    const card = el("section", undefined, "card");
    card.append(el("h1", t("vlans.title")));
    const status = statusLine();
    say(status, t("loading"));
    card.append(status);
    this.draw(card);
    Promise.all([groups(), vlansOf()]).then(
      ([found, vlans]) => {
        if (generation !== this.generation) return;
        const slugs = new Map(found.map((g) => [g.id, g.slug]));
        say(status, vlans.length ? "" : t("vlans.none"));
        if (vlans.length) {
          card.append(vlanTable(t("vlans.caption"), vlans, (id) => slugs.get(id) ?? id));
        }
      },
      (error: unknown) => {
        if (generation === this.generation)
          say(status, t("failed", { detail: detailOf(error) }), true);
      },
    );
  }
}

/**
 * The fields that choose a VLAN of a group: the group, then its VLANs. With `proposeVid`,
 * a VID field proposed as the next free one of the group instead of the VLAN list.
 */
async function groupFields(proposeVid: boolean): Promise<{
  nodes: HTMLElement[];
  group: HTMLSelectElement;
  vlan: HTMLSelectElement | null;
  vid: HTMLInputElement | null;
}> {
  const found = await groups();
  const group = field(t("field.group"), select("group"));
  for (const item of found) group.control.append(option(item.id, item.slug));
  const nodes: HTMLElement[] = [group.label];
  if (proposeVid) {
    const vid = field(t("field.vid"), input("vid", "number"));
    vid.control.min = "1";
    vid.control.max = "4094";
    vid.control.required = true;
    const propose = async () => {
      if (!group.control.value) return;
      try {
        const { data } = await api.get("/v1/vlan-groups/{id}/next-free", {
          path: { id: group.control.value },
        });
        vid.control.value = String(data.vid);
      } catch {
        vid.control.value = "";
      }
    };
    group.control.addEventListener("change", () => void propose());
    await propose();
    nodes.push(vid.label);
    return { nodes, group: group.control, vlan: null, vid: vid.control };
  }
  const vlanField = field(t("field.vlan"), select("vlan"));
  const fill = async () => {
    const vlans = group.control.value ? await vlansOf(group.control.value) : [];
    vlanField.control.replaceChildren(
      ...vlans.sort((a, b) => a.vid - b.vid).map((v) => option(v.id, named(v))),
    );
  };
  group.control.addEventListener("change", () => void fill().catch(() => undefined));
  await fill();
  nodes.push(vlanField.label);
  return { nodes, group: group.control, vlan: vlanField.control, vid: null };
}

/** The VLAN tab of a network: its VLAN, and, at the present, set it or remove it. */
class NetworkTab extends VlanElement {
  protected render(): void {
    const prefix = this.object?.id;
    if (!prefix) return;
    const generation = ++this.generation;
    const card = el("section", undefined, "card");
    card.append(el("h2", t("tab.title")));
    const current = statusLine();
    say(current, t("loading"));
    card.append(current);
    this.draw(card);
    vlanOfNetwork(prefix).then(
      async (found) => {
        if (generation !== this.generation) return;
        say(current, found ? t("tab.current", { vlan: named(found) }) : t("tab.none"));
        if (!present()) return;
        const form = el("form");
        const chosen = await groupFields(false);
        if (generation !== this.generation) return;
        const motive = field(t("motive"), input("motive"));
        motive.control.required = true;
        const set = el("button", t("tab.set"));
        set.type = "submit";
        form.append(...chosen.nodes, motive.label, set);
        const status = statusLine();
        const write = async (vlanId: string | null) => {
          if (!motive.control.reportValidity()) return;
          try {
            await setNetworkVlan(prefix, vlanId, motive.control.value.trim());
            this.render();
          } catch (error) {
            say(status, t("failed", { detail: detailOf(error) }), true);
          }
        };
        form.addEventListener("submit", (event) => {
          event.preventDefault();
          if (chosen.vlan?.value) void write(chosen.vlan.value);
        });
        card.append(form);
        if (found) {
          const remove = el("button", t("tab.remove"));
          remove.type = "button";
          remove.addEventListener("click", () => void write(null));
          card.append(remove);
        }
        card.append(status);
      },
      (error: unknown) => {
        if (generation === this.generation)
          say(current, t("failed", { detail: detailOf(error) }), true);
      },
    );
  }
}

/** The VLAN column of the prefix lists. */
class VlanColumn extends VlanElement {
  row: Record<string, unknown> | null = null;

  protected render(): void {
    const prefix =
      this.object?.id ?? (typeof this.row?.["id"] === "string" ? this.row["id"] : null);
    const generation = ++this.generation;
    const cell = el("span", "—");
    this.draw(cell);
    if (!prefix) return;
    limited(
      () => generation === this.generation,
      () => vlanOfNetwork(prefix),
    ).then(
      (found) => {
        if (generation === this.generation && found) cell.textContent = named(found);
      },
      () => undefined,
    );
  }
}

/** The action of a network: creates a VLAN in a chosen group and sets it on the network. */
class CreateVlan extends VlanElement {
  protected render(): void {
    if (this.root.querySelector("form")) return;
    const prefix = this.object?.id;
    const generation = ++this.generation;
    const status = statusLine();
    const close = el("button", t("action.close"));
    close.type = "button";
    close.addEventListener("click", () =>
      this.dispatchEvent(new Event("kartenn-close", { bubbles: true, composed: true })),
    );
    this.draw(status, close);
    if (!prefix) return;
    say(status, t("loading"));
    groupFields(true).then(
      (chosen) => {
        if (generation !== this.generation) return;
        say(status, "");
        const form = el("form");
        const name = field(t("field.name"), input("name"));
        name.control.required = true;
        const motive = field(t("motive"), input("motive"));
        motive.control.required = true;
        const submit = el("button", t("action.submit"));
        submit.type = "submit";
        form.append(...chosen.nodes, name.label, motive.label, submit);
        form.addEventListener("submit", (event) => {
          event.preventDefault();
          void (async () => {
            const why = motive.control.value.trim();
            try {
              const vid = Number(chosen.vid?.value);
              await api.post("/v1/vlans", {
                body: {
                  motive: why,
                  group: { id: chosen.group.value },
                  vid,
                  name: name.control.value.trim(),
                },
              });
              // The answer is the changeset: the VLAN is read back by its group and VID.
              const { data } = await api.get("/v1/vlans", {
                query: { group: chosen.group.value, vid: String(vid) },
              });
              const created = data.items[0];
              if (!created) throw new Error(t("field.named", { vid, name: "?" }));
              await setNetworkVlan(prefix, created.id, why);
              say(status, t("action.done", { vlan: named(created) }));
            } catch (error) {
              say(status, t("failed", { detail: detailOf(error) }), true);
            }
          })();
        });
        this.draw(form, status, close);
      },
      (error: unknown) => {
        if (generation === this.generation)
          say(status, t("failed", { detail: detailOf(error) }), true);
      },
    );
  }
}

defined.handle = definePlugin({
  name: "vlan",
  elements: {
    "vlan-groups": GroupsView,
    "vlan-vlans": VlansView,
    "vlan-network": NetworkTab,
    "vlan-column": VlanColumn,
    "vlan-create": CreateVlan,
  },
});
