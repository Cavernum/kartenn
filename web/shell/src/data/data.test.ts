import { afterEach, describe, expect, it, vi } from "vitest";
import { Pager, read, send, withQuery, write, type Read } from "./api.ts";
import { cell, linkOf, nameOf, pageOf, valueText } from "./collections.ts";

const PROBLEM = {
  type: "urn:kartenn:problem:invalid-cursor",
  title: "Bad Request",
  detail: null,
  status: 400,
};

/** A reader that answers from a table of URLs, recording what it was asked. */
function reader(answers: Record<string, () => Read<unknown>>, asked: string[]) {
  return async <B>(url: string): Promise<Read<B>> => {
    asked.push(url);
    const answer = answers[url];
    if (!answer) throw new Error(`unexpected read of ${url}`);
    return answer() as Read<B>;
  };
}

describe("the pages of a list (design « Écrans et routes du client »)", () => {
  it("follows the next links to the last page", async () => {
    const asked: string[] = [];
    const pager = new Pager<number>(
      "/v1/addresses",
      reader(
        {
          "/v1/addresses": () => ({
            ok: true,
            status: 200,
            body: { items: [1, 2], links: { next: "/v1/addresses?cursor=a" } },
          }),
          "/v1/addresses?cursor=a": () => ({
            ok: true,
            status: 200,
            body: { items: [3], links: { next: null } },
          }),
        },
        asked,
      ),
    );
    await pager.more();
    expect(pager.done.value).toBe(false);
    await pager.more();
    await pager.more();
    expect(pager.items.value).toEqual([1, 2, 3]);
    expect(pager.done.value).toBe(true);
    expect(asked).toEqual(["/v1/addresses", "/v1/addresses?cursor=a"]);
  });

  it("starts again from the first page when a cursor is refused", async () => {
    const asked: string[] = [];
    let first = 0;
    const pager = new Pager<number>(
      "/v1/prefixes",
      reader(
        {
          "/v1/prefixes": () => ({
            ok: true,
            status: 200,
            body: { items: [first++], links: { next: "/v1/prefixes?cursor=old" } },
          }),
          "/v1/prefixes?cursor=old": () => ({ ok: false, status: 400, problem: PROBLEM }),
        },
        asked,
      ),
    );
    await pager.more();
    await pager.more();
    expect(pager.restarted.value).toBe(true);
    expect(pager.items.value).toEqual([1]);
    expect(pager.problem.value).toBeNull();
    expect(asked).toEqual(["/v1/prefixes", "/v1/prefixes?cursor=old", "/v1/prefixes"]);
  });

  it("stops on another problem and keeps it", async () => {
    const forbidden = { ...PROBLEM, type: "urn:kartenn:problem:forbidden", status: 403 };
    const pager = new Pager<number>(
      "/v1/sites",
      reader({ "/v1/sites": () => ({ ok: false, status: 403, problem: forbidden }) }, []),
    );
    await pager.more();
    expect(pager.problem.value).toEqual(forbidden);
    expect(pager.done.value).toBe(true);
  });

  it("puts the filters in the URL, empty ones left out", () => {
    expect(
      withQuery("/v1/prefixes", { domain: "d", value: "", page_size: "100", as_of: null }),
    ).toBe("/v1/prefixes?domain=d&page_size=100");
    expect(withQuery("/v1/a?x=1", { y: "2" })).toBe("/v1/a?x=1&y=2");
  });
});

describe("the fields of an object", () => {
  const prefix = {
    id: "p1",
    value: "10.1.0.0/24",
    nature: "network",
    domain: { id: "d1", slug: "production", href: "/v1/domains/d1" },
    public: false,
    links: { self: "/v1/prefixes/p1" },
  };

  it("names an object by its value, slug or bounds", () => {
    expect(nameOf(prefix)).toBe("10.1.0.0/24");
    expect(nameOf({ id: "x", slug: "paris" })).toBe("paris");
    expect(nameOf({ id: "r", first: "10.0.0.1", last: "10.0.0.9" })).toBe("10.0.0.1 – 10.0.0.9");
    expect(nameOf({ id: "z" })).toBe("z");
  });

  it("shows summaries by their slug and links them to their record", () => {
    expect(cell(prefix, "domain")).toBe("production");
    expect(cell(prefix, "public")).toBe("false");
    expect(cell({ parent: null }, "parent")).toBe("—");
    expect(linkOf(prefix, "domain")).toBe("/domains/d1");
    expect(linkOf(prefix, "nature")).toBeNull();
  });

  it("maps a URL of the API to a page of the shell, collections of the core only", () => {
    expect(pageOf("/v1/prefixes/p1")).toBe("/prefixes/p1");
    expect(pageOf("https://kartenn.example/v1/addresses/a1")).toBe("/addresses/a1");
    expect(pageOf("/v1/changesets/c1")).toBeNull();
    expect(pageOf(undefined)).toBeNull();
  });

  it("writes values of properties: lists, references, nothing", () => {
    expect(valueText(["a", "b"])).toBe("a, b");
    expect(valueText([{ id: "e", slug: "exemple" }])).toBe("exemple");
    expect(valueText(null)).toBe("—");
    expect(valueText([])).toBe("—");
    expect(valueText(42)).toBe("42");
  });
});

describe("a body cut short by a navigation", () => {
  afterEach(() => vi.unstubAllGlobals());

  // Firefox rejects the read of a body when the page leaves: the screen gets a network
  // problem, never an error that Vue would log as uncaught.
  function cutShort(): void {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        const response = new Response("{}", { status: 200 });
        vi.spyOn(response, "text").mockRejectedValue(new TypeError("NetworkError"));
        vi.spyOn(response, "json").mockRejectedValue(new TypeError("NetworkError"));
        return response;
      }),
    );
  }

  it("is a network problem for a read, a write and a request", async () => {
    cutShort();
    await expect(read("/v1/domains")).resolves.toMatchObject({ ok: false, status: 0 });
    await expect(write("/v1/changesets", {})).resolves.toMatchObject({ ok: false, status: 0 });
    await expect(send("DELETE", "/v1/x")).resolves.toMatchObject({ ok: false, status: 0 });
  });
});
