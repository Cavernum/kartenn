// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { afterEach, describe, expect, it, vi } from "vitest";
import * as host from "../src/host.ts";
import * as sdk from "../src/index.ts";

// The commands and shortcuts of plugins (spec 021, REQ-021-3, 7; design « API des
// plugins »): declared through the SDK, read by the shell through the host.

const removals: (() => void)[] = [];
afterEach(() => {
  for (const remove of removals.splice(0)) remove();
  host.reserveKeys([]);
});

const command = (id: string): sdk.Command => ({ id, label: () => id, run: () => {} });
const shortcut = (id: string, key: string): sdk.Shortcut => ({
  id,
  key,
  label: () => id,
  scope: "view",
  run: () => {},
});

describe("commands", () => {
  it("are listed in order, followed, and removed by what register returns", () => {
    const changes = vi.fn();
    const stop = host.commandRegistry.subscribe(changes);
    const remove = sdk.commands.register(command("vlan.create"));
    removals.push(sdk.commands.register(command("vlan.import")));
    expect(host.commandRegistry.list().map((c) => c.id)).toEqual(["vlan.create", "vlan.import"]);
    remove();
    remove();
    expect(host.commandRegistry.list().map((c) => c.id)).toEqual(["vlan.import"]);
    expect(changes).toHaveBeenCalledTimes(3);
    stop();
  });

  it("refuse an identifier already registered", () => {
    removals.push(sdk.commands.register(command("vlan.create")));
    expect(() => sdk.commands.register(command("vlan.create"))).toThrow(/already registered/);
  });
});

describe("shortcuts", () => {
  it("refuse a key of the shell, a key taken, and anything but one character", () => {
    host.reserveKeys(["g", "c", "e", "/", "?"]);
    expect(() => sdk.shortcuts.register(shortcut("vlan.go", "g"))).toThrow(/key of the shell/);
    removals.push(sdk.shortcuts.register(shortcut("vlan.new", "n")));
    expect(() => sdk.shortcuts.register(shortcut("hosts.new", "n"))).toThrow(/taken by vlan\.new/);
    for (const key of ["", " ", "Control+n", "nn"]) {
      expect(() => sdk.shortcuts.register(shortcut(`x.${key}`, key))).toThrow(/one character/);
    }
    removals.push(sdk.shortcuts.register(shortcut("vlan.big", "N")));
    expect(host.shortcutRegistry.list().map((s) => s.key)).toEqual(["n", "N"]);
  });

  it("free their key once removed", () => {
    sdk.shortcuts.register(shortcut("vlan.new", "n"))();
    removals.push(sdk.shortcuts.register(shortcut("hosts.new", "n")));
    expect(host.shortcutRegistry.list().map((s) => s.id)).toEqual(["hosts.new"]);
  });
});
