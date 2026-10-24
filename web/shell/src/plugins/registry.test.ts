import { describe, expect, it } from "vitest";
import { NO_RIGHTS, Rights } from "../rights/permissions.ts";
import { extensionsOf, pluginKey, type Loaded, type UiPlugin } from "./registry.ts";

const notes: UiPlugin = {
  name: "notes",
  version: "1.0.0",
  sdk: "^0.1",
  base: "/plugins/notes/1.0.0/",
  entry: "/plugins/notes/1.0.0/index.js",
  integrity: {},
  catalogs: { en: "i18n/en.json" },
  extensions: [
    { point: "view", id: "overview", element: "notes-overview", nav: "nav.overview" },
    { point: "tab", id: "notes", element: "notes-tab", object_types: ["prefix"], label: "tab" },
    {
      point: "action",
      id: "reserve",
      element: "notes-reserve",
      object_types: ["prefix"],
      label: "action",
      permission: "core.address.edit",
    },
  ],
};

const loaded: Loaded[] = [{ plugin: notes, status: { state: "loaded" } }];

describe("extensions of the loaded plugins (REQ-007-13)", () => {
  it("are filtered by the type of the object", () => {
    expect(extensionsOf(loaded, "tab", NO_RIGHTS, "prefix").map((e) => e.id)).toEqual(["notes"]);
    expect(extensionsOf(loaded, "tab", NO_RIGHTS, "address")).toEqual([]);
  });

  it("are hidden without their permission", () => {
    expect(extensionsOf(loaded, "action", NO_RIGHTS, "prefix")).toEqual([]);
    const writer = new Rights(["core.*.edit"]);
    expect(extensionsOf(loaded, "action", writer, "prefix").map((e) => e.id)).toEqual(["reserve"]);
  });

  it("come only from plugins that loaded", () => {
    for (const status of [
      { state: "loading" },
      { state: "rejected", reason: "timeout" },
    ] as const) {
      expect(extensionsOf([{ plugin: notes, status }], "view", NO_RIGHTS)).toEqual([]);
    }
    expect(extensionsOf(loaded, "view", NO_RIGHTS)).toEqual([
      expect.objectContaining({ plugin: "notes", element: "notes-overview" }),
    ]);
  });

  it("have their labels under the namespace of the plugin", () => {
    const [view] = extensionsOf(loaded, "view", NO_RIGHTS);
    expect(pluginKey(view!, view!.nav)).toBe("notes.nav.overview");
  });
});
