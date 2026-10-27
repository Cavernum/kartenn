// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { afterEach, describe, expect, it } from "vitest";
import { createLabels, lookup, type Dictionary } from "../src/data/labels.ts";
import { addCatalog } from "../src/i18n.ts";
import { resetContext, setContext } from "../src/context.ts";
import { P1, fakeCache, json, notModified } from "./helpers.ts";

afterEach(() => resetContext());

// `GET /v1/labels` is the contract of the 018 (design « Dictionnaire »): mocked here.
const dictionary = (revision: string, environment: string): Dictionary => ({
  lang: "fr",
  default_lang: "fr",
  revision,
  spaces: {
    value: { "policy.environment": { prod: environment } },
    kind: { "core.prefix": "Préfixe" },
  },
  fallback: null,
});

function setup() {
  let current = dictionary("r1", "Production");
  const cache = fakeCache((request) => {
    if (request.url.startsWith("/v1/labels")) {
      const etag = `"${current.revision}.fr"`;
      return request.headers.get("If-None-Match") === etag
        ? notModified(etag)
        : json(current, { etag });
    }
    return json({ id: P1 });
  });
  const labels = createLabels({ ...cache, locale: () => "fr" });
  const publish = (next: Dictionary) => (current = next);
  return { ...cache, labels, publish };
}

describe("labels (REQ-021-19, spec 018)", () => {
  it("look a key up by space, a value of a list under its property", () => {
    const d = dictionary("r1", "Production");
    expect(lookup(d, "kind", "core.prefix")).toBe("Préfixe");
    expect(lookup(d, "value", "policy.environment/prod")).toBe("Production");
    expect(lookup(d, "value", "policy.environment/dev")).toBeNull();
    expect(lookup(undefined, "kind", "x")).toBeNull();
  });

  it("show the key until the dictionary is read, then the label; the catalog in between", async () => {
    const s = setup();
    setContext({ locale: "fr" });
    // A catalog nests its keys: `labels.kind.policy.rule`.
    addCatalog("fr", "labels", { kind: { policy: { rule: "Règle" } } });
    expect(s.labels.label("kind", "core.prefix")).toBe("core.prefix");
    await s.queryClient.fetchQuery(s.labels.query("fr"));
    expect(s.labels.label("kind", "core.prefix")).toBe("Préfixe");
    expect(s.labels.label("kind", "policy.rule")).toBe("Règle");
    expect(s.labels.label("kind", "machines.machine")).toBe("machines.machine");
    expect(s.seen.filter((r) => r.url === "/v1/labels?lang=fr")).toHaveLength(1);
  });

  it("the event labels: another revision is read again, no object is", async () => {
    const s = setup();
    const object = s.reads.object("core.prefix", P1, null);
    await s.queryClient.fetchQuery(object);
    await s.queryClient.fetchQuery(s.labels.query("fr"));
    const event = (revision: string) => s.labels.changed(revision);
    // The revision already held: nothing to read.
    await event("r1");
    expect(s.queryClient.getQueryState(["v1", "labels", "fr"])?.isInvalidated).toBe(false);
    s.publish(dictionary("r2", "Prod."));
    await event("r2");
    await s.queryClient.refetchQueries({ queryKey: ["v1", "labels"] });
    expect(s.labels.label("value", "policy.environment/prod")).toBe("Prod.");
    expect(s.queryClient.getQueryState(object.queryKey)?.isInvalidated).toBe(false);
    expect(s.seen.filter((r) => r.url.startsWith("/v1/prefixes"))).toHaveLength(1);
  });
});
