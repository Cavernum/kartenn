// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Example plugin on the SDK 1.0 (spec 021, task 19), for now without any extension: the
// first package of web/v2/plugins, built and served by the test instance of the new shell
// (web/v2/e2e/kartenn.toml). Its elements come with task 19.

import { definePlugin } from "@kartenn/sdk";

definePlugin({ name: "example", elements: {} });
