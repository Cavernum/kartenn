// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { afterEach, describe, expect, it } from "vitest";
import { nextTick, ref } from "vue";
import { ownsEscape, usePanelClose } from "../src/components/internal/panel.ts";

// Whose is an Escape heard by a panel (`SidePanel`, `BottomPanel`; tasks 021-12p,
// 021-12qa): an open popup's, a composition's; anything else lets it go to the panel, a
// field typed in too, since a modified panel asks before it closes (Q-021-22). The panels'
// keyboard is played in the browser (e2e/components/side-panel.spec.ts,
// bottom-panel.spec.ts, e2e/visual/clavier.spec.ts).

/** What `ownsEscape` says of an Escape pressed on `el` (its target, as the panel hears it). */
function owns(el: Element, init: KeyboardEventInit = {}): boolean {
  let said: boolean | undefined;
  document.body.addEventListener("keydown", (event) => (said = ownsEscape(event)), {
    once: true,
  });
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

  it("a field typed in lets it go: text, search, text area, select, segment of a date", () => {
    for (const html of [
      '<input type="text" value="Serveurs">',
      '<input type="search" value="192.0.2.0">',
      "<input>",
      "<textarea>Maintenance</textarea>",
      "<select><option>A</option></select>",
      '<span role="spinbutton" contenteditable="true">2026</span>',
    ])
      expect(owns(make(html)), html).toBe(false);
  });

  it("a button, a check box, a closed select-only combobox let it go", () => {
    for (const html of [
      "<button>Fermer</button>",
      '<input type="checkbox">',
      '<button role="combobox" aria-expanded="false">Nature</button>',
    ])
      expect(owns(make(html)), html).toBe(false);
  });

  it("a composition in progress keeps it, whatever the field, also once WebKit ended it", () => {
    expect(owns(make('<input type="text">'), { isComposing: true })).toBe(true);
    expect(owns(make("<textarea></textarea>"), { keyCode: 229 })).toBe(true);
    const closed = '<input role="combobox" aria-expanded="false">';
    expect(owns(make(closed), { isComposing: true })).toBe(true);
    expect(owns(make(closed), { keyCode: 229 })).toBe(true);
  });

  it("an Escape without an element for target is the panel's", () => {
    expect(ownsEscape(new KeyboardEvent("keydown", { key: "Escape" }))).toBe(false);
  });
});

describe("usePanelClose (SidePanel, BottomPanel)", () => {
  const settled = () => new Promise((resolve) => setTimeout(resolve));

  /**
   * An opener, then a panel holding a field, opened from it; `modified` as the host says.
   * `escape()` presses Escape in the field (`repeat`: a held key).
   */
  function setup(changed: boolean) {
    document.body.innerHTML =
      '<button id="opener">Ouvrir</button><aside><input type="text" value="Supervision"></aside>';
    const opener = document.getElementById("opener")!;
    const el = document.querySelector("aside")!;
    const field = el.querySelector("input")!;
    opener.focus();
    const modified = ref(changed);
    const open = ref(true);
    let closed = 0;
    const close = usePanelClose({
      panel: ref<HTMLElement | null>(el),
      open,
      modified: () => modified.value,
      closed: () => closed++,
    });
    el.addEventListener("keydown", close.onKeydown);
    field.focus();
    const escape = (repeat = false) => {
      const event = new KeyboardEvent("keydown", {
        key: "Escape",
        repeat,
        bubbles: true,
        cancelable: true,
      });
      field.dispatchEvent(event);
      return event;
    };
    return { close, open, modified, escape, closed: () => closed, el, opener };
  }

  it("intact, Escape from a field closes at once, the focus to the opener", async () => {
    const { close, open, escape, closed, opener } = setup(false);
    expect(escape().defaultPrevented).toBe(true);
    expect(close.asking.value).toBe(false);
    expect(open.value).toBe(false);
    expect(closed()).toBe(1);
    await nextTick();
    expect(document.activeElement).toBe(opener);
  });

  it("modified, Escape asks; « Abandonner » closes, the leaving panel inert, focus to the opener", async () => {
    const { close, open, escape, closed, el, opener } = setup(true);
    escape();
    expect(close.asking.value).toBe(true);
    expect(open.value).toBe(true);
    expect(closed()).toBe(0);
    close.answer(true);
    await settled();
    expect(close.asking.value).toBe(false);
    expect(open.value).toBe(false);
    expect(closed()).toBe(1);
    expect(el.hasAttribute("inert")).toBe(true);
    expect(document.activeElement).toBe(opener);
  });

  it("modified, « Continuer la saisie » leaves it open", async () => {
    const { close, open, escape, closed } = setup(true);
    escape();
    close.answer(false);
    await settled();
    expect(close.asking.value).toBe(false);
    expect(open.value).toBe(true);
    expect(closed()).toBe(0);
  });

  it("modified, the repeats of a held Escape neither ask again nor answer", () => {
    const { close, open, escape } = setup(true);
    escape();
    expect(escape(true).defaultPrevented).toBe(false);
    expect(close.asking.value).toBe(true);
    expect(open.value).toBe(true);
  });

  it("closed by the host's model during the question: withdrawn, focus to the opener", async () => {
    const { close, open, escape, closed, el, opener } = setup(true);
    escape();
    const asked = close.ask();
    open.value = false;
    await nextTick();
    await nextTick();
    expect(close.asking.value).toBe(false);
    expect(await asked).toBe(false);
    expect(closed()).toBe(0);
    expect(el.hasAttribute("inert")).toBe(true);
    expect(document.activeElement).toBe(opener);
  });

  it("`modified` turned false during the question withdraws it, the panel open", async () => {
    const { close, open, modified, escape } = setup(true);
    escape();
    const asked = close.ask();
    modified.value = false;
    await nextTick();
    expect(close.asking.value).toBe(false);
    expect(await asked).toBe(false);
    expect(open.value).toBe(true);
  });

  it("`ask()` (a host's guard) asks without closing; intact, says yes at once", async () => {
    const intact = setup(false);
    expect(await intact.close.ask()).toBe(true);
    const { close, open } = setup(true);
    const asked = close.ask();
    expect(close.asking.value).toBe(true);
    expect(close.ask()).toBe(asked);
    close.answer(true);
    expect(await asked).toBe(true);
    expect(open.value).toBe(true);
  });
});
