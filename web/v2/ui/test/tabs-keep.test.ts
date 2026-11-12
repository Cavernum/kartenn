// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { afterEach, describe, expect, it } from "vitest";
import { createApp, h, nextTick, ref } from "vue";
import Tabs from "../src/components/Tabs/Tabs.vue";

// `keepMounted` (Q-021-22, task 021-12qb; the tabs of `SidePanel` and `BottomPanel`): a
// panel shown once stays mounted, hidden, while another tab is in front, so that a form
// typed in survives a change of tab; never mounted before its tab is first shown.

afterEach(() => {
  document.body.innerHTML = "";
});

/** Reka's presence settles after a few ticks and a frame. */
const settled = () => new Promise((resolve) => setTimeout(resolve, 30));

async function mount(keepMounted: boolean) {
  const host = document.createElement("div");
  document.body.append(host);
  const tab = ref("a");
  const app = createApp({
    render: () =>
      h(
        Tabs,
        {
          label: "Panneau",
          tabs: [
            { value: "a", label: "Détail" },
            { value: "b", label: "Brouillon" },
          ],
          keepMounted,
          modelValue: tab.value,
          "onUpdate:modelValue": (v: string) => (tab.value = v),
        },
        { a: () => h("input", { id: "field-a" }), b: () => h("p", { id: "text-b" }, "B") },
      ),
  });
  app.mount(host);
  await nextTick();
  return { tab, app };
}

describe("Tabs, keepMounted", () => {
  it("keeps a panel shown once, hidden, and mounts the other only when shown", async () => {
    const { tab } = await mount(true);
    const field = document.getElementById("field-a");
    expect(field).not.toBeNull();
    expect(document.getElementById("text-b")).toBeNull();
    tab.value = "b";
    await settled();
    expect(document.getElementById("text-b")).not.toBeNull();
    expect(document.getElementById("field-a")).toBe(field);
    expect(field!.closest("[hidden]")).not.toBeNull();
  });

  it("without it, the panel left goes", async () => {
    const { tab } = await mount(false);
    tab.value = "b";
    await settled();
    expect(document.getElementById("field-a")).toBeNull();
  });
});
