// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The three views of the plugin under the space `hosts` (spec 021, design « Plugin
// Machines »), each a list and the records of its objects: `/x/machines/machines`, `…/<id>`;
// `/x/machines/clusters`, `…/<id>`; `/x/machines/interfaces`, `…/<id>`, whose list is a
// search by MAC, address and VID (incident response). The shell creates the element again
// for each address of the page: it reads its own at its creation.

import { queryClient, reads, type Entry, type Operation } from "@kartenn/sdk";
import {
  Area,
  button,
  el,
  empty,
  failure,
  link,
  mono,
  none,
  fill,
  option,
  pageId,
  PAGES,
  pairs,
  problemOf,
  row,
  section,
  selectField,
  skeleton,
  stack,
  t,
  table,
  textField,
} from "./dom.ts";
import { MachinesElement, type Followed, type FollowedList } from "./element.ts";
import { draftForm, type DraftForm } from "./forms.ts";
import {
  KINDS,
  modeOf,
  Names,
  type Cluster,
  type Interface,
  type Machine,
  type MachineCard,
  type Name,
  type Vlan,
} from "./reads.ts";

/** A name as drawn: the name, or a short wait, or the identifier of an unreadable object. */
export function nameNode(name: Name, id: string, page?: string): Node {
  if (name.state === "read") return page ? link(name.name, page) : el("span", name.name);
  if (name.state === "failed") return mono(id);
  const span = el("span", undefined, "k-machines-muted");
  const dots = el("span", "…");
  dots.setAttribute("aria-hidden", "true");
  span.append(dots, el("span", t("loading"), "k-visually-hidden"));
  return span;
}

/** What a name says, in a signature. */
function said(name: Name): string {
  return name.state === "read" ? name.name : name.state;
}

/** The heading of a page: its `h1`, then a sentence under it. */
function heading(title: Node, intro?: Node): HTMLElement {
  const header = el("header", undefined, "k-stack");
  header.dataset["kGap"] = "1";
  const h1 = el("h1", undefined, "k-machines-title");
  // Where the focus goes when the page of the element replaces another (`MachinesElement`).
  h1.tabIndex = -1;
  h1.append(title);
  header.append(h1);
  if (intro) {
    const sentence = el("p", undefined, "k-machines-muted k-machines-prose");
    sentence.append(intro);
    header.append(sentence);
  }
  return header;
}

/**
 * The heading of a record: its `h1` drawn once (the focus may be on it) and its text set as
 * the object is read; the failure of the read under it.
 */
class Title {
  private readonly text = document.createTextNode("");
  private readonly problem = new Area();
  readonly node: HTMLElement;

  constructor(mono = false) {
    const name = el("span", undefined, mono ? "k-mono" : undefined);
    name.append(this.text);
    this.node = stack("2", heading(name), this.problem.node);
  }

  /** Its name, or the word of its kind before it is read; `error` if it could not be. */
  set(name: string, error: unknown, failed: string): void {
    if (this.text.data !== name) this.text.data = name;
    this.problem.show(problemOf(error), () => (error ? failure(failed, error) : []));
  }
}

/**
 * The state of a list in its place: a skeleton until its first page, a failure, a sentence
 * when empty, else `draw` of its items — with « Afficher la suite » while pages remain, and
 * the failure of a page read again above the items it keeps.
 */
function listState<T>(
  list: FollowedList<T>,
  texts: { failed: string; empty: string; emptyText: string },
  draw: (items: T[]) => Node,
): Node {
  const items = list.items;
  if (!items) return list.error ? failure(texts.failed, list.error) : skeleton(4);
  if (items.length === 0) return empty(texts.empty, texts.emptyText);
  const nodes: Node[] = [draw(items)];
  if (list.more) {
    const more = button(document.createTextNode(t("list.more")), "secondary");
    more.dataset["kKeep"] = "more";
    if (list.fetching) more.setAttribute("aria-busy", "true");
    more.addEventListener("click", () => list.next());
    nodes.push(el("div", undefined, "k-machines-actions"));
    nodes[nodes.length - 1]!.appendChild(more);
  }
  if (list.error) nodes.unshift(failure(texts.failed, list.error));
  return stack("2", ...nodes);
}

/** The host of a machine: a machine or a cluster, with its link. */
function hostOf(machine: Machine, names: Names): { node: Node; said: string } {
  const host = machine.host;
  if (!host) return { node: none(), said: "" };
  if ("machine" in host) {
    const name = names.of("machines.machine", host.machine.id);
    return {
      node: nameNode(name, host.machine.id, PAGES.machine(host.machine.id)),
      said: said(name),
    };
  }
  const name = names.of("machines.cluster", host.cluster.id);
  return {
    node: nameNode(name, host.cluster.id, PAGES.cluster(host.cluster.id)),
    said: said(name),
  };
}

/** An entity or a site cited by its identifier: its slug once read. */
function cited(names: Names, type: "core.entity" | "core.site", id?: string | null) {
  if (!id) return { node: none(), said: "" };
  const name = names.of(type, id);
  return { node: nameNode(name, id), said: said(name) };
}

// ----------------------------------------------------------------------------- machines

/** The view `machines`: the list of the machines, or the record of one. */
export class MachinesView extends MachinesElement {
  private readonly names = new Names(
    () => this.update(),
    () => this.isConnected,
  );
  private page: { update(): void } | null = null;

  protected build(): Node[] {
    const id = pageId("machines");
    const page = id ? new MachineRecord(this, id) : new MachineList(this);
    this.page = page;
    return page.build();
  }

  protected update(): void {
    this.page?.update();
  }

  /** What the pages of the element use of it. */
  get kit() {
    return {
      names: this.names,
      texts: this.texts,
      follow: this.follow.bind(this),
      followList: this.followList.bind(this),
    };
  }
}

type Kit = MachinesView["kit"];

class MachineList {
  private readonly area = new Area();
  private readonly list;
  private readonly entities;
  private readonly kit: Kit;
  private creation: DraftForm | null = null;
  private owner: HTMLSelectElement | null = null;

  constructor(view: MachinesView) {
    this.kit = view.kit;
    this.list = this.kit.followList(() => reads.list("/v1/machines"));
    this.entities = this.kit.followList(() => reads.list("/v1/entities"));
  }

  build(): Node[] {
    const { texts } = this.kit;
    const name = textField(texts, "machines.name", "name", { required: true });
    const kind = selectField(texts, "machines.kind", "kind", { required: true });
    for (const value of KINDS) kind.control.append(option(value, texts.text(`kind.${value}`)));
    const owner = selectField(texts, "machines.owner", "owner", { required: true });
    this.owner = owner.control;
    this.creation = draftForm(texts, {
      fields: [name, kind, owner],
      submit: "form.submit",
      permission: "machines.edit",
      operation: () => ({
        op: "machines.create_machine",
        name: name.control.value.trim(),
        kind: kind.control.value,
        owner: { id: owner.control.value },
      }),
    });
    return [
      stack(
        "6",
        heading(texts.text("machines.title"), texts.text("machines.intro")),
        this.area.node,
        section(texts.text("machines.create"), this.creation.form),
      ),
    ];
  }

  update(): void {
    const { names } = this.kit;
    this.creation?.refresh();
    this.fillOwners();
    const items = this.list.items ?? [];
    const rows = items.map((m) => ({
      machine: m,
      owner: cited(names, "core.entity", m.owner?.id),
      site: cited(names, "core.site", m.site?.id),
      host: hostOf(m, names),
    }));
    this.area.show(
      [
        this.list.items === undefined,
        problemOf(this.list.error),
        this.list.more,
        this.list.fetching,
        rows.map((r) => [
          r.machine.id,
          r.machine.name,
          r.machine.kind,
          r.owner.said,
          r.site.said,
          r.host.said,
        ]),
      ],
      () =>
        listState(
          this.list,
          {
            failed: t("machines.failed"),
            empty: t("machines.none"),
            emptyText: t("machines.noneText"),
          },
          () => {
            const { scroll, body } = table(t("machines.title"), [
              t("machines.name"),
              t("machines.kind"),
              t("machines.owner"),
              t("machines.site"),
              t("machines.host"),
            ]);
            for (const r of rows) {
              row(
                body,
                link(r.machine.name, PAGES.machine(r.machine.id)),
                t(`kind.${r.machine.kind}`),
                r.owner.node,
                r.site.node,
                r.host.node,
              );
            }
            return scroll;
          },
        ),
    );
  }

  /** The entities a machine may be owned by: the first page, by slug, the choice kept. */
  private fillOwners(): void {
    const select = this.owner;
    const entities = this.entities.items;
    if (!select || !entities) return;
    fill(
      select,
      [...entities].sort((a, b) => a.slug.localeCompare(b.slug)).map((e) => [e.id, e.slug]),
    );
  }
}

class MachineRecord {
  private readonly title = new Title();
  private readonly attributes = new Area();
  private readonly ifaces = new Area();
  private readonly hostedArea = new Area();
  private readonly machine;
  private readonly interfaces;
  private readonly hosted;
  private readonly kit: Kit;
  private creation: DraftForm | null = null;

  constructor(
    view: MachinesView,
    private readonly id: string,
  ) {
    this.kit = view.kit;
    this.machine = this.kit.follow(() => reads.object<MachineCard>("machines.machine", id));
    this.interfaces = this.kit.followList(() => reads.list("/v1/interfaces", { machine: id }));
    this.hosted = this.kit.followList(() =>
      reads.subPages<Machine>("machines.machine", id, "hosted"),
    );
  }

  build(): Node[] {
    const { texts } = this.kit;
    const name = textField(texts, "interfaces.name", "name", { required: true, mono: true });
    const mac = textField(texts, "interfaces.mac", "mac", {
      mono: true,
      hint: "interfaces.macHint",
    });
    this.creation = draftForm(texts, {
      fields: [name, mac],
      submit: "form.submit",
      permission: "machines.interface.edit",
      operation: () => {
        const value = mac.control.value.trim();
        return {
          op: "machines.create_interface",
          machine: { id: this.id },
          name: name.control.value.trim(),
          ...(value ? { mac: value } : {}),
        };
      },
    });
    return [
      stack(
        "6",
        this.title.node,
        this.attributes.node,
        section(texts.text("interfaces.title"), this.ifaces.node),
        section(texts.text("machines.hosted"), this.hostedArea.node),
        section(texts.text("interfaces.create"), this.creation.form),
      ),
    ];
  }

  update(): void {
    const { names } = this.kit;
    this.creation?.refresh();
    const machine = this.machine.data;
    const failed = problemOf(this.machine.error);
    this.title.set(
      machine ? machine.name : t("machines.record"),
      machine ? null : this.machine.error,
      t("machines.failedOne"),
    );
    if (machine) {
      const owner = cited(names, "core.entity", machine.owner?.id);
      const site = cited(names, "core.site", machine.site?.id);
      const host = hostOf(machine, names);
      const member = machine.member_of ? names.of("machines.cluster", machine.member_of.id) : null;
      this.attributes.show(
        [machine.kind, owner.said, site.said, host.said, member && said(member)],
        () =>
          pairs([
            [t("machines.kind"), t(`kind.${machine.kind}`)],
            [t("machines.owner"), owner.node],
            [t("machines.site"), site.node],
            [t("machines.host"), host.node],
            [
              t("machines.memberOf"),
              member && machine.member_of
                ? nameNode(member, machine.member_of.id, PAGES.cluster(machine.member_of.id))
                : none(),
            ],
          ]),
      );
    } else {
      this.attributes.show(null, () => (failed ? [] : skeleton(3)));
    }
    this.drawInterfaces(machine);
    this.drawHosted();
  }

  private drawInterfaces(machine: MachineCard | undefined): void {
    const { names } = this.kit;
    const addresses = new Map(
      (machine?.interfaces ?? []).map((i) => [i.id, i.addresses.map((a) => a.value)]),
    );
    const items = this.interfaces.items ?? [];
    const rows = items.map((i) => ({
      iface: i,
      mode: modeOf(i, names),
      addresses: addresses.get(i.id) ?? [],
    }));
    this.ifaces.show(
      [
        this.interfaces.items === undefined,
        problemOf(this.interfaces.error),
        this.interfaces.more,
        this.interfaces.fetching,
        rows.map((r) => [
          r.iface.id,
          r.iface.name,
          r.iface.mac,
          r.iface.enabled,
          r.mode,
          r.addresses,
        ]),
      ],
      () =>
        listState(
          this.interfaces,
          {
            failed: t("interfaces.failed"),
            empty: t("interfaces.none"),
            emptyText: t("interfaces.noneText"),
          },
          () => interfaceTable(t("interfaces.title"), rows, null),
        ),
    );
  }

  private drawHosted(): void {
    const items = this.hosted.items ?? [];
    this.hostedArea.show(
      [
        this.hosted.items === undefined,
        problemOf(this.hosted.error),
        this.hosted.more,
        this.hosted.fetching,
        items.map((m) => [m.id, m.name, m.kind]),
      ],
      () =>
        listState(
          this.hosted,
          {
            failed: t("machines.failed"),
            empty: t("machines.noneHosted"),
            emptyText: t("machines.noneHostedText"),
          },
          (machines) => {
            const { scroll, body } = table(t("machines.hosted"), [
              t("machines.name"),
              t("machines.kind"),
            ]);
            for (const m of machines) {
              row(body, link(m.name, PAGES.machine(m.id)), t(`kind.${m.kind}`));
            }
            return scroll;
          },
        ),
    );
  }
}

/** A row of a table of interfaces: the interface, its mode, its addresses, its machine. */
export interface InterfaceRow {
  iface: Interface;
  mode: string;
  addresses?: string[];
  machine?: { node: Node; said: string };
}

/** The table of interfaces of a machine (`machine` null) or of any machine. */
export function interfaceTable(
  name: string,
  rows: InterfaceRow[],
  withMachine: true | null,
): HTMLElement {
  const headers = [t("interfaces.name")];
  if (withMachine) headers.push(t("interfaces.machine"));
  headers.push(t("interfaces.mac"), t("interfaces.mode"));
  if (!withMachine) headers.push(t("interfaces.addresses"));
  headers.push(t("interfaces.state"));
  const { scroll, body } = table(name, headers);
  for (const r of rows) {
    const cells: (string | Node)[] = [];
    if (withMachine) cells.push(r.machine?.node ?? none());
    cells.push(mono(r.iface.mac), r.mode);
    if (!withMachine) {
      cells.push(r.addresses?.length ? mono(r.addresses.join(", ")) : none());
    }
    cells.push(t(r.iface.enabled ? "interfaces.enabled" : "interfaces.disabled"));
    row(body, link(r.iface.name, PAGES.interface(r.iface.id), true), ...cells);
  }
  return scroll;
}

// ----------------------------------------------------------------------------- clusters

/** The view `clusters`: the list of the clusters, or the record of one with its machines. */
export class ClustersView extends MachinesElement {
  private readonly names = new Names(
    () => this.update(),
    () => this.isConnected,
  );
  // Not `title` nor `id`: members of every `HTMLElement`.
  private readonly top = new Title(true);
  private readonly main = new Area();
  private readonly members = new Area();
  private objectId: string | null = null;
  private list: FollowedList<Cluster> | null = null;
  private cluster: Followed<Cluster> | null = null;
  private machines: FollowedList<Machine> | null = null;

  protected build(): Node[] {
    this.objectId = pageId("clusters");
    if (!this.objectId) {
      this.list = this.followList(() => reads.list("/v1/clusters"));
      return [
        stack(
          "6",
          heading(this.texts.text("clusters.title"), this.texts.text("clusters.intro")),
          this.main.node,
        ),
      ];
    }
    const id = this.objectId;
    this.cluster = this.follow(() => reads.object<Cluster>("machines.cluster", id));
    this.machines = this.followList(() =>
      reads.subPages<Machine>("machines.cluster", id, "machines"),
    );
    return [
      stack(
        "6",
        this.top.node,
        this.main.node,
        section(this.texts.text("clusters.machines"), this.members.node),
      ),
    ];
  }

  protected update(): void {
    if (this.list) this.drawList(this.list);
    else this.drawRecord();
  }

  private drawList(list: FollowedList<Cluster>): void {
    const rows = (list.items ?? []).map((c) => ({
      cluster: c,
      owner: cited(this.names, "core.entity", c.owner?.id),
      site: cited(this.names, "core.site", c.site?.id),
    }));
    this.main.show(
      [
        list.items === undefined,
        problemOf(list.error),
        list.more,
        list.fetching,
        rows.map((r) => [r.cluster.id, r.cluster.slug, r.owner.said, r.site.said]),
      ],
      () =>
        listState(
          list,
          {
            failed: t("clusters.failed"),
            empty: t("clusters.none"),
            emptyText: t("clusters.noneText"),
          },
          () => {
            const { scroll, body } = table(t("clusters.title"), [
              t("clusters.slug"),
              t("machines.owner"),
              t("machines.site"),
            ]);
            for (const r of rows) {
              row(
                body,
                link(r.cluster.slug, PAGES.cluster(r.cluster.id), true),
                r.owner.node,
                r.site.node,
              );
            }
            return scroll;
          },
        ),
    );
  }

  private drawRecord(): void {
    const cluster = this.cluster?.data;
    const failed = problemOf(this.cluster?.error);
    this.top.set(
      cluster ? cluster.slug : t("clusters.record"),
      cluster ? null : this.cluster?.error,
      t("clusters.failedOne"),
    );
    if (cluster) {
      const owner = cited(this.names, "core.entity", cluster.owner?.id);
      const site = cited(this.names, "core.site", cluster.site?.id);
      this.main.show([owner.said, site.said], () =>
        pairs([
          [t("machines.owner"), owner.node],
          [t("machines.site"), site.node],
        ]),
      );
    } else {
      this.main.show(null, () => (failed ? [] : skeleton(2)));
    }
    const machines = this.machines;
    if (!machines) return;
    const id = this.objectId;
    const items = machines.items ?? [];
    this.members.show(
      [
        machines.items === undefined,
        problemOf(machines.error),
        machines.more,
        machines.fetching,
        items.map((m) => [m.id, m.name, m.kind, m.member_of?.id, m.host]),
      ],
      () =>
        listState(
          machines,
          {
            failed: t("machines.failed"),
            empty: t("clusters.noMachines"),
            emptyText: t("clusters.noMachinesText"),
          },
          (list) => {
            const { scroll, body } = table(t("clusters.machines"), [
              t("machines.name"),
              t("machines.kind"),
              t("clusters.relation"),
            ]);
            for (const m of list) {
              const member = m.member_of?.id === id;
              const hosted = m.host && "cluster" in m.host && m.host.cluster.id === id;
              const relation = [member && t("clusters.member"), hosted && t("clusters.hosted")]
                .filter(Boolean)
                .join(", ");
              row(body, link(m.name, PAGES.machine(m.id)), t(`kind.${m.kind}`), relation || none());
            }
            return scroll;
          },
        ),
    );
  }
}

// --------------------------------------------------------------------------- interfaces

/** The criteria of the search of the interfaces, as the address of the page holds them. */
interface Criteria {
  mac: string;
  address: string;
  vid: string;
}

function criteriaOfPage(): Criteria {
  const query = new URLSearchParams(location.search);
  return {
    mac: query.get("mac") ?? "",
    address: query.get("address") ?? "",
    vid: query.get("vid") ?? "",
  };
}

/** The identifiers of the objects whose `filter` is `value`: the first page of the list. */
async function idsOf(path: "/v1/addresses" | "/v1/vlans", filter: string, value: string) {
  const data = await queryClient.fetchInfiniteQuery(reads.list(path, { [filter]: value }) as never);
  const pages = (data as { pages: Entry<{ items: { id: string }[] }>[] }).pages;
  return pages.flatMap((page) => page.data.items.map((item) => item.id));
}

/** The view `interfaces`: a search by MAC, address and VID, or the record of one. */
export class InterfacesView extends MachinesElement {
  private readonly names = new Names(
    () => this.update(),
    () => this.isConnected,
  );
  // Not `title` nor `id`: members of every `HTMLElement`.
  private readonly top = new Title(true);
  private readonly main = new Area();
  private readonly addressesArea = new Area();
  /** What the search found, in words: a live region there before its first sentence. */
  private readonly announced = (() => {
    const region = el("p", "", "k-visually-hidden");
    region.setAttribute("role", "status");
    return region;
  })();
  private objectId: string | null = null;
  // The search: one list per address and VLAN found (an address in several domains, a VID
  // in several groups), their interfaces merged.
  private searches: FollowedList<Interface>[] = [];
  /** The criteria the lists were made for; a resolution that failed or found nothing. */
  private searched: { criteria: Criteria; problem: unknown; nothing: string | null } | null = null;
  private generation = 0;
  private iface: Followed<Interface> | null = null;
  private card: Followed<MachineCard> | null = null;
  private vlans: FollowedList<Vlan> | null = null;
  private mode: DraftForm | null = null;
  private access: HTMLSelectElement | null = null;
  private modeSelect: HTMLSelectElement | null = null;

  protected build(): Node[] {
    this.objectId = pageId("interfaces");
    return this.objectId ? this.buildRecord(this.objectId) : this.buildSearch();
  }

  protected update(): void {
    if (this.objectId) this.drawRecord();
    else this.drawSearch();
  }

  private buildSearch(): Node[] {
    const { texts } = this;
    const criteria = criteriaOfPage();
    const mac = textField(texts, "search.mac", "mac", { mono: true });
    const address = textField(texts, "search.address", "address", { mono: true });
    const vid = textField(texts, "search.vid", "vid", { mono: true, numeric: true });
    mac.control.value = criteria.mac;
    address.control.value = criteria.address;
    vid.control.value = criteria.vid;
    const form = el("form", undefined, "k-machines-search");
    form.setAttribute("role", "search");
    form.setAttribute("aria-label", t("search.label"));
    const fields = el("div", undefined, "k-machines-fields");
    fields.append(mac.field, address.field, vid.field);
    const actions = el("div", undefined, "k-machines-actions");
    const submit = button(texts.text("search.submit"), "primary", "submit");
    const clear = button(texts.text("search.clear"), "secondary");
    actions.append(submit, clear);
    form.append(fields, actions);
    const run = () => {
      const next = {
        mac: mac.control.value.trim(),
        address: address.control.value.trim(),
        vid: vid.control.value.trim(),
      };
      // The criteria in the address of the page, which a link shares; the shell's state of
      // the entry is kept as it is.
      const search = new URLSearchParams(location.search);
      for (const [key, value] of Object.entries(next)) {
        if (value) search.set(key, value);
        else search.delete(key);
      }
      const query = search.toString();
      history.replaceState(history.state, "", `${location.pathname}${query ? `?${query}` : ""}`);
      void this.search(next);
    };
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      run();
    });
    clear.addEventListener("click", () => {
      mac.control.value = "";
      address.control.value = "";
      vid.control.value = "";
      run();
    });
    void this.search(criteria);
    return [
      stack(
        "6",
        heading(texts.text("interfaces.title"), texts.text("interfaces.intro")),
        form,
        this.announced,
        this.main.node,
      ),
    ];
  }

  /** Resolves the address and the VID to their objects, then lists their interfaces. */
  private async search(criteria: Criteria): Promise<void> {
    const generation = ++this.generation;
    for (const list of this.searches.splice(0)) this.drop(list);
    this.searched = null;
    this.update();
    let addresses: (string | undefined)[] = [undefined];
    let vlans: (string | undefined)[] = [undefined];
    try {
      if (criteria.address) addresses = await idsOf("/v1/addresses", "value", criteria.address);
      if (criteria.vid) vlans = await idsOf("/v1/vlans", "vid", criteria.vid);
    } catch (problem) {
      if (generation !== this.generation) return;
      this.searched = { criteria, problem, nothing: null };
      this.update();
      return;
    }
    if (generation !== this.generation) return;
    const nothing =
      addresses.length === 0
        ? t("search.noAddress", { value: criteria.address })
        : vlans.length === 0
          ? t("search.noVlan", { value: criteria.vid })
          : null;
    this.searched = { criteria, problem: null, nothing };
    if (!nothing) {
      for (const address of addresses) {
        for (const vlan of vlans) {
          const filters = {
            ...(criteria.mac ? { mac: criteria.mac } : {}),
            ...(address ? { address } : {}),
            ...(vlan ? { vlan } : {}),
          };
          this.searches.push(this.followList(() => reads.list("/v1/interfaces", filters)));
        }
      }
    }
    this.update();
  }

  private drawSearch(): void {
    const searched = this.searched;
    const lists = this.searches;
    const seen = new Set<string>();
    const items: Interface[] = [];
    for (const list of lists) {
      for (const item of list.items ?? []) {
        if (!seen.has(item.id)) {
          seen.add(item.id);
          items.push(item);
        }
      }
    }
    const loading = !searched || lists.some((l) => l.items === undefined && !l.error);
    const error = (searched?.problem ?? lists.find((l) => l.error)?.error) || null;
    const rows: InterfaceRow[] = items.map((iface) => {
      const name = this.names.of("machines.machine", iface.machine.id);
      return {
        iface,
        mode: modeOf(iface, this.names),
        machine: {
          node: nameNode(name, iface.machine.id, PAGES.machine(iface.machine.id)),
          said: said(name),
        },
      };
    });
    const more = lists.filter((l) => l.more);
    // What the search found, said by a live region there from the start.
    const sentence = searched?.nothing
      ? searched.nothing
      : error && items.length === 0
        ? t("interfaces.failed")
        : loading && items.length === 0
          ? t("search.searching")
          : items.length === 0
            ? t("search.none")
            : t("search.count", { n: items.length });
    if (this.announced.textContent !== sentence) this.announced.textContent = sentence;
    this.main.show(
      [
        loading,
        problemOf(error),
        searched?.nothing ?? null,
        more.length,
        lists.some((l) => l.fetching),
        rows.map((r) => [
          r.iface.id,
          r.iface.name,
          r.iface.mac,
          r.iface.enabled,
          r.mode,
          r.machine?.said,
        ]),
      ],
      () => {
        if (searched?.nothing) return empty(searched.nothing, t("search.again"));
        if (error && items.length === 0) return failure(t("interfaces.failed"), error);
        if (loading && items.length === 0) return skeleton(4);
        if (items.length === 0) return empty(t("search.none"), t("search.again"));
        const nodes: Node[] = [];
        if (error) nodes.push(failure(t("interfaces.failed"), error));
        const count = el("p", t("search.count", { n: items.length }), "k-machines-muted");
        count.setAttribute("aria-hidden", "true");
        nodes.push(count, interfaceTable(t("interfaces.found"), rows, true));
        if (more.length) {
          const next = button(document.createTextNode(t("list.more")), "secondary");
          next.dataset["kKeep"] = "more";
          if (more.some((l) => l.fetching)) next.setAttribute("aria-busy", "true");
          next.addEventListener("click", () => more.forEach((l) => l.next()));
          const actions = el("div", undefined, "k-machines-actions");
          actions.append(next);
          nodes.push(actions);
        }
        return stack("2", ...nodes);
      },
    );
  }

  private buildRecord(id: string): Node[] {
    const { texts } = this;
    this.iface = this.follow(() => reads.object<Interface>("machines.interface", id));
    this.vlans = this.followList(() => reads.list("/v1/vlans"));
    const mode = selectField(texts, "interfaces.mode", "mode", { required: true });
    for (const value of ["none", "access"]) {
      mode.control.append(option(value, texts.text(`mode.${value}`)));
    }
    const access = selectField(texts, "interfaces.access", "access");
    this.modeSelect = mode.control;
    this.access = access.control;
    // The VLAN is asked, and required, in the mode access only.
    const toggle = () => {
      const needed = mode.control.value === "access";
      access.field.hidden = !needed;
      access.control.required = needed;
    };
    mode.control.addEventListener("change", toggle);
    toggle();
    this.mode = draftForm(texts, {
      fields: [mode, access],
      submit: "form.submit",
      permission: "machines.interface.edit",
      added: () => {},
      operation: () => {
        if (mode.control.value === "access" && !access.control.value) {
          access.refuse(t("form.required"));
          access.control.focus();
          return null;
        }
        const vlan =
          mode.control.value === "access"
            ? { mode: "access", access: { id: access.control.value } }
            : { mode: "none" };
        return { op: "machines.update_interface", id: { id }, vlan } as Operation;
      },
    });
    return [
      stack(
        "6",
        this.top.node,
        this.main.node,
        section(texts.text("interfaces.addresses"), this.addressesArea.node),
        section(texts.text("interfaces.setMode"), this.mode.form),
      ),
    ];
  }

  private drawRecord(): void {
    this.mode?.refresh();
    const iface = this.iface?.data;
    const failed = problemOf(this.iface?.error);
    // The card of its machine, which holds its addresses, once the interface names it.
    if (iface && !this.card) {
      const machine = iface.machine.id;
      this.card = this.follow(() => reads.object<MachineCard>("machines.machine", machine));
    }
    this.fillVlans(iface);
    const machineName = iface ? this.names.of("machines.machine", iface.machine.id) : null;
    this.top.set(
      iface ? iface.name : t("interfaces.record"),
      iface ? null : this.iface?.error,
      t("interfaces.failedOne"),
    );
    if (iface && machineName) {
      const mode = modeOf(iface, this.names);
      this.main.show([said(machineName), iface.mac, iface.enabled, mode], () =>
        pairs([
          [
            t("interfaces.machine"),
            nameNode(machineName, iface.machine.id, PAGES.machine(iface.machine.id)),
          ],
          [t("interfaces.mac"), mono(iface.mac)],
          [t("interfaces.mode"), mode],
          [t("interfaces.state"), t(iface.enabled ? "interfaces.enabled" : "interfaces.disabled")],
        ]),
      );
    } else {
      this.main.show(null, () => (failed ? [] : skeleton(3)));
    }
    const card = this.card;
    const own = card?.data?.interfaces.find((i) => i.id === this.objectId);
    this.addressesArea.show(
      [card?.data === undefined, problemOf(card?.error), own?.addresses ?? null],
      () => {
        if (!card?.data) {
          return card?.error ? failure(t("interfaces.failedAddresses"), card.error) : skeleton(2);
        }
        const addresses = own?.addresses ?? [];
        if (addresses.length === 0) {
          return empty(t("interfaces.noAddress"), t("interfaces.noAddressText"));
        }
        const { scroll, body } = table(t("interfaces.addresses"), [
          t("tab.address"),
          t("tab.primary"),
        ]);
        for (const address of addresses) {
          row(
            body,
            link(address.value, `/addresses/${encodeURIComponent(address.id)}`, true),
            t(address.primary ? "tab.yes" : "tab.no"),
          );
        }
        return scroll;
      },
    );
  }

  /** The VLAN an interface may be in access on: the first page, by VID, its own chosen. */
  private fillVlans(iface: Interface | undefined): void {
    const select = this.access;
    const vlans = this.vlans?.items;
    if (!select || !vlans || !iface || !this.modeSelect) return;
    const first = select.dataset["filled"] === undefined;
    const options: [string, string][] = [
      ["", t("interfaces.chooseVlan")],
      ...[...vlans]
        .sort((a, b) => a.vid - b.vid)
        .map((v): [string, string] => [v.id, `${v.vid} · ${v.name}`]),
    ];
    if (!fill(select, options) || !first) return;
    // The mode the interface has, to change from.
    this.modeSelect.value = iface.vlan.mode === "access" ? "access" : "none";
    this.modeSelect.dispatchEvent(new Event("change"));
    select.value = iface.vlan.access?.id ?? "";
  }
}
