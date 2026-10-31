// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import { describe, expect, it } from "vitest";
import ToastRegion from "../src/components/Toast/ToastRegion.vue";
import { TOAST_DURATION, createToasts } from "../src/components/Toast/toasts.ts";

// The toasts (spec 020, task 22; design « Inventaire », line 14): the store, and the two
// live regions of the region as rendered on the server.

describe("createToasts", () => {
  it("settles the tone and the duration; an error, a warning, an action have none", () => {
    const toasts = createToasts();
    const info = toasts.show({ title: "Importé." });
    const held = toasts.show({ tone: "ok", title: "Ajouté.", duration: Infinity });
    const error = toasts.show({ tone: "ko", title: "Échec.", duration: 1000 });
    toasts.show({ tone: "warn", title: "Proche du seuil.", duration: 1000 });
    toasts.show({ tone: "ok", title: "Proposé.", action: { label: "Voir", run: () => {} } });
    expect(new Set([info, held, error]).size).toBe(3);
    expect(toasts.list.map(({ tone, duration }) => [tone, duration])).toEqual([
      ["info", TOAST_DURATION],
      ["ok", Infinity],
      ["ko", Infinity],
      ["warn", Infinity],
      ["ok", Infinity],
    ]);
  });

  it("dismisses by id, the others kept in their order", () => {
    const toasts = createToasts();
    const first = toasts.show({ title: "Un." });
    toasts.show({ title: "Deux." });
    toasts.dismiss(first);
    toasts.dismiss("k-toast-unknown");
    expect(toasts.list.map((toast) => toast.title)).toEqual(["Deux."]);
  });

  it("gives ids unique in the page, whatever the store", () => {
    expect(createToasts().show({ title: "A." })).not.toBe(createToasts().show({ title: "B." }));
  });
});

describe("ToastRegion", () => {
  const render = (toasts = createToasts()) =>
    renderToString(createSSRApp({ render: () => h(ToastRegion, { toasts }) }));

  it("holds both live regions, empty, before any toast; no landmark yet", async () => {
    const html = await render();
    expect(html).toContain('<div class="k-toasts__lane" role="alert" aria-atomic="false">');
    expect(html).toContain('<div class="k-toasts__lane" role="status" aria-atomic="false">');
    expect(html).not.toContain("aria-label");
    expect(html).not.toContain("k-toast ");
    // An empty list is no list.
    expect(html.match(/<ol [^>]*role="none"/g)).toHaveLength(2);
  });

  it("puts an error in the alert region, the others in the status one", async () => {
    const toasts = createToasts();
    toasts.show({ tone: "ok", title: "Préfixe ajouté." });
    toasts.show({ tone: "ko", title: "Échec de l'écriture.", code: "urn:kartenn:problem:x" });
    const html = await render(toasts);
    expect(html).toMatch(/^<section class="k-toasts" aria-label="Notifications">/);
    const [alert, status] = html.split('class="k-toasts__lane" role="status"');
    expect(alert).toContain("Échec de l&#39;écriture.");
    expect(alert).toContain("urn:kartenn:problem:x");
    expect(alert).not.toContain("Préfixe ajouté.");
    expect(status).toContain("Préfixe ajouté.");
    expect(html.match(/aria-label="Fermer la notification"/g)).toHaveLength(2);
    // The tone in words before the title (WCAG 1.3.1).
    expect(html).toMatch(/Erreur[^<]*<\/span>Échec/);
  });
});
