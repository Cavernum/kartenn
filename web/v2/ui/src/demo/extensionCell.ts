// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** The tag of the element of the demo extension column (`DataGrid`, state `extension`). */
export const DEMO_CELL = "k-demo-cell";

/**
 * Defines, once, the element of a demo extension column (point `column`, W5): a custom
 * element in the light DOM, as a first-party plugin renders, which writes what it reads of
 * its `row` property (set as a property by the grid, never an attribute).
 */
export function defineDemoCell(): void {
  if (typeof customElements === "undefined" || customElements.get(DEMO_CELL)) return;
  customElements.define(
    DEMO_CELL,
    class extends HTMLElement {
      #row: Record<string, unknown> | undefined;
      set row(value: Record<string, unknown> | undefined) {
        this.#row = value;
        this.#render();
      }
      get row(): Record<string, unknown> | undefined {
        return this.#row;
      }
      connectedCallback(): void {
        this.#render();
      }
      #render(): void {
        const used = parseInt(String(this.#row?.used ?? ""), 10);
        this.textContent = !this.#row ? "" : used >= 50 ? "Sondé" : "Non sondé";
      }
    },
  );
}
