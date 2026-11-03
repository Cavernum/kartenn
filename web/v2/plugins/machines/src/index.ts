// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Plugin of the machines and interfaces module on the SDK 1.0 (spec 021, tasks 18a to 18cd;
// spec 009, REQ-009-20). Under the space `hosts`: the lists and records of the machines,
// clusters and interfaces; on the screens of the core, the « Interfaces » tab of an address,
// a network and a VLAN, the machine column of the addresses, and the action that attaches an
// address to an interface. In Vue 3 and @kartenn/ui, shared with the shell (A19, A23): each
// element is an application of `vue` mounted in its light DOM by the thin host (`host.ts`;
// tasks 18ca to 18cd), without a sheet of its own. Reads go through the shared cache; writes
// are operations added to the draft of the shell (`draft.add`, `draft.move`), offered at the
// present only (W9).

import AttachAction from "./AttachAction.vue";
import ClustersView from "./ClustersView.vue";
import { defined } from "./dom.ts";
import { pluginHost } from "./host.ts";
import InterfacesTab from "./InterfacesTab.vue";
import InterfacesView from "./InterfacesView.vue";
import MachineColumn from "./MachineColumn.vue";
import MachinesView from "./MachinesView.vue";

const host = pluginHost();

defined.handle = host.define({
  name: "machines",
  elements: {
    "machines-machines": host.element(MachinesView),
    "machines-clusters": host.element(ClustersView),
    "machines-interfaces": host.element(InterfacesView),
    "machines-tab": host.element(InterfacesTab),
    "machines-column": host.element(MachineColumn),
    "machines-attach": host.element(AttachAction),
  },
});
