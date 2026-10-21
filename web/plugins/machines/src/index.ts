// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Plugin of the machines and interfaces module (spec 009, REQ-009-20; design « Interface »):
// Custom Elements without a framework, written with the public SDK only, like the VLAN
// plugin. The views of the machines (with their interfaces, addresses and 802.1Q mode), of
// the clusters and of the interfaces; the « Machines » tab of an address and of a network;
// the machine column of the address list; and the action that attaches an address to an
// interface. Writes go through the routes of the module, under the rights of the user and
// the `If-Match` of the object they change; they are offered only at the present (W9).

import {
  ApiProblem,
  api,
  context,
  definePlugin,
  type components,
  type ObjectRef,
  type PluginHandle,
} from "@kartenn/sdk";

type Machine = components["schemas"]["Machine"];
type MachineCard = components["schemas"]["MachineCard"];
type Interface = components["schemas"]["Interface"];
type Cluster = components["schemas"]["Cluster"];
type Vlan = components["schemas"]["Vlan"];

/** What `definePlugin` gives back, once the elements are defined (at the end). */
const defined: { handle?: PluginHandle } = {};

const KINDS = ["physical", "virtual", "container", "appliance"] as const;

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
): { label: HTMLLabelElement; control: E } {
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

/** A required motive field. */
function motiveField(): { label: HTMLLabelElement; control: HTMLInputElement } {
  const motive = field(t("motive"), input("motive"));
  motive.control.required = true;
  return motive;
}

function submitButton(text: string): HTMLButtonElement {
  const button = el("button", text);
  button.type = "submit";
  return button;
}

// ------------------------------------------------------------------------- reads

/** Every item of a list of the API, page after page. */
async function all<T>(pages: AsyncIterable<{ items: T[] }>): Promise<T[]> {
  const items: T[] = [];
  for await (const page of pages) items.push(...page.items);
  return items;
}

function machines(query?: Record<string, string>): Promise<Machine[]> {
  return all(api.pages("/v1/machines", query ? { query } : {}));
}

function interfaces(query?: Record<string, string>): Promise<Interface[]> {
  return all(api.pages("/v1/interfaces", query ? { query } : {}));
}

/** The machines already read, by identifier: a column reads the same machine on many rows. */
const machineCache = new Map<string, Promise<Machine>>();

function machine(id: string): Promise<Machine> {
  let found = machineCache.get(id);
  if (!found) {
    found = api.get("/v1/machines/{id}", { path: { id } }).then(({ data }) => data as Machine);
    found.catch(() => machineCache.delete(id));
    machineCache.set(id, found);
  }
  return found;
}

/** The interfaces that hold an address, each with its machine. */
async function holders(address: string): Promise<{ iface: Interface; machine: Machine }[]> {
  const found = await interfaces({ address });
  return Promise.all(
    found.map(async (iface) => ({ iface, machine: await machine(iface.machine.id) })),
  );
}

/** `eth0 (access 120)`: the name of an interface with its 802.1Q mode. */
function modeOf(iface: Interface, vlans: Map<string, Vlan>): string {
  const vid = (id?: string | null) => (id ? String(vlans.get(id)?.vid ?? "?") : "");
  switch (iface.vlan.mode) {
    case "access":
      return `${t("mode.access")} ${vid(iface.vlan.access?.id)}`;
    case "trunk":
      return `${t("mode.trunk")} ${(iface.vlan.tagged ?? []).map((v) => vid(v.id)).join(", ")}`;
    default:
      return t("mode.none");
  }
}

// ------------------------------------------------------------------------ writes

/** Changes an interface under its `If-Match`, read just before. */
async function patchInterface(id: string, body: Record<string, unknown>): Promise<void> {
  const { etag } = await api.get("/v1/interfaces/{id}", { path: { id } });
  await api.patch("/v1/interfaces/{id}", {
    path: { id },
    ifMatch: etag ?? "*",
    body: body as components["schemas"]["InterfacePatch"],
  });
}

/** Attaches an address to an interface under the `If-Match` of the interface. */
async function attach(iface: string, address: string, primary: boolean, motive: string) {
  const { etag } = await api.get("/v1/interfaces/{id}", { path: { id: iface } });
  await api.post("/v1/interfaces/{id}/attachments", {
    path: { id: iface },
    ifMatch: etag ?? "*",
    body: { motive, address: { id: address }, primary },
  });
}

// ---------------------------------------------------------------------- elements

/**
 * The common part of the elements: an open shadow root, the style sheet of the plugin
 * added once, and a redraw whenever the context of the shell changes (language, dates).
 */
abstract class MachinesElement extends HTMLElement {
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
): { table: HTMLTableElement; body: HTMLTableSectionElement } {
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

/** The view of the machines: list, interfaces and addresses of one, creations, 802.1Q. */
class MachinesView extends MachinesElement {
  protected render(): void {
    const generation = ++this.generation;
    const card = el("section", undefined, "card");
    card.append(el("h1", t("machines.title")));
    const status = statusLine();
    say(status, t("loading"));
    card.append(status);
    this.draw(card);
    Promise.all([
      machines(),
      present() ? all(api.pages("/v1/entities")) : Promise.resolve([]),
    ]).then(
      ([found, entities]) => {
        if (generation !== this.generation) return;
        say(status, found.length ? "" : t("machines.none"));
        const detail = el("section");
        if (found.length) {
          const { table: list, body } = table(t("machines.caption"), [
            t("machines.name"),
            t("machines.kind"),
            t("machines.details"),
          ]);
          for (const item of [...found].sort((a, b) => a.name.localeCompare(b.name))) {
            const row = el("tr");
            const show = el("button", t("machines.show", { name: item.name }));
            show.type = "button";
            show.addEventListener("click", () => void this.details(item.id, detail));
            cells(row, item.name, t(`kind.${item.kind}`), show);
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

  /** The interfaces of a machine with their addresses and 802.1Q mode. */
  private async details(id: string, into: HTMLElement): Promise<void> {
    const status = statusLine();
    into.replaceChildren(status);
    say(status, t("loading"));
    try {
      const [{ data }, ifaces, vlans] = await Promise.all([
        api.get("/v1/machines/{id}", { path: { id } }),
        interfaces({ machine: id }),
        all(api.pages("/v1/vlans")).catch(() => [] as Vlan[]),
      ]);
      const card = data as MachineCard;
      const byId = new Map(vlans.map((v) => [v.id, v]));
      const addresses = new Map(
        card.interfaces.map((i) => [i.id, i.addresses.map((a) => a.value).join(", ")]),
      );
      say(status, ifaces.length ? "" : t("interfaces.none"));
      into.prepend(el("h2", t("machines.of", { name: card.name })));
      if (ifaces.length) {
        const { table: list, body } = table(t("interfaces.of", { name: card.name }), [
          t("interfaces.name"),
          t("interfaces.mac"),
          t("interfaces.mode"),
          t("interfaces.addresses"),
        ]);
        for (const iface of [...ifaces].sort((a, b) => a.name.localeCompare(b.name))) {
          const row = el("tr");
          cells(
            row,
            iface.name,
            iface.mac ?? "—",
            modeOf(iface, byId),
            addresses.get(iface.id) || "—",
          );
          body.append(row);
        }
        into.append(list);
      }
      if (present()) {
        into.append(this.newInterface(id, into));
        for (const iface of ifaces) into.append(this.vlanForm(iface, vlans, id, into));
      }
    } catch (error) {
      say(status, t("failed", { detail: detailOf(error) }), true);
    }
  }

  /** The form that creates a machine owned by an entity. */
  private creation(entities: { id: string; slug: string }[]): HTMLElement {
    const section = el("section");
    section.append(el("h2", t("machines.create")));
    const form = el("form");
    const name = field(t("machines.name"), input("name"));
    name.control.required = true;
    const kind = field(t("machines.kind"), select("kind"));
    for (const value of KINDS) kind.control.append(option(value, t(`kind.${value}`)));
    const owner = field(t("machines.owner"), select("owner"));
    for (const item of entities) owner.control.append(option(item.id, item.slug));
    const motive = motiveField();
    form.append(
      name.label,
      kind.label,
      owner.label,
      motive.label,
      submitButton(t("machines.submit")),
    );
    const status = statusLine();
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      void (async () => {
        try {
          await api.post("/v1/machines", {
            body: {
              motive: motive.control.value.trim(),
              name: name.control.value.trim(),
              kind: kind.control.value,
              owner: { id: owner.control.value },
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

  /** The form that adds an interface to a machine. */
  private newInterface(machineId: string, into: HTMLElement): HTMLElement {
    const section = el("section");
    section.append(el("h3", t("interfaces.create")));
    const form = el("form");
    const name = field(t("interfaces.name"), input("interface"));
    name.control.required = true;
    const mac = field(t("interfaces.mac"), input("mac"));
    const motive = motiveField();
    form.append(name.label, mac.label, motive.label, submitButton(t("interfaces.submit")));
    const status = statusLine();
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      void (async () => {
        try {
          const value = mac.control.value.trim();
          await api.post("/v1/interfaces", {
            body: {
              motive: motive.control.value.trim(),
              machine: { id: machineId },
              name: name.control.value.trim(),
              ...(value ? { mac: value } : {}),
            },
          });
          await this.details(machineId, into);
        } catch (error) {
          say(status, t("failed", { detail: detailOf(error) }), true);
        }
      })();
    });
    section.append(form, status);
    return section;
  }

  /** The form that sets the 802.1Q mode of an interface: none, or access on a VLAN. */
  private vlanForm(iface: Interface, vlans: Vlan[], machineId: string, into: HTMLElement) {
    const section = el("section");
    section.append(el("h3", t("interfaces.vlan", { name: iface.name })));
    const form = el("form");
    const mode = field(t("interfaces.mode"), select("mode"));
    for (const value of ["none", "access"]) mode.control.append(option(value, t(`mode.${value}`)));
    mode.control.value = iface.vlan.mode === "access" ? "access" : "none";
    const access = field(t("interfaces.access"), select("access"));
    for (const item of [...vlans].sort((a, b) => a.vid - b.vid)) {
      access.control.append(option(item.id, `${item.vid} · ${item.name}`));
    }
    if (iface.vlan.access?.id) access.control.value = iface.vlan.access.id;
    const motive = motiveField();
    form.append(mode.label, access.label, motive.label, submitButton(t("interfaces.set")));
    const status = statusLine();
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      void (async () => {
        try {
          const vlan =
            mode.control.value === "access"
              ? { mode: "access", access: { id: access.control.value } }
              : { mode: "none" };
          await patchInterface(iface.id, { motive: motive.control.value.trim(), vlan });
          await this.details(machineId, into);
        } catch (error) {
          say(status, t("failed", { detail: detailOf(error) }), true);
        }
      })();
    });
    section.append(form, status);
    return section;
  }
}

/** The view of the clusters. */
class ClustersView extends MachinesElement {
  protected render(): void {
    const generation = ++this.generation;
    const card = el("section", undefined, "card");
    card.append(el("h1", t("clusters.title")));
    const status = statusLine();
    say(status, t("loading"));
    card.append(status);
    this.draw(card);
    all<Cluster>(api.pages("/v1/clusters")).then(
      (found) => {
        if (generation !== this.generation) return;
        say(status, found.length ? "" : t("clusters.none"));
        if (!found.length) return;
        const { table: list, body } = table(t("clusters.caption"), [t("clusters.slug")]);
        for (const item of [...found].sort((a, b) => a.slug.localeCompare(b.slug))) {
          const row = el("tr");
          cells(row, item.slug);
          body.append(row);
        }
        card.append(list);
      },
      (error: unknown) => {
        if (generation === this.generation)
          say(status, t("failed", { detail: detailOf(error) }), true);
      },
    );
  }
}

/** The view of every interface the user may read, with its machine. */
class InterfacesView extends MachinesElement {
  protected render(): void {
    const generation = ++this.generation;
    const card = el("section", undefined, "card");
    card.append(el("h1", t("interfaces.title")));
    const status = statusLine();
    say(status, t("loading"));
    card.append(status);
    this.draw(card);
    Promise.all([interfaces(), machines(), all(api.pages("/v1/vlans")).catch(() => [])]).then(
      ([found, owners, vlans]) => {
        if (generation !== this.generation) return;
        say(status, found.length ? "" : t("interfaces.none"));
        if (!found.length) return;
        const names = new Map(owners.map((m) => [m.id, m.name]));
        const byId = new Map((vlans as Vlan[]).map((v) => [v.id, v]));
        const { table: list, body } = table(t("interfaces.caption"), [
          t("machines.name"),
          t("interfaces.name"),
          t("interfaces.mac"),
          t("interfaces.mode"),
        ]);
        for (const iface of found) {
          const row = el("tr");
          cells(
            row,
            names.get(iface.machine.id) ?? iface.machine.id,
            iface.name,
            iface.mac ?? "—",
            modeOf(iface, byId),
          );
          body.append(row);
        }
        card.append(list);
      },
      (error: unknown) => {
        if (generation === this.generation)
          say(status, t("failed", { detail: detailOf(error) }), true);
      },
    );
  }
}

/** The « Machines » tab: who holds an address, or the machines of a network. */
class MachinesTab extends MachinesElement {
  protected render(): void {
    const object = this.object;
    if (!object) return;
    const generation = ++this.generation;
    const card = el("section", undefined, "card");
    card.append(el("h2", t("tab.title")));
    const status = statusLine();
    say(status, t("loading"));
    card.append(status);
    this.draw(card);
    if (object.type === "prefix") {
      machines({ network: object.id }).then(
        (found) => {
          if (generation !== this.generation) return;
          say(status, found.length ? "" : t("tab.none"));
          if (!found.length) return;
          const { table: list, body } = table(t("tab.caption"), [
            t("machines.name"),
            t("machines.kind"),
          ]);
          for (const item of found) {
            const row = el("tr");
            cells(row, item.name, t(`kind.${item.kind}`));
            body.append(row);
          }
          card.append(list);
        },
        (error: unknown) => {
          if (generation === this.generation)
            say(status, t("failed", { detail: detailOf(error) }), true);
        },
      );
      return;
    }
    holders(object.id).then(
      (found) => {
        if (generation !== this.generation) return;
        say(status, found.length ? "" : t("tab.none"));
        const list = el("ul");
        for (const { iface, machine } of found) {
          list.append(el("li", t("tab.holds", { machine: machine.name, interface: iface.name })));
        }
        if (found.length) card.append(list);
      },
      (error: unknown) => {
        if (generation === this.generation)
          say(status, t("failed", { detail: detailOf(error) }), true);
      },
    );
  }
}

/** The machine column of the address list. */
class MachineColumn extends MachinesElement {
  row: Record<string, unknown> | null = null;

  protected render(): void {
    const address =
      this.object?.id ?? (typeof this.row?.["id"] === "string" ? this.row["id"] : null);
    const generation = ++this.generation;
    const cell = el("span", "—");
    this.draw(cell);
    if (!address) return;
    holders(address).then(
      (found) => {
        if (generation === this.generation && found.length) {
          cell.textContent = [...new Set(found.map((h) => h.machine.name))].join(", ");
        }
      },
      () => undefined,
    );
  }
}

/** The action of an address: attaches it to an interface of a chosen machine. */
class AttachAction extends MachinesElement {
  protected render(): void {
    if (this.root.querySelector("form")) return;
    const address = this.object?.id;
    const generation = ++this.generation;
    const status = statusLine();
    const close = el("button", t("action.close"));
    close.type = "button";
    close.addEventListener("click", () =>
      this.dispatchEvent(new Event("kartenn-close", { bubbles: true, composed: true })),
    );
    this.draw(status, close);
    if (!address) return;
    say(status, t("loading"));
    machines().then(
      async (found) => {
        if (generation !== this.generation) return;
        say(status, "");
        const form = el("form");
        const owner = field(t("action.machine"), select("machine"));
        for (const item of [...found].sort((a, b) => a.name.localeCompare(b.name))) {
          owner.control.append(option(item.id, item.name));
        }
        const iface = field(t("action.interface"), select("interface"));
        const fill = async () => {
          const list = owner.control.value
            ? await interfaces({ machine: owner.control.value })
            : [];
          iface.control.replaceChildren(...list.map((i) => option(i.id, i.name)));
        };
        owner.control.addEventListener("change", () => void fill().catch(() => undefined));
        await fill().catch(() => undefined);
        const primary = field(t("action.primary"), input("primary", "checkbox"));
        const motive = motiveField();
        form.append(
          owner.label,
          iface.label,
          primary.label,
          motive.label,
          submitButton(t("action.submit")),
        );
        form.addEventListener("submit", (event) => {
          event.preventDefault();
          void (async () => {
            try {
              await attach(
                iface.control.value,
                address,
                primary.control.checked,
                motive.control.value.trim(),
              );
              say(
                status,
                t("action.done", {
                  interface: iface.control.selectedOptions[0]?.textContent ?? "",
                  machine: owner.control.selectedOptions[0]?.textContent ?? "",
                }),
              );
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
  name: "machines",
  elements: {
    "machines-machines": MachinesView,
    "machines-clusters": ClustersView,
    "machines-interfaces": InterfacesView,
    "machines-tab": MachinesTab,
    "machines-column": MachineColumn,
    "machines-attach": AttachAction,
  },
});
