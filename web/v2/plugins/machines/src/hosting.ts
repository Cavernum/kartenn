// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The host of a machine changed from its record (spec 021, task 18b; REQ-021-32, 33; design
// « Déplacements dans les arbres »): the counterpart, without framework, of a drop in the tree
// of the hosting, which the `TreeGrid` of @kartenn/ui (a component of Vue) cannot draw here.
// Each host offered is checked by `canMove` of the SDK — a cycle, a type the contract does not
// admit, a right, the present —, refused ones kept in the list, active and reachable at the
// keyboard, their reason and the code of the rule of the server in their name, refused again
// on the field if sent. The move goes to the active draft by `draft.move`, which simulates
// it: never optimistic, the host shown stays the one read, « en attente » until the answer;
// the server stays the only judge (its rules on the kinds of the pair, the site of the host).

import {
  canMove,
  context,
  draft,
  MAX_PAGES,
  type DraftMove,
  type MoveSubject,
  type MoveVerdict,
  type Operation,
} from "@kartenn/sdk";
import {
  Area,
  button,
  el,
  failure,
  option,
  problemOf,
  ruled,
  selectField,
  t,
  type Field,
  type Texts,
} from "./dom.ts";
import { present, type FollowedList } from "./element.ts";
import type { Cluster, Machine, MachineCard } from "./reads.ts";

/** What the record gives the move: its texts, the clusters and machines it follows. */
export interface HostKit {
  texts: Texts;
  clusters: FollowedList<Cluster>;
  machines: FollowedList<Machine>;
}

/** Longest chain of hosts walked for the ancestors (M5: two levels; more is a loop). */
const MAX_DEPTH = 8;

/** The machine moved: its type in the table of the admitted parents. */
const MACHINE = "machines.machine";
const CLUSTER = "machines.cluster";

/** A host offered: its value in the list, the node `canMove` judges, its name. */
interface Host {
  value: string;
  subject: MoveSubject | null;
  label: string;
}

/** The host of a machine as a value of the list: `machine:<id>`, `cluster:<id>`, `none`. */
function valueOf(host: Machine["host"]): string {
  if (!host) return "none";
  return "machine" in host ? `machine:${host.machine.id}` : `cluster:${host.cluster.id}`;
}

/** The member `host` of `machines.update_machine` for a host chosen (`null`: none). */
function hostMember(subject: MoveSubject | null) {
  if (!subject) return null;
  return subject.type === CLUSTER
    ? { cluster: { id: subject.id } }
    : { machine: { id: subject.id } };
}

/** The section « Hébergement » of the record of a machine: its form and what it said. */
export class HostMove {
  readonly node: HTMLElement;
  private readonly clusters: FollowedList<Cluster>;
  private readonly machines: FollowedList<Machine>;
  private readonly field: Field<HTMLSelectElement>;
  private readonly submit: HTMLButtonElement;
  private readonly reason: HTMLParagraphElement;
  /** The state of the lists of hosts: read, failed, pages still coming. */
  private readonly lists = new Area();
  /**
   * What the last move says, in live regions there from the start: its outcome, the
   * warnings of the server; a refusal, as an alert.
   */
  private readonly said: HTMLParagraphElement;
  private readonly warned: HTMLDivElement;
  private readonly alerts: HTMLDivElement;
  private machine: MachineCard | undefined;
  private filled = "";
  /** The value of the host whose move is in flight, `null` if none. */
  private sent: string | null = null;
  /** Whether the list has the focus: not drawn again under it. */
  private focused = false;

  constructor(
    kit: HostKit,
    private readonly id: string,
  ) {
    const { texts } = kit;
    this.clusters = kit.clusters;
    this.machines = kit.machines;
    this.field = selectField(texts, "move.host", "host", { required: true, hint: "move.hint" });
    this.submit = button(texts.text("move.submit"), "primary", "submit");
    this.reason = el("p", undefined, "k-machines-muted");
    this.reason.id = `k-machines-move-reason-${id}`;
    this.reason.hidden = true;
    this.said = el("p", undefined, "k-machines-outcome");
    this.said.setAttribute("role", "status");
    this.warned = el("div", undefined, "k-machines-outcome");
    this.warned.setAttribute("role", "status");
    this.alerts = el("div", undefined, "k-machines-outcome");
    const form = el("form", undefined, "k-stack");
    form.dataset["kGap"] = "4";
    form.noValidate = true;
    const actions = el("div", undefined, "k-machines-actions");
    actions.append(this.submit);
    const fields = el("div", undefined, "k-machines-fields");
    fields.append(this.field.field);
    form.append(this.lists.node, fields, this.said, this.warned, this.alerts, actions, this.reason);
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      void this.move();
    });
    // What the last move said, gone once another host is chosen.
    // While a move is in flight, the list shows the host sent and its wait stays said: the
    // choice comes back to it (a second intent is ignored, never shown as sent).
    this.field.control.addEventListener("change", () => {
      if (this.sent !== null) this.field.control.value = this.sent;
      else this.clear();
    });
    // The list is not drawn again under the focus: once it leaves, with what came meanwhile.
    this.field.control.addEventListener("focus", () => (this.focused = true));
    this.field.control.addEventListener("blur", () => {
      this.focused = false;
      // Not once the record is gone (a blur of its removal).
      if (this.node.isConnected) this.update(this.machine);
    });
    this.node = form;
  }

  /** Whether the caller writes machines (`canMove` asks it again with the target's). */
  private allowed(): boolean {
    return context.get().user.has("machines.edit");
  }

  private env() {
    // The host machine of each machine read: the ancestors of a target, from its parent up.
    const hosts = new Map<string, string>();
    for (const m of this.machines.items ?? []) {
      if (m.host && "machine" in m.host) hosts.set(m.id, m.host.machine.id);
    }
    return {
      ancestors: (id: string) => {
        const up: string[] = [];
        for (let at = hosts.get(id); at && up.length < MAX_DEPTH; at = hosts.get(at)) {
          if (up.includes(at)) break;
          up.push(at);
        }
        return up;
      },
      can: (permission: string) => context.get().user.has(permission),
      present: present(),
    };
  }

  /** The reason of a refusal, in words, with the code of the rule of the server. */
  private refusal(verdict: Extract<MoveVerdict, { ok: false }>): string {
    const reason = t(`move.${verdict.refusal}`, { permission: verdict.permission ?? "" });
    return verdict.code ? `${reason} (${verdict.code})` : reason;
  }

  /** The hosts offered: none, the clusters and the machines read (ten pages at most). */
  private hosts(): { clusters: Host[]; machines: Host[] } {
    return {
      clusters: [...(this.clusters.items ?? [])]
        .sort((a, b) => a.slug.localeCompare(b.slug))
        .map((c) => ({
          value: `cluster:${c.id}`,
          subject: { id: c.id, type: CLUSTER },
          label: c.slug,
        })),
      machines: [...(this.machines.items ?? [])]
        .sort((a, b) => a.name.localeCompare(b.name))
        .map((m) => ({
          value: `machine:${m.id}`,
          subject: { id: m.id, type: MACHINE },
          label: m.name,
        })),
    };
  }

  /** Each host with its verdict: the current one, or the first refusal of `canMove`. */
  private judged(host: Host, current: string, env: ReturnType<HostMove["env"]>) {
    if (host.value === current) return { ...host, refused: t("move.current") };
    const verdict = canMove({ id: this.id, type: MACHINE }, host.subject, env);
    return { ...host, refused: verdict.ok ? null : this.refusal(verdict) };
  }

  update(machine: MachineCard | undefined): void {
    this.machine = machine;
    // At another date, nothing of the move is offered (W9).
    (this.node.closest("section") ?? this.node).hidden = !present();
    this.more(this.clusters);
    this.more(this.machines);
    this.refuseWithout();
    this.drawLists();
    const select = this.field.control;
    if (!machine || this.focused) return;
    const current = valueOf(machine.host);
    const env = this.env();
    const { clusters, machines } = this.hosts();
    const none = this.judged({ value: "none", subject: null, label: t("move.none") }, current, env);
    const groups: [string, ReturnType<HostMove["judged"]>[]][] = [
      [t("move.clusters"), clusters.map((h) => this.judged(h, current, env))],
      [t("move.machines"), machines.map((h) => this.judged(h, current, env))],
    ];
    // Drawn again only when what it offers changes: the choice kept while still offered.
    const signature = JSON.stringify([none, groups]);
    if (signature === this.filled) return;
    this.filled = signature;
    const chosen = select.value;
    // A refused host stays reachable at the keyboard and read: its reason in its name,
    // refused again on the field if it is sent (a disabled option is skipped).
    const entry = (h: ReturnType<HostMove["judged"]>) =>
      option(h.value, h.refused ? `${h.label} — ${h.refused}` : h.label);
    const placeholder = option("", t("move.choose"));
    placeholder.disabled = true;
    placeholder.hidden = true;
    select.replaceChildren(
      placeholder,
      entry(none),
      ...groups
        .filter(([, hosts]) => hosts.length)
        .map(([label, hosts]) => {
          const group = el("optgroup");
          group.label = label;
          group.append(...hosts.map(entry));
          return group;
        }),
    );
    const still = chosen && [...select.options].some((o) => o.value === chosen);
    select.value = still ? chosen : "";
  }

  /** Whether the hosts are read, failed, or still coming page after page. */
  private drawLists(): void {
    const lists = [this.clusters, this.machines];
    const failed = lists.find((l) => l.error)?.error;
    const reading = lists.some((l) => l.items === undefined || l.fetching);
    this.lists.show([problemOf(failed), !failed && reading], () => {
      if (failed) return failure(t("move.unreadable"), failed);
      return reading ? [el("p", t("move.reading"), "k-machines-muted")] : [];
    });
  }

  /** The pages after the first, ten at most: every host offered, its ancestors known. */
  private more<T>(list: FollowedList<T>): void {
    const pages = list.observer.getCurrentResult().data?.pages.length ?? 0;
    if (list.more && !list.fetching && pages < MAX_PAGES) list.next();
  }

  /** Without `machines.edit`, the button stays, refused with its reason beside it. */
  private refuseWithout(): void {
    if (this.allowed()) {
      if (this.submit.getAttribute("aria-busy") !== "true") {
        this.submit.removeAttribute("aria-disabled");
      }
      this.submit.removeAttribute("aria-describedby");
      this.reason.hidden = true;
      return;
    }
    this.submit.setAttribute("aria-disabled", "true");
    this.reason.replaceChildren(t("form.needs"), " ", el("code", "machines.edit"));
    this.reason.hidden = false;
    this.submit.setAttribute("aria-describedby", this.reason.id);
  }

  private clear(): void {
    this.said.textContent = "";
    this.warned.replaceChildren();
    this.alerts.replaceChildren();
    this.field.refuse(null);
  }

  /** The move of the host chosen, checked again: to the draft, then its simulation. */
  private async move(): Promise<void> {
    // A second intent while one is in flight: ignored.
    if (!this.allowed() || !present() || this.submit.getAttribute("aria-busy") === "true") return;
    this.clear();
    const select = this.field.control;
    const value = select.value;
    if (!value) {
      this.field.refuse(t("form.required"));
      select.focus();
      return;
    }
    const { clusters, machines } = this.hosts();
    const host =
      value === "none"
        ? { value, subject: null, label: t("move.none") }
        : [...clusters, ...machines].find((h) => h.value === value);
    if (!host) return;
    // Its host now, or a host `canMove` refuses: said on the field, nothing sent.
    const verdict = canMove({ id: this.id, type: MACHINE }, host.subject, this.env());
    const current = this.machine && valueOf(this.machine.host) === value;
    if (current || !verdict.ok) {
      this.field.refuse(current || verdict.ok ? t("move.already") : this.refusal(verdict));
      select.focus();
      return;
    }
    this.submit.setAttribute("aria-busy", "true");
    this.submit.setAttribute("aria-disabled", "true");
    this.sent = value;
    // The host shown stays the one read until its read shows another (X33).
    this.said.textContent = t("move.pending", { host: host.label });
    const op = {
      op: "machines.update_machine",
      id: { id: this.id },
      host: hostMember(host.subject),
    } as Operation;
    try {
      this.outcome(await draft.move(op, { id: this.id, type: MACHINE }), host.label);
    } catch (error) {
      this.said.textContent = "";
      this.alerts.replaceChildren(failure(t("move.failed", { host: host.label }), error));
    } finally {
      this.sent = null;
      this.submit.removeAttribute("aria-busy");
      this.refuseWithout();
    }
  }

  /** What the draft and its simulation said of the move. */
  private outcome(outcome: DraftMove, host: string): void {
    if (outcome.kind !== "drafted") {
      this.said.textContent = "";
      this.alerts.replaceChildren(failure(t("move.failed", { host }), outcome.problem));
      return;
    }
    if (outcome.verdict === "refused") {
      // Refused by a rule of the server: the operation stays in the draft, a new choice
      // replaces it.
      this.said.textContent = "";
      this.alerts.replaceChildren(
        ruled("ko", t("move.refused", { host }), [outcome.violation]),
        el("p", t("move.kept"), "k-machines-muted"),
      );
      return;
    }
    if (outcome.verdict === "unknown") {
      this.said.textContent = t("move.drafted", { host });
      this.alerts.replaceChildren(failure(t("move.unsimulated"), outcome.problem));
      return;
    }
    this.said.textContent = t("move.valid", { host });
    // Written into a live region there from the start: read with the outcome.
    this.warned.replaceChildren(
      ...(outcome.warnings.length ? [ruled("warn", t("move.warnings"), outcome.warnings)] : []),
    );
  }
}
