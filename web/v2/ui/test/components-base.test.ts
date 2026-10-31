// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createSSRApp, h, type Component } from "vue";
import { renderToString } from "vue/server-renderer";
import { afterEach, describe, expect, it } from "vitest";
import { copyText } from "../src/clipboard.ts";
import Button from "../src/components/Button/Button.vue";
import { ariaKeyshortcuts } from "../src/components/Button/shortcut.ts";
import Checkbox from "../src/components/Checkbox/Checkbox.vue";
import CodeBadge from "../src/components/CodeBadge/CodeBadge.vue";
import Combobox from "../src/components/Combobox/Combobox.vue";
import { filterOptions } from "../src/components/Combobox/combobox.ts";
import EnvChip from "../src/components/EnvChip/EnvChip.vue";
import { envHue } from "../src/components/EnvChip/env.ts";
import FormField from "../src/components/FormField/FormField.vue";
import IconButton from "../src/components/IconButton/IconButton.vue";
import Kbd from "../src/components/Kbd/Kbd.vue";
import { keyLabel } from "../src/components/Kbd/kbd.ts";
import KeyLabel from "../src/components/KeyLabel/KeyLabel.vue";
import Select from "../src/components/Select/Select.vue";
import ShortcutHint from "../src/components/ShortcutHint/ShortcutHint.vue";
import { shortcutAria, shortcutSteps } from "../src/components/ShortcutHint/shortcutHint.ts";
import Split from "../src/components/Split/Split.vue";
import Stack from "../src/components/Stack/Stack.vue";
import StatusBadge from "../src/components/StatusBadge/StatusBadge.vue";
import Switch from "../src/components/Switch/Switch.vue";
import TextField from "../src/components/TextField/TextField.vue";
import { setLocale } from "../src/i18n.ts";
import { STATES } from "../src/tokens/index.ts";

// The visual version of the base components (spec 020, task 10; REQ-020-16, 17, 21, 22):
// roles, ARIA states and names as rendered on the server.

const render = (component: Component, props: Record<string, unknown> = {}, slot?: string) =>
  renderToString(
    createSSRApp({ render: () => h(component, props, slot ? () => slot : undefined) }),
  );

afterEach(() => setLocale("fr"));

describe("Button", () => {
  it("is a button of type button, with its variant and size", async () => {
    const html = await render(Button, { variant: "primary", size: "sm" }, "Appliquer");
    expect(html).toMatch(/^<button class="k-button" type="button"/);
    expect(html).toContain('data-variant="primary"');
    expect(html).toContain('data-size="sm"');
    expect(html).toContain("Appliquer");
    expect(html).not.toContain("aria-busy");
    expect(html).not.toContain("aria-pressed");
  });

  it("stays focusable while busy: aria-disabled and aria-busy, never disabled", async () => {
    const html = await render(Button, { loading: true }, "Appliquer");
    expect(html).toContain('aria-disabled="true"');
    expect(html).toContain('aria-busy="true"');
    expect(html).not.toMatch(/ disabled[ >]/);
    expect(html).toContain("k-spinner");
    // Said in words too: under reduced motion the indicator stands still.
    expect(html.replace(/<[^>]+>/g, "")).toMatch(/Appliquer\s*\(Chargement\)/);
  });

  it("disabled leaves the tab order", async () => {
    expect(await render(Button, { disabled: true }, "Appliquer")).toMatch(/ disabled[ >]/);
  });

  it("a toggle states aria-pressed, true or false", async () => {
    expect(await render(Button, { pressed: true }, "Filtres")).toContain('aria-pressed="true"');
    expect(await render(Button, { pressed: false }, "Filtres")).toContain('aria-pressed="false"');
  });

  it("shows its shortcut, hidden from the name and given by aria-keyshortcuts", async () => {
    const html = await render(Button, { shortcut: ["Mod", "Enter"] }, "Appliquer");
    expect(html).toContain('aria-keyshortcuts="Control+Enter"');
    expect(html).toMatch(/<kbd class="k-kbd" aria-hidden="true">/);
  });

  it("names shortcuts as WAI-ARIA does", () => {
    expect(ariaKeyshortcuts(["Mod", "k"])).toBe("Control+k");
    expect(ariaKeyshortcuts(["Shift", " "])).toBe("Shift+Space");
  });
});

describe("IconButton", () => {
  it("is named by its label, its icon hidden", async () => {
    const html = await render(IconButton, { icon: "pencil", label: "Modifier" });
    expect(html).toContain('aria-label="Modifier"');
    expect(html).toContain('aria-hidden="true"');
    expect(html).toContain('data-variant="ghost"');
  });

  it("does not compile without a name", () => {
    // @ts-expect-error: label is required (design « Inventaire », line 3)
    const props: InstanceType<typeof IconButton>["$props"] = { icon: "pencil" };
    expect(props).toBeDefined();
  });

  it("gives its shortcut by aria-keyshortcuts; its name tooltip is not a description", async () => {
    const html = await render(IconButton, {
      icon: "panel-left",
      label: "Replier la barre latérale",
      shortcut: ["["],
    });
    expect(html).toContain('aria-keyshortcuts="["');
    expect(html).not.toContain("aria-describedby");
  });

  it("refused, is described by its reason alone", async () => {
    const html = await render(IconButton, {
      icon: "trash",
      label: "Supprimer",
      forbidden: "prefix.delete",
    });
    const described = /aria-describedby="([^"]+)"/.exec(html)![1]!;
    expect(described).toMatch(/-reason$/);
    expect(html).toContain('aria-disabled="true"');
  });
});

describe("ShortcutHint", () => {
  it("joins the steps of a sequence by a word, a combination in one Kbd", async () => {
    const sequence = await render(ShortcutHint, { keys: [["g"], ["p"]] });
    expect(sequence.match(/<kbd class="k-kbd">/g)).toHaveLength(2);
    expect(sequence).toContain('<span class="k-shortcut-hint__then">puis</span>');
    const combination = await render(ShortcutHint, { keys: ["Mod", "k"] });
    expect(combination.match(/<kbd class="k-kbd">/g)).toHaveLength(1);
    expect(combination).not.toContain("k-shortcut-hint__then");
    setLocale("en");
    expect(await render(ShortcutHint, { keys: [["g"], ["p"]] })).toContain(">then</span>");
  });

  it("gives aria-keyshortcuts for a combination only", () => {
    expect(shortcutSteps(["Mod", "k"])).toEqual([["Mod", "k"]]);
    expect(shortcutSteps([["g"], ["p"]])).toEqual([["g"], ["p"]]);
    expect(shortcutAria(["Mod", "k"])).toBe("Control+k");
    expect(shortcutAria([["Control", "Enter"]])).toBe("Control+Enter");
    expect(shortcutAria([["g"], ["p"]])).toBeUndefined();
    expect(shortcutAria([])).toBeUndefined();
  });
});

describe("Kbd", () => {
  it("nests one kbd per key, named in the language", async () => {
    const html = await render(Kbd, { keys: ["Mod", "Enter"] });
    expect(html).toContain('<kbd class="k-kbd__key">Ctrl</kbd>');
    expect(html).toContain('<kbd class="k-kbd__key">Entrée</kbd>');
    setLocale("en");
    expect(keyLabel("Escape")).toBe("Esc");
    expect(keyLabel("ArrowDown")).toBe("↓");
    expect(keyLabel("k")).toBe("k");
    expect(keyLabel("k", true)).toBe("K");
  });

  it("names the other named keys in the language", () => {
    expect(["Shift", "Home", "End", "PageDown", "Delete"].map((k) => keyLabel(k))).toEqual([
      "Maj",
      "Origine",
      "Fin",
      "Pg suiv.",
      "Suppr",
    ]);
    setLocale("en");
    expect(keyLabel("Shift")).toBe("Shift");
    expect(keyLabel("F2")).toBe("F2");
  });
});

describe("TextField", () => {
  it("labels its input and links the error, then the help", async () => {
    const html = await render(TextField, {
      label: "Préfixe",
      error: "Adresse hors de l'espace IPv4.",
      hint: "Notation CIDR.",
      required: true,
      mono: true,
    });
    const id = /<input id="([^"]+)"/.exec(html)![1]!;
    expect(html).toContain(`<label id="${id}-label" for="${id}"`);
    expect(html).toContain('aria-invalid="true"');
    expect(html).toContain(`aria-describedby="${id}-error ${id}-hint"`);
    expect(html).toContain(`<p id="${id}-error"`);
    expect(html).toContain("(obligatoire)");
    expect(html).toContain("k-mono");
  });

  it("without error, is neither invalid nor described by an error", async () => {
    const html = await render(TextField, { label: "Nom" });
    expect(html).not.toContain("aria-invalid");
    expect(html).not.toContain("aria-describedby");
  });

  it("pending, read-only, a hidden label", async () => {
    const html = await render(TextField, {
      label: "Filtrer",
      hideLabel: true,
      pending: true,
      readonly: true,
    });
    expect(html).toContain('aria-busy="true"');
    const pending = /aria-describedby="([^"]+)"/.exec(html)![1]!;
    expect(html).toMatch(new RegExp(`id="${pending}"[^>]*>Chargement<`));
    expect(html).toMatch(/ readonly[ >]/);
    expect(html).toMatch(/<label[^>]*class="[^"]*k-visually-hidden/);
  });

  it("holds an empty live region before any error, the error inside it once given", async () => {
    // A region inserted with its text is not announced: it is there, empty, beforehand.
    expect(await render(TextField, { label: "Nom" })).toMatch(
      /<div class="k-form-field__live" aria-live="polite"><!--[^>]*--><\/div>/,
    );
    const html = await render(TextField, { label: "Nom", error: "Le nom est obligatoire." });
    const id = /<input id="([^"]+)"/.exec(html)![1]!;
    expect(html).toMatch(new RegExp(`aria-live="polite"><p id="${id}-error"`));
  });
});

describe("FormField", () => {
  it("labels the control the slot receives, with its description", async () => {
    const html = await renderToString(
      createSSRApp({
        render: () =>
          h(
            FormField,
            { label: "Description", hint: "Facultative.", error: "Trop longue.", id: "d" },
            {
              default: (slot: { id: string; describedBy?: string; invalid: boolean }) =>
                h("textarea", {
                  id: slot.id,
                  "aria-describedby": slot.describedBy,
                  "aria-invalid": slot.invalid ? "true" : undefined,
                }),
            },
          ),
      }),
    );
    expect(html).toContain('<label id="d-label" for="d"');
    expect(html).toContain(
      '<textarea id="d" aria-describedby="d-error d-hint" aria-invalid="true"',
    );
    expect(html).toContain('<p id="d-hint"');
    expect(html).not.toContain("role=");
  });

  it("group: a named and described group, its label no label element", async () => {
    const html = await render(FormField, {
      label: "Colonnes",
      group: true,
      error: "Choisissez au moins une colonne.",
      id: "g",
    });
    expect(html).toMatch(
      /^<div class="k-form-field" role="group" aria-labelledby="g-label" aria-describedby="g-error">/,
    );
    expect(html).toContain('<span id="g-label" class="k-form-field__label"');
    expect(html).not.toContain("<label");
  });
});

describe("Checkbox", () => {
  it("a native checkbox in its label, the help outside the name", async () => {
    const html = await render(Checkbox, {
      label: "Anycast",
      hint: "Une adresse, plusieurs lieux.",
    });
    expect(html).toMatch(/^<div class="k-checkbox" data-size="md"><label class="k-checkbox__hit">/);
    const id = /<input id="([^"]+)"/.exec(html)![1]!;
    expect(html).toContain('type="checkbox"');
    expect(html).toContain(`aria-describedby="${id}-hint"`);
    expect(html).toMatch(new RegExp(`</label><p id="${id}-hint"`));
    expect(html).not.toMatch(/ checked[ >]/);
    expect(html).not.toContain("k-checkbox__mark");
  });

  it("checked shows the tick; mixed the dash, never aria-checked on the native box", async () => {
    const checked = await render(Checkbox, { label: "Anycast", modelValue: true });
    expect(checked).toMatch(/ checked[ >]/);
    expect(checked).toContain("k-checkbox__mark");
    const mixed = await render(Checkbox, { label: "Tout sélectionner", modelValue: "mixed" });
    expect(mixed).not.toMatch(/ checked[ >]/);
    expect(mixed).not.toContain("aria-checked");
    expect(mixed).toContain("k-checkbox__mark");
    expect(mixed).not.toEqual(checked.replace("Anycast", "Tout sélectionner"));
  });
});

describe("Switch", () => {
  it("a button of role switch, named by its label, its state in aria-checked", async () => {
    const off = await render(Switch, { label: "Montrer les réservées", hint: "Aussitôt." });
    const id = /<button id="([^"]+)"/.exec(off)![1]!;
    expect(off).toContain('type="button" role="switch" aria-checked="false"');
    expect(off).toContain(`aria-labelledby="${id}-label"`);
    expect(off).toContain(`aria-describedby="${id}-hint"`);
    expect(off).toContain(`<span id="${id}-label"`);
    expect(await render(Switch, { label: "Montrer", modelValue: true })).toContain(
      'aria-checked="true"',
    );
  });
});

describe("Select", () => {
  const usages = [
    { value: "host", label: "Hôte" },
    { value: "gateway", label: "Passerelle" },
  ];

  it("a combobox button named by its label, error then help", async () => {
    const html = await render(Select, {
      label: "Usage",
      options: usages,
      modelValue: "gateway",
      error: "Usage refusé.",
      hint: "Liste fermée.",
    });
    const id = /<button[^>]* id="([^"]+)"/.exec(html)![1]!;
    expect(html).toContain('role="combobox"');
    expect(html).toContain(`aria-labelledby="${id}-label"`);
    expect(html).toContain(`aria-describedby="${id}-error ${id}-hint"`);
    expect(html).toContain('aria-invalid="true"');
    expect(html).toContain('aria-expanded="false"');
  });

  it("read-only says so; inline, the label is written in the button, hidden above", async () => {
    const html = await render(Select, {
      label: "Type de flux",
      options: usages,
      modelValue: "host",
      readonly: true,
      inlineLabel: true,
    });
    expect(html).toContain('aria-readonly="true"');
    expect(html).toMatch(/<label[^>]*class="[^"]*k-visually-hidden/);
    expect(html).toMatch(/class="k-select__prefix" aria-hidden="true">Type de flux\u202f:/);
  });
});

describe("Combobox", () => {
  it("an input of role combobox, a list autocomplete, closed, error then help", async () => {
    const html = await render(Combobox, {
      label: "Groupe",
      suggest: async () => [],
      error: "Aucun groupe.",
      hint: "Groupes du site.",
    });
    const id = /<input[^>]* id="([^"]+)"/.exec(html)![1]!;
    expect(html).toContain('role="combobox"');
    expect(html).toContain('aria-autocomplete="list"');
    expect(html).toContain('aria-expanded="false"');
    expect(html).toContain(`aria-describedby="${id}-error ${id}-hint"`);
    expect(html).toContain(`<label id="${id}-label" for="${id}"`);
    // The region that says what the list holds is there, empty, before the list opens.
    expect(html).toMatch(
      /<span class="k-visually-hidden" role="status" aria-live="polite"><\/span>/,
    );
  });

  it("filterOptions finds a label or a detail, whatever the case and the accents", () => {
    const options = [
      { value: "a", label: "Préfixe" },
      { value: "b", label: "campus-ouest", detail: "100–199" },
    ];
    expect(filterOptions(options, "prefixe").map((o) => o.value)).toEqual(["a"]);
    expect(filterOptions(options, " 100 ").map((o) => o.value)).toEqual(["b"]);
    expect(filterOptions(options, "")).toHaveLength(2);
    expect(filterOptions(options, "dmz")).toEqual([]);
  });
});

describe("StatusBadge", () => {
  it.each(STATES)("state %s: its colour, an icon and its label", async (state) => {
    const html = await render(StatusBadge, { state, label: "Libellé" });
    expect(html).toContain(`data-state="${state}"`);
    expect(html).toContain("<svg");
    expect(html).toContain("Libellé");
  });

  it("without a label, the word of its state", async () => {
    expect(await render(StatusBadge, { state: "review" })).toContain("En revue");
    setLocale("en");
    expect(await render(StatusBadge, { state: "neutral" })).toContain("Inactive");
  });
});

describe("KeyLabel and EnvChip (REQ-020-21)", () => {
  /** The text of the button, and the description it points to. */
  const parts = (html: string) => {
    const button = /<button[^>]*>(.*?)<\/button>/s.exec(html)![1]!;
    const described = /aria-describedby="([^"]+)"/.exec(html)![1]!;
    const description = new RegExp(`id="${described}" hidden>([^<]*)<`).exec(html)![1]!;
    return { name: button.replace(/<[^>]+>/g, "").trim(), description };
  };

  it("show the label, else the key in fixed pitch", async () => {
    const labelled = await render(KeyLabel, { value: "dc-nord", label: "Centre nord" });
    expect(labelled).toContain('data-state="labelled"');
    expect(parts(labelled).name).toBe("Centre nord");
    const alone = await render(KeyLabel, { value: "bureaux", label: null });
    expect(alone).toMatch(/<code[^>]*>bureaux<\/code>/);
    const env = await render(EnvChip, { value: "form", hue: 4 });
    expect(env).toMatch(/<code[^>]*>form<\/code>/);
    expect(env).toContain('data-hue="4"');
  });

  it("are buttons that copy the key, said by their description and a status region", async () => {
    const labelled = await render(KeyLabel, { value: "prod", label: "Production" });
    expect(labelled).toMatch(/<button class="k-key-label__text" type="button"/);
    expect(parts(labelled).description).toBe("Clé : prod. Clic, Entrée ou Espace : copier la clé.");
    expect(labelled).toContain('role="status"');
    // Without a label, the key is the name: the description only says how to copy it.
    const alone = await render(KeyLabel, { value: "prod", label: null });
    expect(parts(alone)).toEqual({
      name: "prod",
      description: "Clic, Entrée ou Espace : copier la clé.",
    });
    const env = await render(EnvChip, { value: "prod", label: "Production", hue: 1 });
    expect(parts(env)).toEqual({
      name: "Production",
      description: "Clé : prod. Clic, Entrée ou Espace : copier la clé.",
    });
    setLocale("en");
    expect(parts(await render(KeyLabel, { value: "prod", label: null })).description).toBe(
      "Click, Enter or Space: copy the key.",
    );
  });

  it("plain: the text alone, no control inside a control", async () => {
    for (const component of [KeyLabel, EnvChip]) {
      const html = await render(component, { value: "prod", label: "Production", plain: true });
      expect(html).not.toContain("<button");
      expect(html).not.toContain('role="status"');
      expect(html).toContain("Production");
    }
  });

  it("EnvChip: the hue from the key, stable, in the palette; the host's first", async () => {
    const keys = ["prod", "rece", "deve", "form", "preprod", "dmz", "é", ""];
    for (const key of keys) expect(envHue(key)).toBeGreaterThanOrEqual(1);
    for (const key of keys) expect(envHue(key)).toBeLessThanOrEqual(6);
    expect(keys.map(envHue)).toEqual(keys.map(envHue));
    // FNV-1a of "prod" (0xed3e3174 = 3980276084), modulo 6, plus 1.
    expect(envHue("prod")).toBe(3);
    expect(await render(EnvChip, { value: "prod" })).toContain(`data-hue="${envHue("prod")}"`);
    expect(await render(EnvChip, { value: "prod", hue: 6 })).toContain('data-hue="6"');
  });
});

describe("CodeBadge (REQ-020-22)", () => {
  it("is a button whose name keeps the visible text, with a status region", async () => {
    const html = await render(CodeBadge, { code: "req_7f3a", label: "Request-Id" });
    const text = html
      .replace(/<[^>]+>/g, "")
      .replace(/\s+/g, " ")
      .trim();
    expect(text).toMatch(/^Copier Request-Id req_7f3a/);
    expect(html).not.toContain("aria-label");
    expect(html).toContain('role="status"');
    expect(html).toMatch(/<code[^>]*>req_7f3a<\/code>/);
  });

  it("a refused copy is an outcome, not a throw", async () => {
    expect(await copyText("x")).toBe("failed");
  });
});

describe("layouts", () => {
  it("take a step of the spacing scale", async () => {
    expect(await render(Stack, { gap: "4" })).toContain('data-k-gap="4"');
  });

  it("Split keeps the reading order of what it shows", async () => {
    const html = await renderToString(
      createSSRApp({
        render: () => h(Split, { side: "end" }, { side: () => "Côté", default: () => "Principal" }),
      }),
    );
    expect(html.indexOf("Principal")).toBeLessThan(html.indexOf("Côté"));
  });
});
