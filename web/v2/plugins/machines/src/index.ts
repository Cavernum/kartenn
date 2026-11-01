// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Plugin of the machines and interfaces module on the SDK 1.0 (spec 021, task 18a; spec
// 009, REQ-009-20): Custom Elements without a framework, written with the public SDK only,
// like the example plugin. Under the space `hosts`: the lists and records of the machines,
// clusters and interfaces; on the screens of the core, the « Interfaces » tab of an address,
// a network and a VLAN, the machine column of the addresses, and the action that attaches an
// address to an interface. Each element draws in its own shadow root with the sheet of
// @kartenn/ui then its own; it reads through the shared cache, and its writes are operations
// added to the draft of the shell (`draft.add`), offered at the present only (W9).

import { definePlugin } from "@kartenn/sdk";
import { defined } from "./dom.ts";
import { AttachAction, InterfacesTab, MachineColumn } from "./extensions.ts";
import { ClustersView, InterfacesView, MachinesView } from "./views.ts";

defined.handle = definePlugin({
  name: "machines",
  elements: {
    "machines-machines": MachinesView,
    "machines-clusters": ClustersView,
    "machines-interfaces": InterfacesView,
    "machines-tab": InterfacesTab,
    "machines-column": MachineColumn,
    "machines-attach": AttachAction,
  },
});
