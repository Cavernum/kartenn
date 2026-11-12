// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { afterEach, describe, expect, it } from "vitest";
import { ownsEscape } from "../src/components/internal/panel.ts";

// Whose is an Escape heard by a panel (`SidePanel`, `BottomPanel`; task 021-12p): an open
// popup's, or a composition's; a closed combobox, a text field, a text area and a segment
// of a date let it go to the panel, as in a dialog. The panels' keyboard is played in the
// browser (e2e/components/side-panel.spec.ts, e2e/visual/clavier.spec.ts).

/** What `ownsEscape` says of an Escape pressed on `el` (its target, as the panel hears it). */
function owns(el: Element, init: KeyboardEventInit = {}): boolean {
  let said: boolean | undefined;
  document.body.addEventListener("keydown", (event) => (said = ownsEscape(event)), { once: true });
  el.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, ...init }));
  return said!;
}

function make(html: string): HTMLElement {
  document.body.innerHTML = html;
  return document.body.firstElementChild as HTMLElement;
}

afterEach(() => {
  document.body.innerHTML = "";
});

describe("ownsEscape (SidePanel, BottomPanel)", () => {
  it("an open combobox keeps it: its first Escape closes its list", () => {
    expect(owns(make('<input role="combobox" aria-expanded="true">'))).toBe(true);
  });

  it("a closed combobox lets it go: its second Escape is the panel's", () => {
    expect(owns(make('<input role="combobox" aria-expanded="false">'))).toBe(false);
    expect(owns(make('<button role="combobox" aria-expanded="false"></button>'))).toBe(false);
  });

  it("a menu button keeps it while its menu is open, not after", () => {
    expect(owns(make('<button aria-haspopup="menu" aria-expanded="true"></button>'))).toBe(true);
    expect(owns(make('<button aria-haspopup="menu" aria-expanded="false"></button>'))).toBe(false);
  });

  it("a disclosure, expanded, lets it go: its content is in place, not a popup", () => {
    expect(owns(make('<button aria-expanded="true"></button>'))).toBe(false);
    expect(owns(make('<button aria-haspopup="false" aria-expanded="true"></button>'))).toBe(false);
  });

  it("a text field, a search, a text area, a select and a segment let it go", () => {
    for (const html of [
      '<input type="text" value="Serveurs">',
      '<input type="search" value="192.0.2.0">',
      "<input>",
      "<textarea>Maintenance</textarea>",
      "<select><option>A</option></select>",
      '<span role="spinbutton" contenteditable="true">2026</span>',
      "<button>Fermer</button>",
    ])
      expect(owns(make(html)), html).toBe(false);
  });

  it("a composition in progress keeps it, whatever the field", () => {
    expect(owns(make('<input type="text">'), { isComposing: true })).toBe(true);
    expect(owns(make('<input role="combobox" aria-expanded="false">'), { isComposing: true })).toBe(
      true,
    );
  });

  it("an Escape without an element for target is the panel's", () => {
    expect(ownsEscape(new KeyboardEvent("keydown", { key: "Escape" }))).toBe(false);
  });
});
