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
});

describe("status bar", () => {
  it("DatePill: present, or the instant with its named zone, opening a dialog", async () => {
    const present = await render(DatePill, { timeZone: "Europe/Paris" });
    expect(present).toContain('data-state="present"');
    expect(present).toContain('aria-haspopup="dialog"');
    expect(present).toContain("Présent");
    const past = await render(DatePill, { at: "2025-11-03T08:30:00Z", timeZone: "Europe/Paris" });
    expect(past).toContain('data-state="past"');
    expect(past).toContain('datetime="2025-11-03T08:30:00.000Z"');
    expect(past).toContain("Europe/Paris");
  });

  it("formats an instant in the language and the named zone", () => {
    expect(formatInstant("2025-11-03T08:30:00Z", "Europe/Paris")).toBe("3 nov. 2025, 09:30");
    setLocale("en");
    expect(formatInstant("2025-11-03T08:30:00Z", "America/Montreal")).toBe("Nov 3, 2025, 03:30");
  });

  it("DraftCounter: its count in the name, muted at zero, controls the panel", async () => {
    const html = await render(DraftCounter, { count: 3, expanded: true, controls: "draft" });
    expect(html.replace(/<[^>]+>/g, "")).toContain("Brouillon(3)");
    expect(html).toContain('aria-expanded="true"');
    expect(html).toContain('aria-controls="draft"');
    expect(await render(DraftCounter, { count: 0 })).toContain('data-state="empty"');
  });

  it("TimeBand: a status with the instant and the way back", async () => {
    const html = await render(TimeBand, { at: "2025-11-03T08:30:00Z", timeZone: "Europe/Paris" });
    expect(html).toContain('role="status"');
    expect(html).toContain("Lecture au");
    expect(html).toContain("Revenir au présent");
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
