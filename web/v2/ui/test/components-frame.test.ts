// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createSSRApp, h, type Component } from "vue";
import { renderToString } from "vue/server-renderer";
import { afterEach, describe, expect, it } from "vitest";
import Breadcrumb from "../src/components/Breadcrumb/Breadcrumb.vue";
import DatePill from "../src/components/DatePill/DatePill.vue";
import DraftCounter from "../src/components/DraftCounter/DraftCounter.vue";
import Menu from "../src/components/Menu/Menu.vue";
import SidebarNav from "../src/components/SidebarNav/SidebarNav.vue";
import Tabs from "../src/components/Tabs/Tabs.vue";
import TimeBand from "../src/components/TimeBand/TimeBand.vue";
import { setLocale } from "../src/i18n.ts";
import { formatInstant } from "../src/time.ts";

// The visual version of the frame components (spec 020, task 11; REQ-020-16): landmarks,
// roles, names and ARIA states as rendered on the server.

const render = (component: Component, props: Record<string, unknown> = {}) =>
  renderToString(createSSRApp({ render: () => h(component, props) }));

afterEach(() => setLocale("fr"));

describe("Breadcrumb", () => {
  it("is a named navigation, an ordered list, the last crumb the current page", async () => {
    const html = await render(Breadcrumb, {
      items: [
        { label: "Référentiel", to: "/" },
        { label: "192.0.2.0/24", mono: true },
      ],
    });
    expect(html).toMatch(/^<nav class="k-breadcrumb" aria-label="Fil d&#39;Ariane">/);
    expect(html).toContain("<ol");
    expect(html).toContain('<a href="/"');
    expect(html).toMatch(/aria-current="page">192\.0\.2\.0\/24</);
    expect(html.match(/aria-current/g)).toHaveLength(1);
  });
});

describe("SidebarNav", () => {
  const groups = [
    {
      id: "referential",
      label: "Référentiel",
      entries: [
        { id: "home", label: "Accueil", icon: "house", to: "/" },
        { id: "prefixes", label: "Préfixes", icon: "prefix", to: "/prefixes", badge: 3 },
      ],
    },
  ];

  it("names its landmark, marks the active entry, discloses its spaces", async () => {
    const html = await render(SidebarNav, { brand: "kartenn", groups, active: "prefixes" });
    expect(html).toContain('aria-label="Navigation principale"');
    expect(html).toMatch(/<a href="\/prefixes"[^>]*aria-current="page"/);
    expect(html).toMatch(/aria-expanded="true" aria-controls="[^"]+-referential"/);
    expect(html).toContain('aria-keyshortcuts="["');
  });

  it("collapsed, keeps every name and drops the disclosures", async () => {
    const html = await render(SidebarNav, { brand: "kartenn", groups, collapsed: true });
    expect(html).toContain('data-state="collapsed"');
    expect(html).toContain("Préfixes");
    expect(html).toContain('aria-label="Référentiel"');
    expect(html).not.toContain("aria-controls");
  });

  it("leaves out a space without entries; « Rechercher » opens a dialog (Ctrl+K)", async () => {
    const html = await render(SidebarNav, {
      brand: "kartenn",
      groups: [...groups, { id: "admin", label: "Administration", entries: [] }],
    });
    expect(html).not.toContain("Administration");
    expect(html).toMatch(/aria-keyshortcuts="Control\+K" aria-haspopup="dialog"/);
  });
});

describe("status bar", () => {
  it("DatePill: present, or the dates read with their named zone, opening a dialog", async () => {
    const text = (markup: string) =>
      markup
        .replace(/<[^>]+>/g, " ")
        .replace(/\s+/g, " ")
        .trim();
    const present = await render(DatePill, { timeZone: "Europe/Paris" });
    expect(present).toContain('data-period="present"');
    expect(present).toContain('aria-haspopup="dialog"');
    expect(present).toContain('aria-expanded="false"');
    expect(text(present)).toBe("Date de lecture : Présent");
    const past = await render(DatePill, {
      validAt: "2025-11-03T08:30:00Z",
      timeZone: "Europe/Paris",
    });
    expect(past).toContain('data-period="past"');
    expect(past).toContain('datetime="2025-11-03T08:30:00.000Z"');
    expect(text(past)).toBe("Date de lecture : 3 nov. 2025, 09:30 Europe/Paris");
    const known = await render(DatePill, {
      asOf: "2025-12-01T07:00:00Z",
      timeZone: "Europe/Paris",
    });
    expect(text(known)).toBe("Date de lecture : Connu au 1 déc. 2025, 08:00 Europe/Paris");
    const both = await render(DatePill, {
      validAt: "2025-11-03T08:30:00Z",
      asOf: "2025-12-01T07:00:00Z",
      timeZone: "Europe/Paris",
    });
    expect(text(both)).toBe(
      "Date de lecture : 3 nov. 2025, 09:30 · connu au 1 déc. 2025, 08:00 Europe/Paris",
    );
  });

  it("formats an instant in the language and the named zone", () => {
    expect(formatInstant("2025-11-03T08:30:00Z", "Europe/Paris")).toBe("3 nov. 2025, 09:30");
    setLocale("en");
    expect(formatInstant("2025-11-03T08:30:00Z", "America/Montreal")).toBe("Nov 3, 2025, 03:30");
  });

  it("DraftCounter: its count in the name, muted at zero, controls the panel", async () => {
    const html = await render(DraftCounter, { count: 3, expanded: true, controls: "draft" });
    // Its name says what the figure counts, after the visible words.
    const name = (markup: string) =>
      markup
        .replace(/<[^>]+>/g, " ")
        .replace(/\s+/g, " ")
        .trim();
    expect(name(html)).toBe("Brouillon 3 opérations");
    expect(name(await render(DraftCounter, { count: 1 }))).toBe("Brouillon 1 opération");
    expect(html).toContain('aria-expanded="true"');
    expect(html).toContain('aria-controls="draft"');
    expect(await render(DraftCounter, { count: 0 })).toContain('data-state="empty"');
  });

  it("TimeBand: a status in two cases (valid_at, as_of) and the way back", async () => {
    const said = (markup: string) =>
      (markup.match(/role="status"[^>]*>(.*?)<\/p>/)?.[1] ?? "")
        // Inline elements (the dates): no space of their own.
        .replace(/<[^>]+>/g, "")
        .replace(/\s+/g, " ")
        .trim();
    const ZONE = "Europe/Paris";
    const valid = await render(TimeBand, { validAt: "2025-11-03T08:30:00Z", timeZone: ZONE });
    // A validity date alone never says that writes are off: a dated draft stays allowed.
    expect(said(valid)).toBe(
      "Lecture au 3 nov. 2025, 09:30 (Europe/Paris) — brouillon daté possible",
    );
    expect(valid).toContain("Revenir au présent");
    const known = await render(TimeBand, { asOf: "2025-12-01T07:00:00Z", timeZone: ZONE });
    expect(said(known)).toBe("État connu au 1 déc. 2025, 08:00 (Europe/Paris) — lecture seule");
    const both = await render(TimeBand, {
      validAt: "2025-11-03T08:30:00Z",
      asOf: "2025-12-01T07:00:00Z",
      timeZone: ZONE,
    });
    expect(said(both)).toBe(
      "Lecture au 3 nov. 2025, 09:30, connu au 1 déc. 2025, 08:00 (Europe/Paris) — lecture seule",
    );
    // At the present, its status region alone, empty: present before the band is said.
    const present = await render(TimeBand, { timeZone: ZONE });
    expect(present).toContain('data-state="present"');
    expect(said(present)).toBe("");
    expect(present).not.toContain("Revenir au présent");
  });
});

describe("Tabs and Menu", () => {
  it("Tabs: a named tablist, the first tab selected, the states said in words", async () => {
    const html = await render(Tabs, {
      label: "Fiche",
      tabs: [
        { value: "a", label: "Attributs" },
        { value: "b", label: "VLAN", state: "loading" },
      ],
    });
    expect(html).toContain('role="tablist"');
    expect(html).toContain('aria-label="Fiche"');
    const selected = /<button[^>]*role="tab"[^>]*aria-selected="true"[^>]*>(.*?)<\/button>/s.exec(
      html,
    );
    expect(selected?.[1]).toContain("Attributs");
    expect(html).toContain("(en chargement)");
  });

  it("Menu: a menu button, closed", async () => {
    const html = await render(Menu, { label: "Actions", items: [{ id: "a", label: "Modifier" }] });
    expect(html).toContain('aria-haspopup="menu"');
    expect(html).toContain('aria-expanded="false"');
  });
});
