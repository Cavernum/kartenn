// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createApp, h, nextTick, type App, type Component } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import CommitForm from "../src/components/CommitForm/CommitForm.vue";
import Menu from "../src/components/Menu/Menu.vue";
import type { MenuItem } from "../src/components/Menu/menu.ts";
import SplitButton from "../src/components/SplitButton/SplitButton.vue";
import type { SplitAction } from "../src/components/SplitButton/split.ts";
import Toolbar from "../src/components/Toolbar/Toolbar.vue";
import type { ToolbarAction } from "../src/components/Toolbar/toolbar.ts";

// A refusal said by every piece of action (task 020-31; REQ-021-37, 024/M14): a `reason`
// alone refuses an entry of `Menu`, an action of `Toolbar` folded in « … », an action of
// `SplitButton`, as it refuses a `Button`: reachable, `aria-disabled`, described by its
// reason, and never run. `CommitForm` read only stays in the tab order and says why.
// Mounted: the menus open at the keyboard, in the document.

const HELD = "Une politique d'approbation retient ce changeset.";

const apps: App[] = [];
afterEach(() => {
  for (const app of apps.splice(0)) app.unmount();
  document.body.innerHTML = "";
  vi.restoreAllMocks();
});

async function mount(component: Component, props: Record<string, unknown>): Promise<HTMLElement> {
  const root = document.body.appendChild(document.createElement("div"));
  const app = createApp({ render: () => h(component, props) });
  apps.push(app);
  app.mount(root);
  await nextTick();
  return root;
}

/** The entries of the menu its button opens, at the keyboard (Down). */
async function openMenu(trigger: HTMLElement, count: number): Promise<HTMLElement[]> {
  trigger.focus();
  trigger.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
  await expect.poll(() => document.querySelectorAll("[role='menuitem']").length).toBe(count);
  return [...document.querySelectorAll<HTMLElement>("[role='menuitem']")];
}
const entry = (entries: HTMLElement[], name: string) =>
  entries.find((e) => e.textContent?.includes(name))!;
/** What an element's `aria-describedby` says. */
const description = (el: Element) =>
  (el.getAttribute("aria-describedby") ?? "")
    .split(" ")
    .filter(Boolean)
    .map((id) => document.getElementById(id)?.textContent?.trim() ?? "")
    .join(" ");

describe("Menu: a reason alone refuses an entry", () => {
  it("reachable, aria-disabled, described by its reason, never chosen", async () => {
    const select = vi.fn();
    const items: MenuItem[] = [
      { id: "edit", label: "Modifier" },
      { id: "move", label: "Déplacer vers…", reason: "Parent déduit de l'inclusion" },
    ];
    const root = await mount(Menu, { label: "Actions", items, onSelect: select });
    const entries = await openMenu(root.querySelector("button")!, 2);
    const move = entry(entries, "Déplacer");
    expect(move.getAttribute("aria-disabled")).toBe("true");
    expect(move.hasAttribute("data-disabled")).toBe(false);
    expect(description(move)).toBe("Parent déduit de l'inclusion");
    move.click();
    await nextTick();
    expect(select).not.toHaveBeenCalled();
    // The allowed entry is neither refused nor described.
    const edit = entry(entries, "Modifier");
    expect(edit.hasAttribute("aria-disabled")).toBe(false);
    expect(edit.hasAttribute("aria-describedby")).toBe(false);
    edit.click();
    expect(select).toHaveBeenCalledWith("edit");
  });
});

describe("Toolbar: an action folded in « … » keeps its refusal and its reason", () => {
  it("in the menu, aria-disabled, described by its reason, never run", async () => {
    // A bar of 150 px, actions of 100 px: under a container xs, the main action alone stays.
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(150);
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
      width: 100,
    } as DOMRect);
    const action = vi.fn();
    const actions: ToolbarAction[] = [
      { id: "validation", label: "Valider", variant: "primary", priority: 3 },
      { id: "rejection", label: "Rejeter", priority: 2, reason: HELD },
      { id: "withdrawal", label: "Retirer", priority: 2 },
    ];
    const root = await mount(Toolbar, { label: "Décisions", actions, onAction: action });
    await expect.poll(() => root.querySelectorAll("[data-action]").length).toBe(1);
    const entries = await openMenu(root.querySelector("[aria-haspopup='menu']")!, 2);
    const rejection = entry(entries, "Rejeter");
    expect(rejection.getAttribute("aria-disabled")).toBe("true");
    expect(description(rejection)).toBe(HELD);
    rejection.click();
    await nextTick();
    expect(action).not.toHaveBeenCalled();
    entry(entries, "Retirer").click();
    expect(action).toHaveBeenCalledWith("withdrawal");
  });
});

describe("SplitButton: a reason alone refuses an action", () => {
  const actions: SplitAction[] = [
    { id: "apply", label: "Appliquer", reason: HELD },
    { id: "review", label: "Soumettre en revue" },
  ];

  it("chosen, the button is refused with its reason and runs nothing", async () => {
    const run = vi.fn();
    const root = await mount(SplitButton, { actions, action: "apply", onRun: run });
    const main = root.querySelector<HTMLButtonElement>(".k-split-button__main")!;
    expect(main.disabled).toBe(false);
    expect(main.getAttribute("aria-disabled")).toBe("true");
    expect(description(main)).toBe(HELD);
    main.click();
    expect(run).not.toHaveBeenCalled();
  });

  it("in the menu, aria-disabled with its reason, never chosen", async () => {
    const choose = vi.fn();
    const root = await mount(SplitButton, {
      actions,
      action: "review",
      "onUpdate:action": choose,
    });
    const entries = await openMenu(root.querySelector("[aria-haspopup='menu']")!, 2);
    const apply = entry(entries, "Appliquer");
    expect(apply.getAttribute("aria-disabled")).toBe("true");
    expect(description(apply)).toBe(HELD);
    apply.click();
    await nextTick();
    expect(choose).not.toHaveBeenCalled();
  });

  it("disabled without a reason, chosen: the button is disabled", async () => {
    const run = vi.fn();
    const root = await mount(SplitButton, {
      actions: [{ id: "apply", label: "Appliquer", disabled: true }],
      onRun: run,
    });
    const main = root.querySelector<HTMLButtonElement>(".k-split-button__main")!;
    expect(main.disabled).toBe(true);
    main.click();
    expect(run).not.toHaveBeenCalled();
  });
});

describe("CommitForm read only (024/M14)", () => {
  const PAST = "Lecture d'une date passée : rien ne s'écrit.";
  const ctrlEnter = (root: HTMLElement) =>
    root
      .querySelector("input")!
      .dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", ctrlKey: true, bubbles: true }));

  it("both fields read only, in the tab order, each described by the reason", async () => {
    const submit = vi.fn();
    const root = await mount(CommitForm, {
      required: true,
      readonly: true,
      reason: PAST,
      summary: "Maintenance",
      description: "Les VIP suivent.",
      onSubmit: submit,
    });
    const input = root.querySelector("input")!;
    const textarea = root.querySelector("textarea")!;
    for (const field of [input, textarea]) {
      expect(field.readOnly).toBe(true);
      expect(field.disabled).toBe(false);
      expect(field.tabIndex).not.toBe(-1);
      expect(description(field)).toBe(PAST);
    }
    // Shown once, under the summary; the description reads it from a hidden copy.
    const shown = [...root.querySelectorAll(".k-form-field__hint")].map((p) => p.textContent);
    expect(shown).toEqual([PAST]);
    ctrlEnter(root);
    expect(submit).not.toHaveBeenCalled();
  });

  it("a reason alone makes it read only; « Ajouter une description » refused with it", async () => {
    const root = await mount(CommitForm, { collapsible: true, reason: PAST });
    expect(root.querySelector("input")!.readOnly).toBe(true);
    const add = [...root.querySelectorAll("button")].find((b) =>
      b.textContent?.includes("Ajouter une description"),
    )!;
    expect(add.disabled).toBe(false);
    expect(add.getAttribute("aria-disabled")).toBe("true");
    expect(description(add)).toBe(PAST);
    add.click();
    await nextTick();
    expect(root.querySelector("textarea")).toBeNull();
  });

  it("read only without a reason: no description, the button out of reach", async () => {
    const root = await mount(CommitForm, { collapsible: true, readonly: true });
    const input = root.querySelector("input")!;
    expect(input.readOnly).toBe(true);
    expect(input.hasAttribute("aria-describedby")).toBe(false);
    const add = root.querySelector("button")!;
    expect(add.disabled).toBe(true);
  });

  it("writable, it submits", async () => {
    const submit = vi.fn();
    const root = await mount(CommitForm, { summary: "Maintenance", onSubmit: submit });
    expect(root.querySelector("input")!.readOnly).toBe(false);
    ctrlEnter(root);
    expect(submit).toHaveBeenCalledOnce();
  });
});
