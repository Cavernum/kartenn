import { defineAsyncComponent } from "vue";
import { worded } from "../views/words.ts";

// The panels the frame opens itself in its right panel, each in a chunk of its own read
// when first asked for, with the words of the screens (task 22n): the draft tab opens the
// edit panel on any page, a screen read or not.

/** Their loaders, which `test/words.test.ts` checks await the words of the screens. */
export const PANEL_LOADERS = {
  // The edit panel of a kind (task 12, REQ-021-36): asked by a view, shown in the Détail tab.
  edit: worded(() => import("../views/edit/EditPanel.vue")),
  // « Clore… » of a record (021/A31, task 11g): the closure in the same place.
  close: worded(() => import("../views/edit/ClosePanel.vue")),
  // The draft tab (task 14, REQ-021-6, 11): its operations and its foot.
  draftOperations: worded(() => import("../draft/DraftOperations.vue")),
  draftSubmit: worded(() => import("../draft/DraftSubmit.vue")),
};

export const EditPanel = defineAsyncComponent(PANEL_LOADERS.edit);
export const ClosePanel = defineAsyncComponent(PANEL_LOADERS.close);
export const DraftOperations = defineAsyncComponent(PANEL_LOADERS.draftOperations);
export const DraftSubmit = defineAsyncComponent(PANEL_LOADERS.draftSubmit);
