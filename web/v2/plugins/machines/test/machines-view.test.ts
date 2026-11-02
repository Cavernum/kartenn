// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { queryClient, type components, type Extension, type User } from "@kartenn/sdk";
import { resetContext } from "@kartenn/sdk/host";
import { announcePlugin, fakeFetch, mount, type Mounted } from "@kartenn/sdk/testing";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import en from "../public/i18n/en.json";
import fr from "../public/i18n/fr.json";
import manifest from "../public/manifest.json";

// The view of the machines in Vue (spec 021, task 18cb; the behaviour of the 18a and 18b
// kept): its list in a `DataGrid` with the typed filters of `/v1/machines` in the query of
// the page, the creation of a machine; the record in sections, the move of its host by
// `canMove` and `draft.move`, the creation of an interface. Mounted in the sandbox of the
// SDK, in the light DOM of its element, with a simulated `fetch`.

const MACHINE = "4c9f2a52-0f1e-4f55-9a43-6f1f2d1b7a10";
const GUEST = "5d0a3b61-1f2e-4a66-8b54-7a2e3c2d8b21";
const OTHER = "5d0a3b61-1f2e-4a66-8b54-7a2e3c2d8b22";
const CLUSTER = "5d0a3b61-1f2e-4a66-8b54-7a2e3c2d8b23";
const ENTITY = "0b7d1c8e-3a54-4b1c-8f0e-2d6a9c4e5f21";
const IFACE = "6a1d3e70-58c4-4f0b-9e2a-1c7d5b3f9e42";
const VLAN = "7e2b1c90-1d4a-4c3e-8b5f-0a9d8c7b6e53";
const ADDRESS = "8f3c2da1-2e5b-4d4f-9c60-1b0e9d8c7f64";
const DRAFT = "1b5e4fc3-4071-4f61-9e82-3d2a1f0e9b86";

type Machine = components["schemas"]["Machine"];

const machine = (name = "srv-a"): Machine => ({
  id: MACHINE,
  href: `/v1/machines/${MACHINE}`,
  name,
  kind: "physical",
  owner: { id: ENTITY },
});
const card = () => ({
  ...machine(),
  interfaces: [
    {
      id: IFACE,
      href: `/v1/interfaces/${IFACE}`,
      name: "eth0",
      mac: "00:00:5e:00:53:01",
      enabled: true,
      addresses: [{ id: ADDRESS, value: "192.0.2.10", primary: true }],
    },
  ],
});
const iface = () => ({
  id: IFACE,
  href: `/v1/interfaces/${IFACE}`,
  machine: { id: MACHINE },
  name: "eth0",
  mac: "00:00:5e:00:53:01",
  enabled: true,
  vlan: { mode: "access", access: { id: VLAN }, tagged: [] },
});
const entity = { id: ENTITY, slug: "exemple", href: `/v1/entities/${ENTITY}` };
const vlan = { id: VLAN, vid: 310, name: "hotes", group: { id: ENTITY }, status: "active" };
const cluster = {
  id: CLUSTER,
  href: `/v1/clusters/${CLUSTER}`,
  slug: "grappe",
  owner: { id: ENTITY },
};
const page = <T>(items: T[]) => ({ items, links: {} }) as never;

/** A user who holds every permission, or none of `missing`. */
const user = (missing: string[] = []): User => ({
  identity: "alice",
  name: "Alice",
  kind: "user",
  has: (permission) => !missing.includes(permission),
});

const problem = (status: number, slug: string) =>
  new Response(JSON.stringify({ type: `urn:kartenn:problem:${slug}`, title: "Refused", status }), {
    status,
    headers: { "Content-Type": "application/problem+json", "Request-Id": "req-42" },
  });

/** A draft of the server holding `operations`. */
const draftOf = (operations: unknown[], version = 1, status = 200) =>
  new Response(
    JSON.stringify({
      id: DRAFT,
      href: `/v1/drafts/${DRAFT}`,
      author: DRAFT,
      title: "t",
      operation_count: operations.length,
      version,
      created_at: "2026-10-24T00:00:00Z",
      updated_at: "2026-10-24T00:00:00Z",
      reason: {},
      operations,
    }),
    { status, headers: { "Content-Type": "application/json", ETag: `"${version}"` } },
  );

let mounted: Mounted<HTMLElement> | null = null;
/** The links the view followed (the shell's router would have). */
let followed: string[] = [];
const follow = (event: MouseEvent) => {
  const anchor = event.target instanceof Element ? event.target.closest("a") : null;
  if (!anchor) return;
  event.preventDefault();
  followed.push(anchor.getAttribute("href") ?? "");
};

beforeAll(async () => {
  const defined = announcePlugin({
    name: manifest.name,
    extensions: manifest.extensions as Extension[],
    catalogs: { en, fr },
    integrity: { "styles.css": "sha384-test" },
  });
  await import("../src/index.ts");
  await defined;
  document.addEventListener("click", follow);
});

afterEach(() => {
  mounted?.unmount();
  mounted = null;
  followed = [];
  queryClient.clear();
  resetContext();
  sessionStorage.clear();
  history.replaceState(null, "", "/");
});

/** The text of the element (light DOM), once it holds `expected`. */
async function text(element: HTMLElement, expected: string | RegExp): Promise<string> {
  let found = "";
  await vi.waitFor(() => {
    found = element.textContent ?? "";
    expect(found).toMatch(expected);
  });
  return found;
}

/** The rows of the grid named `name`, each its cells' texts. */
const rows = (element: HTMLElement, name: string) => {
  const grid = [...element.querySelectorAll("[role=grid]")].find(
    (g) => g.getAttribute("aria-label") === name,
  );
  return [...(grid?.querySelectorAll("[role=row]") ?? [])]
    .slice(1)
    .map((row) => [...row.querySelectorAll("[role=gridcell]")].map((c) => c.textContent?.trim()));
};

const buttonOf = (element: HTMLElement, name: string) =>
  [...element.querySelectorAll<HTMLButtonElement>("button")].find((b) =>
    b.textContent?.includes(name),
  )!;

/** The value of the field named `label`, typed as the user would. */
function type(element: HTMLElement, label: string, value: string): void {
  const field = [...element.querySelectorAll("label")].find((l) =>
    l.textContent?.startsWith(label),
  );
  const input = element.querySelector<HTMLInputElement>(`#${CSS.escape(field!.htmlFor)}`)!;
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

describe("the list of the machines (task 18cb)", () => {
  const reads = {
    "GET /v1/entities": () => page([entity]),
    "GET /v1/entities/{id}": () => entity as never,
  };

  it("lists the machines in a grid through the shared cache, again once invalidated", async () => {
    let name = "srv-a";
    let answer: () => void = () => {};
    const answered = new Promise<void>((resolve) => (answer = resolve));
    history.replaceState(null, "", "/x/machines/machines");
    mounted = mount("machines-machines", {
      context: { locale: "fr", user: user() },
      fetch: fakeFetch({
        ...reads,
        "GET /v1/machines": async () => {
          await answered;
          return page([machine(name)]);
        },
      }),
    });
    const { element } = mounted;
    // Skeleton rows in a busy grid, the heading already there.
    await vi.waitFor(() =>
      expect(element.querySelector("[role=grid]")?.getAttribute("aria-busy")).toBe("true"),
    );
    const heading = element.querySelector("h1")!;
    expect(heading.textContent).toBe("Machines");
    answer();
    await vi.waitFor(() =>
      expect(rows(element, "Machines")).toEqual([
        // The kind by its label, its key in its description (REQ-020-21).
        ["srv-a", "physiquephysical", "exemple", "—aucun", "—aucun"],
      ]),
    );
    const link = element.querySelector<HTMLAnchorElement>("[role=gridcell] a")!;
    expect(link.getAttribute("href")).toBe(`/x/machines/machines/${MACHINE}`);
    // No sheet of its own, no shadow root (A23).
    expect(element.shadowRoot).toBeNull();
    expect(element.querySelector("link, style")).toBeNull();
    name = "srv-b";
    await queryClient.invalidateQueries({ queryKey: ["v1", "/v1/machines"], exact: false });
    await vi.waitFor(() => expect(rows(element, "Machines")[0]?.[0]).toBe("srv-b"));
    expect(element.querySelector("h1")).toBe(heading);
  });

  it("says a failed list with its code and Request-Id, an empty one with what to do", async () => {
    history.replaceState(null, "", "/x/machines/machines");
    let answer: () => Response | never = () => problem(403, "forbidden");
    mounted = mount("machines-machines", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({ ...reads, "GET /v1/machines": () => answer() }),
    });
    await text(mounted.element, "The machines could not be read");
    await text(mounted.element, "req-42");
    expect(mounted.element.textContent).toContain("urn:kartenn:problem:forbidden");
    // « Retry » reads it again: answered this time.
    answer = () => page([machine()]);
    buttonOf(mounted.element, "Retry").click();
    await vi.waitFor(() => expect(rows(mounted!.element, "Machines")[0]?.[0]).toBe("srv-a"));
    mounted.unmount();
    queryClient.clear();
    mounted = mount("machines-machines", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({ ...reads, "GET /v1/machines": () => page([]) }),
    });
    await text(mounted.element, "No machine");
    expect(mounted.element.textContent).toContain("Add one to the draft with the form below.");
  });

  it("filters by the parameters of the route, held in the query of the page", async () => {
    const asked: string[] = [];
    history.replaceState(
      null,
      "",
      "/x/machines/machines?kind=virtual&valid_at=2026-01-01T00%3A00%3A00Z",
    );
    mounted = mount("machines-machines", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        ...reads,
        "GET /v1/machines": ({ query }) => {
          asked.push(query.get("kind") ?? "");
          return page([]);
        },
      }),
    });
    const { element } = mounted;
    await text(element, "No machine matches these filters");
    expect(asked).toEqual(["virtual"]);
    // The chip of the filter in its group, its column marked filtered.
    expect(element.querySelector("[role=group]")?.textContent).toContain("virtual");
    // Removed: the query of the page without it, the dates kept, by a link of the shell.
    buttonOf(element, "Remove the filters").click();
    await vi.waitFor(() =>
      expect(followed).toEqual(["/x/machines/machines?valid_at=2026-01-01T00%3A00%3A00Z"]),
    );
    // Back in the history: the filters of the page read again.
    history.replaceState(null, "", "/x/machines/machines?kind=container");
    window.dispatchEvent(new PopStateEvent("popstate"));
    await vi.waitFor(() => expect(asked).toContain("container"));
  });

  it("refuses two values of one filter before anything is sent", async () => {
    const asked: string[] = [];
    history.replaceState(null, "", "/x/machines/machines?kind=virtual&kind=physical");
    mounted = mount("machines-machines", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        ...reads,
        "GET /v1/machines": ({ query }) => (asked.push(query.toString()), page([])),
      }),
    });
    await text(mounted.element, "One value only for each filter: Kind.");
    await new Promise((done) => setTimeout(done, 20));
    expect(asked).toEqual([]);
  });

  it("creates a machine in the draft, refusing an empty name before sending anything", async () => {
    const posted: unknown[] = [];
    history.replaceState(null, "", "/x/machines/machines");
    mounted = mount("machines-machines", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        ...reads,
        "GET /v1/machines": () => page([]),
        "GET /v1/drafts": () => page([]),
        "POST /v1/drafts": ({ body }) => {
          posted.push(body);
          return draftOf((body as { operations: unknown[] }).operations, 1, 201);
        },
      }),
    });
    const { element } = mounted;
    // The first owner taken once read.
    await text(element, "exemple");
    const form = element.querySelector("section form") as HTMLFormElement;
    form.requestSubmit();
    const name = await vi.waitFor(() => {
      const input = form.querySelector<HTMLInputElement>("input[aria-invalid=true]");
      expect(input).not.toBeNull();
      return input!;
    });
    await vi.waitFor(() => expect(document.activeElement).toBe(name));
    expect(form.textContent).toContain("Required field.");
    expect(posted).toEqual([]);
    type(element, "Name", "srv-c");
    form.requestSubmit();
    await text(element, "Operation added; the draft holds 1.");
    expect(posted).toMatchObject([
      {
        operations: [
          { op: "machines.create_machine", name: "srv-c", kind: "physical", owner: { id: ENTITY } },
        ],
      },
    ]);
    expect(name.value).toBe("");
    expect(name.hasAttribute("aria-invalid")).toBe(false);
  });

  it("keeps the button visible but refused without the permission, the form gone at another date", async () => {
    const posted: unknown[] = [];
    history.replaceState(null, "", "/x/machines/machines");
    mounted = mount("machines-machines", {
      context: { locale: "en", user: user(["machines.edit"]) },
      fetch: fakeFetch({
        ...reads,
        "GET /v1/machines": () => page([]),
        "POST /v1/drafts": ({ body }) => (posted.push(body), draftOf([])),
      }),
    });
    const { element } = mounted;
    await text(element, "No machine");
    const submit = element.querySelector<HTMLButtonElement>("section button[type=submit]")!;
    expect(submit.getAttribute("aria-disabled")).toBe("true");
    type(element, "Name", "srv-c");
    submit.closest("form")!.requestSubmit();
    await text(element, "Needs the permission machines.edit");
    expect(posted).toEqual([]);
    mounted.unmount();
    queryClient.clear();
    mounted = mount("machines-machines", {
      context: { user: user(), time: { validAt: "2026-01-01T00:00:00Z", asOf: null } },
      fetch: fakeFetch({ ...reads, "GET /v1/machines": () => page([]) }),
    });
    await text(mounted.element, "No machine");
    expect(mounted.element.querySelector("section")).toBeNull();
  });
});

describe("the record of a machine (task 18cb)", () => {
  const listed = () =>
    page([
      machine(),
      { ...machine("vm-a"), id: GUEST, kind: "virtual", host: { machine: { id: MACHINE } } },
      { ...machine("srv-b"), id: OTHER },
    ]);
  const simulation = (valid: boolean, violations: unknown[]) => ({
    valid,
    violations,
    diff: [],
    sample: [],
    cascaded: [],
    sections: [],
  });
  const reads = {
    "GET /v1/machines/{id}": () => card() as never,
    "GET /v1/machines/{id}/hosted": () => page([]),
    "GET /v1/interfaces": () => page([iface()]),
    "GET /v1/entities/{id}": () => entity as never,
    "GET /v1/vlans/{id}": () => vlan as never,
    "GET /v1/machines": () => listed(),
    "GET /v1/clusters": () => page([cluster]),
  };

  /** The host field of the record: its input, and the options it suggests for `text`. */
  async function suggested(element: HTMLElement, text: string): Promise<HTMLElement[]> {
    const input = element.querySelector<HTMLInputElement>("section form input[role=combobox]")!;
    input.value = text;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    return vi.waitFor(() => {
      const options = [...document.querySelectorAll<HTMLElement>("[role=option]")];
      expect(options.length).toBeGreaterThan(0);
      return options;
    });
  }
  /** Takes the host whose name starts with `name`, then sends the form. */
  async function move(element: HTMLElement, name: string): Promise<void> {
    const options = await suggested(element, name);
    const option = options.find((o) => o.textContent?.startsWith(name));
    expect(option, name).toBeDefined();
    option!.click();
    await new Promise((done) => setTimeout(done, 0));
    element.querySelector<HTMLFormElement>("section form")!.requestSubmit();
  }
  /** The host the record shows (the second-to-last attribute: « Hosted by »). */
  const hostedBy = (element: HTMLElement) =>
    [...element.querySelectorAll("dt")].at(-2)!.nextElementSibling!.textContent;

  it("draws its sections: attributes, interfaces with their state, hosted machines; adds an interface", async () => {
    history.replaceState(null, "", `/x/machines/machines/${MACHINE}`);
    const posted: unknown[] = [];
    mounted = mount("machines-machines", {
      context: { locale: "fr", user: user() },
      fetch: fakeFetch({
        ...reads,
        "GET /v1/drafts": () => page([]),
        "POST /v1/drafts": ({ body }) => {
          posted.push(body);
          return draftOf((body as { operations: unknown[] }).operations, 1, 201);
        },
      }),
    });
    const { element } = mounted;
    await vi.waitFor(() => expect(element.querySelector("h1")?.textContent).toBe("srv-a"));
    await vi.waitFor(() =>
      expect(rows(element, "Interfaces")[0]).toEqual([
        "eth0",
        "00:00:5e:00:53:01",
        "accès 310",
        "192.0.2.10",
        expect.stringContaining("active"),
      ]),
    );
    expect(hostedBy(element)).toBe("—aucun");
    await text(element, "exemple");
    expect(element.textContent).toContain("Aucune machine hébergée");
    const names = [...element.querySelectorAll("section h2")].map((h) => h.textContent);
    expect(names).toEqual([
      "Hébergement",
      "Interfaces",
      "Machines hébergées",
      "Ajouter une interface",
    ]);
    const form = [...element.querySelectorAll("section form")].at(-1) as HTMLFormElement;
    type(form, "Interface", "eth1");
    type(form, "MAC", "00:00:5e:00:53:02");
    form.requestSubmit();
    await text(element, "Opération ajoutée ; le brouillon en compte 1.");
    expect(posted).toMatchObject([
      {
        operations: [
          {
            op: "machines.create_interface",
            machine: { id: MACHINE },
            name: "eth1",
            mac: "00:00:5e:00:53:02",
          },
        ],
      },
    ]);
  });

  it("offers each host judged by canMove: a cycle and a right refused, with their code", async () => {
    history.replaceState(null, "", `/x/machines/machines/${MACHINE}`);
    const posted: unknown[] = [];
    mounted = mount("machines-machines", {
      context: { locale: "en", user: user(["machines.cluster.edit"]) },
      fetch: fakeFetch({
        ...reads,
        "POST /v1/drafts": ({ body }) => (posted.push(body), draftOf([])),
      }),
    });
    const { element } = mounted;
    await text(element, "srv-a");
    await vi.waitFor(async () => expect((await suggested(element, "")).length).toBe(5));
    const said = (await suggested(element, "")).map((o) => o.textContent?.trim());
    // None, its host now; a cluster without its right; itself and the machine it hosts, a
    // cycle, with the rule of the module; another machine, offered. Its type beside each.
    expect(said).toEqual([
      "None (not hosted) — current host",
      "grappe — refused: needs the permission machines.cluster.editcluster",
      "srv-a — refused: it hosts it, or is it (machines.host.invalid)machine",
      "srv-bmachine",
      "vm-a — refused: it hosts it, or is it (machines.host.invalid)machine",
    ]);
    // Sent anyway: refused on the field with its code, which takes the focus; nothing sent.
    await move(element, "vm-a");
    const input = element.querySelector<HTMLInputElement>("section form input[role=combobox]")!;
    await vi.waitFor(() => expect(input.getAttribute("aria-invalid")).toBe("true"));
    expect(element.querySelector("section form")!.textContent).toContain(
      "refused: it hosts it, or is it (machines.host.invalid)",
    );
    await vi.waitFor(() => expect(document.activeElement).toBe(input));
    await new Promise((done) => setTimeout(done, 20));
    expect(posted).toEqual([]);
    input.blur();
  });

  it("refuses the button without machines.edit, says why", async () => {
    history.replaceState(null, "", `/x/machines/machines/${MACHINE}`);
    const posted: unknown[] = [];
    mounted = mount("machines-machines", {
      context: { locale: "en", user: user(["machines.edit"]) },
      fetch: fakeFetch({
        ...reads,
        "POST /v1/drafts": ({ body }) => (posted.push(body), draftOf([])),
      }),
    });
    const { element } = mounted;
    await text(element, "srv-a");
    const submit = buttonOf(element, "Move in the draft");
    expect(submit.getAttribute("aria-disabled")).toBe("true");
    await move(element, "srv-b");
    await text(element, "Needs the permission machines.edit");
    await new Promise((done) => setTimeout(done, 20));
    expect(posted).toEqual([]);
    (document.activeElement as HTMLElement | null)?.blur();
  });

  it("in the draft, « en attente » until simulated, the host read unchanged; the warnings said", async () => {
    history.replaceState(null, "", `/x/machines/machines/${MACHINE}`);
    const posted: unknown[] = [];
    let release!: () => void;
    const held = new Promise<void>((done) => (release = done));
    const warning = {
      rule: "machines.host.site_mismatch",
      severity: "warning",
      detail: "the machine is hosted on another site",
      subjects: [MACHINE],
      operation: null,
      pointer: null,
    };
    mounted = mount("machines-machines", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        ...reads,
        "GET /v1/drafts": () => page([]),
        "POST /v1/drafts": ({ body }) => {
          posted.push(body);
          return draftOf((body as { operations: unknown[] }).operations);
        },
        "POST /v1/drafts/{id}/simulation": async () => {
          await held;
          return simulation(true, [warning]) as never;
        },
      }),
    });
    const { element } = mounted;
    await text(element, "srv-a");
    await move(element, "grappe");
    await text(element, "Move to grappe waiting for the server");
    const submit = buttonOf(element, "Move in the draft");
    expect(submit.getAttribute("aria-busy")).toBe("true");
    // Never optimistic: the host read stays (none).
    expect(hostedBy(element)).toBe("—none");
    release();
    await text(element, "Move to grappe in the draft, held by its simulation");
    expect(element.querySelector("[data-tone=warn]")?.textContent).toContain(
      "machines.host.site_mismatch",
    );
    expect(posted).toEqual([
      {
        title: expect.any(String),
        operations: [
          {
            op: "machines.update_machine",
            id: { id: MACHINE },
            host: { cluster: { id: CLUSTER } },
          },
        ],
      },
    ]);
    expect(hostedBy(element)).toBe("—none");
    expect(submit.hasAttribute("aria-busy")).toBe(false);
    (document.activeElement as HTMLElement | null)?.blur();
  });

  it("refused by the simulation: its rule said, the operation replaced by the next move", async () => {
    history.replaceState(null, "", `/x/machines/machines/${MACHINE}`);
    sessionStorage.setItem("kartenn.draft.active", DRAFT);
    const earlier = {
      op: "machines.update_machine",
      id: { id: MACHINE },
      host: { cluster: { id: CLUSTER } },
    };
    let operations: unknown[] = [earlier];
    const patched: unknown[] = [];
    const invalid = {
      rule: "machines.host.invalid",
      severity: "error",
      detail: "une machine physical ne peut pas être hébergée par une machine physical",
      subjects: [MACHINE],
      operation: 0,
      pointer: "/operations/0",
    };
    mounted = mount("machines-machines", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        ...reads,
        "GET /v1/drafts/{id}": () => draftOf(operations),
        "PATCH /v1/drafts/{id}": ({ body }) => {
          patched.push(body);
          operations = (body as { operations: unknown[] }).operations;
          return draftOf(operations, 2);
        },
        "POST /v1/drafts/{id}/simulation": () => simulation(false, [invalid]) as never,
      }),
    });
    const { element } = mounted;
    await text(element, "srv-a");
    await move(element, "srv-b");
    await text(element, "Move to srv-b refused by the server");
    expect(element.querySelector("[role=alert]")?.textContent).toContain("machines.host.invalid");
    expect(element.textContent).toContain(
      "The operation stays in the draft; another host chosen replaces it.",
    );
    expect(patched).toEqual([
      {
        operations: [
          { op: "machines.update_machine", id: { id: MACHINE }, host: { machine: { id: OTHER } } },
        ],
      },
    ]);
    (document.activeElement as HTMLElement | null)?.blur();
  });
});
