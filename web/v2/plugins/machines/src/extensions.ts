// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What the plugin adds to the screens of the core, without a framework: the « Interfaces »
// tab of an address, of a network and of a VLAN (task 18cc moves it to Vue). The machine
// column of the lists of addresses and the action that attaches an address are in Vue
// (`MachineColumn.vue`, `AttachAction.vue`, task 18ca).

import { reads } from "@kartenn/sdk";
import {
  Area,
  el,
  empty,
  failure,
  link,
  mono,
  PAGES,
  problemOf,
  row,
  skeleton,
  stack,
  t,
  table,
} from "./dom.ts";
import { MachinesElement, type Followed, type FollowedList } from "./element.ts";
import {
  modeOf,
  Names,
  within,
  type Interface,
  type Machine,
  type MachineAddress,
} from "./reads.ts";
import { interfaceTable, nameNode, type InterfaceRow } from "./views.ts";

type Prefix = { value: string };

/**
 * The « Interfaces » tab: the interfaces that hold an address, or that carry a VLAN; for a
 * network, the interfaces of the machines that hold one of its addresses, with the address.
 */
export class InterfacesTab extends MachinesElement {
  private readonly names = new Names(
    () => this.update(),
    () => this.isConnected,
  );
  private readonly area = new Area();
  private interfaces: FollowedList<Interface> | null = null;
  // Not `prefix`: a member of every `Element` (its namespace prefix).
  private network: Followed<Prefix> | null = null;
  private machines: FollowedList<Machine> | null = null;
  /** The addresses of each machine of the network, followed as the list of machines says. */
  private readonly addresses = new Map<string, Followed<{ items: MachineAddress[] }>>();

  protected build(): Node[] {
    const object = this.object;
    if (object?.type === "prefix") {
      this.network = this.follow(() => reads.object<Prefix>("prefix", object.id));
      this.machines = this.followList(() => reads.list("/v1/machines", { network: object.id }));
    } else if (object) {
      const filter = object.type === "vlan" ? { vlan: object.id } : { address: object.id };
      this.interfaces = this.followList(() => reads.list("/v1/interfaces", filter));
    }
    // No heading of its own: the tab of the shell names its panel, the table its caption.
    return [this.area.node];
  }

  protected update(): void {
    if (this.interfaces) this.drawInterfaces(this.interfaces);
    else if (this.machines) this.drawNetwork(this.machines);
  }

  private drawInterfaces(list: FollowedList<Interface>): void {
    const rows: InterfaceRow[] = (list.items ?? []).map((iface) => {
      const name = this.names.of("machines.machine", iface.machine.id);
      return {
        iface,
        mode: modeOf(iface, this.names),
        machine: {
          node: nameNode(name, iface.machine.id, PAGES.machine(iface.machine.id)),
          said: name.state === "read" ? name.name : name.state,
        },
      };
    });
    this.area.show(
      [
        list.items === undefined,
        problemOf(list.error),
        list.more,
        rows.map((r) => [r.iface.id, r.iface.name, r.iface.mac, r.mode, r.machine?.said]),
      ],
      () => {
        const items = list.items;
        if (!items) return list.error ? failure(t("tab.failed"), list.error) : skeleton(3);
        if (items.length === 0) return empty(t("tab.none"), t("tab.noneText"));
        const nodes: Node[] = [interfaceTable(t("tab.title"), rows, true)];
        if (list.error) nodes.unshift(failure(t("tab.failed"), list.error));
        if (list.more) nodes.push(el("p", t("tab.more"), "k-machines-muted"));
        return stack("2", ...nodes);
      },
    );
  }

  private drawNetwork(list: FollowedList<Machine>): void {
    const machines = list.items ?? [];
    for (const machine of machines) {
      if (!this.addresses.has(machine.id)) {
        const id = machine.id;
        this.addresses.set(
          id,
          this.follow(() =>
            reads.sub<{ items: MachineAddress[] }>("machines.machine", id, "addresses"),
          ),
        );
      }
    }
    const prefix = this.network?.data?.value;
    const reading =
      list.items === undefined ||
      !prefix ||
      machines.some((m) => {
        const read = this.addresses.get(m.id)!;
        return read.data === undefined && !read.error;
      });
    const error =
      list.error ??
      this.network?.error ??
      machines.map((m) => this.addresses.get(m.id)!.error).find(Boolean) ??
      null;
    const rows = machines.flatMap((machine) =>
      (this.addresses.get(machine.id)?.data?.items ?? [])
        .filter((a) => prefix !== undefined && within(a.value, prefix))
        .map((address) => ({ machine, address })),
    );
    this.area.show(
      [
        reading,
        problemOf(error),
        list.more,
        rows.map((r) => [
          r.machine.id,
          r.machine.name,
          r.address.interface.id,
          r.address.value,
          r.address.primary,
        ]),
      ],
      () => {
        if (rows.length === 0 && error) return failure(t("tab.failed"), error);
        if (rows.length === 0 && reading) return skeleton(3);
        if (rows.length === 0) return empty(t("tab.noneNetwork"), t("tab.noneText"));
        const { scroll, body } = table(t("tab.title"), [
          t("interfaces.name"),
          t("interfaces.machine"),
          t("tab.address"),
          t("tab.primary"),
        ]);
        for (const { machine, address } of rows) {
          row(
            body,
            link(address.interface_name, PAGES.interface(address.interface.id), true),
            link(machine.name, PAGES.machine(machine.id)),
            mono(address.value),
            t(address.primary ? "tab.yes" : "tab.no"),
          );
        }
        const nodes: Node[] = [scroll];
        if (error) nodes.unshift(failure(t("tab.failed"), error));
        if (list.more) nodes.push(el("p", t("tab.more"), "k-machines-muted"));
        return stack("2", ...nodes);
      },
    );
  }
}
