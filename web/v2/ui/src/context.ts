// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * What the host gives the library once, at the top of its tree: the component that renders
 * a link (the router's, so that a click navigates without reloading), and where floating
 * layers go (the end of `body` by default; the catalogue keeps them inside its `main`, so
 * that the snapshot of the accessibility tree sees them).
 */

import { inject, provide, type Component, type InjectionKey } from "vue";

const LINK: InjectionKey<Component> = Symbol("k-link");
const PORTAL: InjectionKey<string> = Symbol("k-portal");

/** The component of links (receives `to`); without one, a plain `<a href>`. */
export function provideLink(component: Component): void {
  provide(LINK, component);
}

export function injectLink(): Component | null {
  return inject(LINK, null);
}

/** The selector of the element that receives floating layers. */
export function providePortalTarget(selector: string): void {
  provide(PORTAL, selector);
}

export function injectPortalTarget(): string {
  return inject(PORTAL, "body");
}
