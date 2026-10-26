// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createSSRApp, h, type Component } from "vue";
import { renderToString } from "vue/server-renderer";
import { afterEach, describe, expect, it } from "vitest";
import { copyText } from "../src/clipboard.ts";
import Button from "../src/components/Button/Button.vue";
import { ariaKeyshortcuts } from "../src/components/Button/shortcut.ts";
import CodeBadge from "../src/components/CodeBadge/CodeBadge.vue";
import EnvChip from "../src/components/EnvChip/EnvChip.vue";
import IconButton from "../src/components/IconButton/IconButton.vue";
import Kbd from "../src/components/Kbd/Kbd.vue";
import { keyLabel } from "../src/components/Kbd/kbd.ts";
import KeyLabel from "../src/components/KeyLabel/KeyLabel.vue";
import Split from "../src/components/Split/Split.vue";
import Stack from "../src/components/Stack/Stack.vue";
import StatusBadge from "../src/components/StatusBadge/StatusBadge.vue";
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
    expect(html).toContain(`<label for="${id}"`);
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
});

describe("StatusBadge", () => {
  it.each(STATES)("state %s: its colour, an icon and its label", async (state) => {
    const html = await render(StatusBadge, { state, label: "Libellé" });
    expect(html).toContain(`data-state="${state}"`);
    expect(html).toContain("<svg");
    expect(html).toContain("Libellé");
  });
});

describe("KeyLabel and EnvChip (REQ-020-21)", () => {
  it("show the label, else the key in fixed pitch", async () => {
    const labelled = await render(KeyLabel, { value: "dc-nord", label: "Centre nord" });
    expect(labelled).toContain('data-state="labelled"');
    expect(labelled).not.toContain("dc-nord");
    const alone = await render(KeyLabel, { value: "bureaux", label: null });
    expect(alone).toMatch(/<code[^>]*>bureaux<\/code>/);
    const env = await render(EnvChip, { value: "form", hue: 4 });
    expect(env).toMatch(/<code[^>]*>form<\/code>/);
    expect(env).toContain('data-hue="4"');
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
