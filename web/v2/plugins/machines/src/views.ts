// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The view of the clusters under the space `hosts` (spec 021, design « Plugin Machines »),
// the list and the records of its objects, still without framework: `/x/machines/clusters`,
// `…/<id>` (task 18cd moves it to Vue). The views of the machines and of the interfaces are
// in Vue (`MachinesView.vue`, task 18cb; `InterfacesView.vue`, task 18cc). The shell creates
// the element again for each path of the page: it reads its own at its creation.

import { reads } from "@kartenn/sdk";
import {
  Area,
  button,
  el,
  empty,
  failure,
  link,
  mono,
  none,
  pageId,
  PAGES,
  pairs,
  problemOf,
  row,
  section,
  skeleton,
  stack,
  t,
  table,
} from "./dom.ts";
import { MachinesElement, type Followed, type FollowedList } from "./element.ts";
import { Names, type Cluster, type Machine, type Name } from "./reads.ts";

/** A name as drawn: the name, or a short wait, or the identifier of an unreadable object. */
function nameNode(name: Name, id: string, page?: string): Node {
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

/** An entity or a site cited by its identifier: its slug once read. */
function cited(names: Names, type: "core.entity" | "core.site", id?: string | null) {
  if (!id) return { node: none(), said: "" };
  const name = names.of(type, id);
  return { node: nameNode(name, id), said: said(name) };
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
