// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { afterEach, describe, expect, it } from "vitest";
import { nextTick, ref } from "vue";
import { ownsEscape, usePanelClose } from "../src/components/internal/panel.ts";

// Whose is an Escape heard by a panel (`SidePanel`, `BottomPanel`; tasks 021-12p,
// 021-12qa): an open popup's, a composition's; a field's that is typed in only while the
// panel is not guarded (its host does not say `modified`: nothing would ask before its
// input went, Q-021-22); a closed combobox and the rest let it go to the panel, as in a
// dialog. The panels' keyboard is played in the browser (e2e/components/side-panel.spec.ts,
// bottom-panel.spec.ts, e2e/visual/clavier.spec.ts).

/** What `ownsEscape` says of an Escape pressed on `el` (its target, as the panel hears it). */
function owns(el: Element, init: KeyboardEventInit = {}, guarded = false): boolean {
  let said: boolean | undefined;
  document.body.addEventListener("keydown", (event) => (said = ownsEscape(event, guarded)), {
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

  const FIELDS = [
    '<input type="text" value="Serveurs">',
    '<input type="search" value="192.0.2.0">',
    "<input>",
    "<textarea>Maintenance</textarea>",
    "<select><option>A</option></select>",
    '<span role="spinbutton" contenteditable="true">2026</span>',
  ];

  it("unguarded, a field typed in keeps it: text, search, text area, select, segment", () => {
    for (const html of FIELDS) expect(owns(make(html)), html).toBe(true);
  });

  it("guarded (the panel asks when modified), a field lets it go", () => {
    for (const html of FIELDS) expect(owns(make(html), {}, true), html).toBe(false);
  });

  it("guarded, an open popup and a composition still keep it", () => {
    expect(owns(make('<input role="combobox" aria-expanded="true">'), {}, true)).toBe(true);
    const menu = '<button aria-haspopup="menu" aria-expanded="true"></button>';
    expect(owns(make(menu), {}, true)).toBe(true);
    expect(owns(make('<input type="text">'), { isComposing: true }, true)).toBe(true);
    expect(owns(make("<textarea></textarea>"), { keyCode: 229 }, true)).toBe(true);
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
    const closed = '<input role="combobox" aria-expanded="false">';
    expect(owns(make(closed), { isComposing: true })).toBe(true);
    expect(owns(make(closed), { keyCode: 229 })).toBe(true);
  });

  it("an Escape without an element for target is the panel's", () => {
    expect(ownsEscape(new KeyboardEvent("keydown", { key: "Escape" }))).toBe(false);
  });
});

describe("usePanelClose (SidePanel, BottomPanel)", () => {
  /** A panel holding a field, open, an Escape pressed in the field, and what its host heard. */
  function setup(modified: boolean | undefined) {
    const el = make('<aside><input type="text" value="Supervision"></aside>');
    const panel = ref<HTMLElement | null>(el);
    const open = ref(true);
    let closed = 0;
    const close = usePanelClose({ panel, open, modified: () => modified, closed: () => closed++ });
    const field = el.querySelector("input")!;
    field.focus();
    const escape = new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true });
    el.addEventListener("keydown", close.onKeydown);
    field.dispatchEvent(escape);
    return { close, open, escape, closed: () => closed, el };
  }

  it("intact, Escape from a field closes at once", () => {
    const { close, open, escape, closed } = setup(false);
    expect(escape.defaultPrevented).toBe(true);
    expect(close.asking.value).toBe(false);
    expect(open.value).toBe(false);
    expect(closed()).toBe(1);
  });

  it("modified, Escape asks first; « Abandonner » closes it, the leaving panel inert", () => {
    const { close, open, closed, el } = setup(true);
    expect(close.asking.value).toBe(true);
    expect(open.value).toBe(true);
    expect(closed()).toBe(0);
    close.discard();
    expect(open.value).toBe(false);
    expect(closed()).toBe(1);
    expect(el.hasAttribute("inert")).toBe(true);
  });

  it("not said, the field keeps Escape: nothing closes, nothing asks", () => {
    const { close, open, escape } = setup(undefined);
    expect(escape.defaultPrevented).toBe(false);
    expect(close.asking.value).toBe(false);
    expect(open.value).toBe(true);
  });

  it("closed by the host's model, a question in progress goes, unsaid, the panel inert", async () => {
    const { close, open, closed, el } = setup(true);
    open.value = false;
    await nextTick();
    expect(close.asking.value).toBe(false);
    expect(closed()).toBe(0);
    expect(el.hasAttribute("inert")).toBe(true);
  });

  it("the repeats of a held Escape are not the panel's", () => {
    const el = make('<aside><button type="button">Fermer</button></aside>');
    const open = ref(true);
    const close = usePanelClose({
      panel: ref(el),
      open,
      modified: () => false,
      closed: () => {},
    });
    const repeat = { key: "Escape", repeat: true, bubbles: true, cancelable: true };
    el.addEventListener("keydown", close.onKeydown);
    el.querySelector("button")!.dispatchEvent(new KeyboardEvent("keydown", repeat));
    expect(open.value).toBe(true);
  });
});
