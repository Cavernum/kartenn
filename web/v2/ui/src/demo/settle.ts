// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref, type Component, type VNode } from "vue";
import Button from "../components/Button/Button.vue";
import Stack from "../components/Stack/Stack.vue";

/**
 * A data view of the catalogue that starts loading and receives its data on demand
 * (REQ-020-23): the button switches the view between its skeleton and its data, and a
 * sentence after the view shows that nothing after it moves (no layout shift; measured by
 * e2e/components/skeleton.spec.ts).
 */
export function settleDemo(view: (loading: boolean) => VNode): Component {
  return defineComponent({
    setup() {
      const loading = ref(true);
      return () =>
        h(Stack, { gap: "2", align: "stretch" }, () => [
          h("span", {}, [
            h(Button, { size: "sm", onClick: () => (loading.value = !loading.value) }, () =>
              loading.value ? "Recevoir les données" : "Recharger",
            ),
          ]),
          view(loading.value),
          h("p", { class: "k-demo-after" }, "Ce qui suit la zone reste à sa place."),
        ]);
    },
  });
}
