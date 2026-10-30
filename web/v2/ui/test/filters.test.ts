// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import { afterEach, describe, expect, it } from "vitest";
import { addressFilters } from "../src/components/FilterPicker/FilterPicker.catalogue.ts";
import FilterPicker from "../src/components/FilterPicker/FilterPicker.vue";
import {
  arity,
  checkFilter,
  checkValue,
  filterParam,
  filterSentence,
  filtersFromParams,
  filtersToParams,
  filterWords,
  propertyOf,
  type Filter,
  type FilterModel,
} from "../src/components/FilterPicker/filters.ts";
import { setLocale } from "../src/i18n.ts";

// Typed filters (spec 020, task 19; REQ-020-20, 021/X18): the check of a value by its
// type, the grammar of the URL (that of the list route, 006 « Filtres ») and its round
// trip, the words of a chip (REQ-020-21: a label, else the key), the bar as rendered.

afterEach(() => setLocale("fr"));

const model = addressFilters;
const read = (query: string) => filtersFromParams(model, new URLSearchParams(query));
const write = (filters: Filter[]) => decodeURIComponent(filtersToParams(model, filters).toString());

describe("the URL of the filters", () => {
  it("writes the grammar of the route: eq, one of, own parameter, bounds, presence", () => {
    expect(
      write([
        { key: "domain", operator: "eq", values: ["dc-nord"] },
        { key: "usage", operator: "eq", values: ["host", "vip"] },
        { key: "value", operator: "within", values: ["192.0.2.0/24"] },
        { key: "prop.dns.ttl", operator: "between", values: ["60", ""] },
        { key: "created", operator: "between", values: ["", "2026-01-15"] },
        { key: "prop.policy.environment", operator: "exists", values: [] },
        { key: "prop.core.owner", operator: "absent", values: [] },
        { key: "prop.core.description", operator: "contains", values: ["dns"] },
        { key: "created", operator: "after", values: ["2026-01-01"] },
      ]),
    ).toBe(
      "domain=dc-nord&usage=host&usage=vip&within=192.0.2.0/24&prop.dns.ttl:between=60..&" +
        "created:between=..2026-01-15&prop.policy.environment:exists=true&" +
        "prop.core.owner:exists=false&prop.core.description:contains=dns&created:after=2026-01-01",
    );
  });

  it("leaves the drafts out", () => {
    expect(
      write([
        { key: "domain", operator: "eq", values: ["dc-nord"] },
        { key: "usage", operator: "eq", values: ["host"], draft: true },
      ]),
    ).toBe("domain=dc-nord");
  });

  it("reads them back, and keeps the parameters that are no filter as they are", () => {
    const { filters, rest } = read(
      "valid_at=2026-01-15T10:00:00Z&usage=host&domain=dc-nord&usage=vip&within=192.0.2.0/24" +
        "&prop.core.owner:exists=false&sort=value",
    );
    expect(filters).toEqual([
      { key: "usage", operator: "eq", values: ["host", "vip"] },
      { key: "domain", operator: "eq", values: ["dc-nord"] },
      { key: "value", operator: "within", values: ["192.0.2.0/24"] },
      { key: "prop.core.owner", operator: "absent", values: [] },
    ]);
    expect(rest.toString()).toBe("valid_at=2026-01-15T10%3A00%3A00Z&sort=value");
  });

  it("keeps a value that does not fit, which its check refuses", () => {
    const { filters } = read(
      "created:after=2026-13-01&within=192.0.2.0/33&prop.dns.ttl:between=60",
    );
    expect(filters.map((f) => checkFilter(model, f))).toEqual([
      "Date invalide : le mois va de 01 à 12.",
      "Longueur de préfixe hors de 0 à 32.",
      "Deux bornes attendues, séparées par « .. ».",
    ]);
    expect(write(filters)).toBe(
      "created:after=2026-13-01&within=192.0.2.0/33&prop.dns.ttl:between=60",
    );
  });

  it("round trip: any valid filters come back the same from their query (seeded)", () => {
    // A small seeded generator (mulberry32): the same 500 cases at each run.
    let seed = 0x5eed;
    const random = () => {
      seed = (seed + 0x6d2b79f5) | 0;
      let x = Math.imul(seed ^ (seed >>> 15), 1 | seed);
      x = (x + Math.imul(x ^ (x >>> 7), 61 | x)) ^ x;
      return ((x ^ (x >>> 14)) >>> 0) / 4294967296;
    };
    const pick = <T>(list: readonly T[]): T => list[Math.floor(random() * list.length)]!;
    const sample: Record<string, readonly string[]> = {
      enum: [],
      text: ["dns", "a&b=c", "é ü", "x..y", "50%"],
      number: ["0", "-4", "3.5", "3600"],
      address: ["192.0.2.10", "2001:db8::1", "::ffff:192.0.2.1"],
      prefix: ["192.0.2.0/24", "2001:db8::/32", "0.0.0.0/0"],
      date: ["2026-01-15", "2024-02-29", "1999-12-31"],
    };
    for (let run = 0; run < 500; run += 1) {
      const filters: Filter[] = [];
      const params = new Set<string>();
      for (let n = Math.floor(random() * 5); n > 0; n -= 1) {
        const property = pick(model);
        const operator = pick(property.operators);
        const type = operator === "within" ? "prefix" : property.type;
        const values = (k: number) =>
          Array.from({ length: k }, () =>
            type === "enum"
              ? pick(property.options?.map((o) => o.value) ?? ["atelier-nord", "exploitation"])
              : pick(sample[type]!),
          );
        const count = arity(operator);
        const filter: Filter = {
          key: property.key,
          operator,
          values:
            count === 0
              ? []
              : count === 2
                ? pick([values(2), [values(1)[0]!, ""], ["", values(1)[0]!]])
                : values(count === 1 ? 1 : 1 + Math.floor(random() * 3)),
        };
        if (count === 2 && type !== "text") filter.values.sort();
        if (count === "many") filter.values = [...new Set(filter.values)];
        const param = filterParam(model, filter);
        if (params.has(param) || checkFilter(model, filter)) continue;
        params.add(param);
        filters.push(filter);
      }
      const query = filtersToParams(model, filters).toString();
      expect(read(query).filters, query).toEqual(filters);
    }
  });
});

describe("the check of a value by its type", () => {
  const of = (key: string) => propertyOf(model, key)!;

  it("prefixes: form, length, host bits with the canonical prefix", () => {
    const within = of("value");
    expect(checkValue(within, "192.0.2.0/24", "within")).toBeUndefined();
    expect(checkValue(within, "2001:db8::/32", "within")).toBeUndefined();
    expect(checkValue(within, "192.0.2.0/33", "within")).toBe(
      "Longueur de préfixe hors de 0 à 32.",
    );
    expect(checkValue(within, "2001:db8::/129", "within")).toBe(
      "Longueur de préfixe hors de 0 à 128.",
    );
    expect(checkValue(within, "192.0.2.10/24", "within")).toBe(
      "Bits d'hôte non nuls : le préfixe est 192.0.2.0/24.",
    );
    expect(checkValue(within, "2001:DB8:0:0:1::/32", "within")).toBe(
      "Bits d'hôte non nuls : le préfixe est 2001:db8::/32.",
    );
    expect(checkValue(within, "192.0.2.0", "within")).toMatch(/^Préfixe attendu/);
  });

  it("addresses: IPv4 without leading zero, IPv6 in its forms", () => {
    const address = of("value");
    for (const ok of ["192.0.2.10", "2001:db8::1", "::", "::1", "::ffff:192.0.2.1", "fe80::1:2"])
      expect(checkValue(address, ok), ok).toBeUndefined();
    for (const ko of [
      "192.0.2.256",
      "192.0.2.010",
      "192.0.2",
      "2001:db8:::1",
      "1:2:3:4:5:6:7:8:9",
      "abc",
    ])
      expect(checkValue(address, ko), ko).toMatch(/^Adresse IPv4 ou IPv6 attendue/);
  });

  it("dates, numbers, closed lists, empty values", () => {
    const created = of("created");
    expect(checkValue(created, "2024-02-29")).toBeUndefined();
    expect(checkValue(created, "2026-13-01")).toBe("Date invalide : le mois va de 01 à 12.");
    expect(checkValue(created, "2026-02-29")).toBe("Date invalide : ce mois a 28 jours.");
    expect(checkValue(created, "15/01/2026")).toBe("Date attendue sous la forme AAAA-MM-JJ.");
    expect(checkValue(of("prop.dns.ttl"), "soixante")).toMatch(/^Nombre attendu/);
    expect(checkValue(of("usage"), "router")).toBe("Valeur hors de la liste : router.");
    // A domain read from the API is checked by the route, not here.
    expect(checkValue(of("prop.core.owner"), "inconnu")).toBeUndefined();
    expect(checkValue(of("usage"), "  ")).toBe("Une valeur est attendue.");
  });

  it("filters: unknown property, operator not served, bounds", () => {
    expect(checkFilter(model, { key: "colour", operator: "eq", values: ["red"] })).toBe(
      "Propriété inconnue de cette liste : colour.",
    );
    expect(checkFilter(model, { key: "usage", operator: "contains", values: ["h"] })).toBe(
      "Opérateur non servi pour cette propriété.",
    );
    expect(checkFilter(model, { key: "prop.dns.ttl", operator: "between", values: ["", ""] })).toBe(
      "Une borne au moins est attendue.",
    );
    expect(
      checkFilter(model, { key: "prop.dns.ttl", operator: "between", values: ["3600", "60"] }),
    ).toBe("La borne basse dépasse la borne haute.");
    expect(
      checkFilter(model, { key: "prop.core.owner", operator: "exists", values: ["peut-être"] }),
    ).toBe("Valeur attendue : true ou false.");
  });
});

describe("the words of a chip", () => {
  it("labels first, the key in fixed pitch without one (REQ-020-21)", () => {
    expect(filterWords(model, { key: "usage", operator: "eq", values: ["host", "vip"] })).toEqual({
      property: "Usage",
      propertyMono: false,
      operator: "est l'un de",
      value: "Hôte, VIP",
      valueMono: false,
    });
    expect(
      filterWords(model, { key: "prop.policy.environment", operator: "eq", values: ["form"] }),
    ).toMatchObject({ property: "Environnement", value: "form", valueMono: true });
    expect(
      filterWords(model, { key: "prop.dns.ttl", operator: "between", values: ["60", "3600"] }),
    ).toMatchObject({ property: "prop.dns.ttl", propertyMono: true, value: "60 et 3600" });
  });

  it("open bounds, no value, the sentence, in both languages", () => {
    const from: Filter = { key: "created", operator: "between", values: ["2026-01-01", ""] };
    expect(filterSentence(model, from)).toBe("Création à partir de 2026-01-01");
    expect(filterSentence(model, { key: "prop.core.owner", operator: "absent", values: [] })).toBe(
      "Propriétaire n'est pas renseigné",
    );
    expect(
      filterSentence(model, { key: "value", operator: "within", values: ["192.0.2.0/24"] }),
    ).toBe("Adresse dans 192.0.2.0/24");
    setLocale("en");
    expect(filterSentence(model, from)).toBe("Création from 2026-01-01");
    expect(filterSentence(model, { key: "usage", operator: "eq", values: ["host", "vip"] })).toBe(
      "Usage is one of Hôte, VIP",
    );
  });
});

describe("FilterPicker as rendered", () => {
  const render = (props: { model: FilterModel; modelValue: Filter[] }) =>
    renderToString(createSSRApp({ render: () => h(FilterPicker, props) }));
  const bare: FilterModel = model;

  it("a named group of chips, a button that opens the picker, a polite region", async () => {
    const html = await render({
      model: bare,
      modelValue: [
        { key: "usage", operator: "eq", values: ["host", "vip"] },
        { key: "created", operator: "after", values: ["2026-13-01"] },
        { key: "prop.core.owner", operator: "eq", values: ["atelier-nord"], draft: true },
      ],
    });
    expect(html).toMatch(/role="group" aria-label="Filtres"/);
    expect(html).toContain('aria-label="Retirer le filtre Usage est l&#39;un de Hôte, VIP"');
    expect(html).toContain("(invalide)");
    expect(html).toContain("Date invalide : le mois va de 01 à 12.");
    expect(html).toContain("(en brouillon)");
    expect(html).toMatch(/aria-haspopup="dialog" aria-expanded="false"[^>]*>.*Ajouter un filtre/s);
    expect(html).toMatch(
      /<span class="k-visually-hidden" role="status" aria-live="polite"><\/span>/,
    );
  });
});
