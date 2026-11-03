// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createApp, h, nextTick, ref, type App } from "vue";
import { afterEach, describe, expect, it } from "vitest";
import { addressFilters } from "../src/components/FilterPicker/FilterPicker.catalogue.ts";
import FilterPicker from "../src/components/FilterPicker/FilterPicker.vue";
import type { Filter } from "../src/components/FilterPicker/filters.ts";

// The focus on « Ajouter un filtre » from the host (task 021-11h): the root of the picker
// is renderless (the popover's), its `$el` no element to look the bar up from; the method
// it exposes, `focusAdd`, does it.

const apps: App[] = [];
afterEach(() => {
  for (const app of apps.splice(0)) app.unmount();
  document.body.innerHTML = "";
});

describe("FilterPicker: focusAdd (task 021-11h)", () => {
  it("puts the focus on « Ajouter un filtre », with chips or without, the popover closed", async () => {
    const picker = ref<InstanceType<typeof FilterPicker> | null>(null);
    const filters = ref<Filter[]>([{ key: "usage", operator: "eq", values: ["host"] }]);
    const root = document.body.appendChild(document.createElement("div"));
    const app = createApp({
      render: () =>
        h(FilterPicker, {
          ref: picker,
          model: addressFilters,
          modelValue: filters.value,
          "onUpdate:modelValue": (next: Filter[]) => (filters.value = next),
        }),
    });
    apps.push(app);
    app.mount(root);
    await nextTick();
    const add = () => document.querySelector<HTMLElement>(".k-filter-picker__add");
    // The node Vue gives as `$el` is no element: nothing to query from it.
    expect(picker.value!.$el instanceof Element).toBe(false);
    expect(document.querySelector(".k-filter-chip")).not.toBeNull();
    picker.value!.focusAdd();
    expect(document.activeElement).toBe(add());
    // The filters taken away by the host, the bar without chips: the same button.
    (document.activeElement as HTMLElement).blur();
    filters.value = [];
    await nextTick();
    expect(document.querySelector(".k-filter-chip")).toBeNull();
    picker.value!.focusAdd();
    expect(document.activeElement).toBe(add());
    expect(add()!.getAttribute("aria-expanded")).toBe("false");
  });
});
