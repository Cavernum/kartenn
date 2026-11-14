// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, h, nextTick, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import IdentityDetail from "../src/admin/IdentityDetail.vue";
import ImportDetail from "../src/admin/ImportDetail.vue";
import { frameRoutes } from "../src/frame/routes.ts";
import { frameWidth, setFrameWidth } from "../src/frame/width.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import { offered, play } from "../src/shortcuts/view.ts";
import { timeOf, writesOf } from "../src/time/index.ts";

// The access of the administration under 1024 px wide (task 021-21e2; REQ-021-29,
// Q-021-12): the service accounts, the identities, the imports. Each write refused at its
// point of action with the width, after the lasting reason; the three that write at once,
// not through the draft (the creation of an account and its secret, the pseudonymisation,
// the steering of an import), read the gate again when sending: under 1024 px nothing goes.

const NARROW = "Modification à partir de 1024 px de large : élargissez la fenêtre pour modifier.";
const SECRET = "ksk_test_value";

const IDENTITY = {
  id: "i",
  name: "bob",
  kind: "person",
  disabled: false,
  issuer: null,
  groups: [],
  bindings: [],
  last_login_idp_groups: [],
};
const IMPORT = {
  id: "m",
  source: "seed.json",
  state: "paused",
  chunks: 1,
  elements: 2,
  batches: 0,
  error: null,
  report: {},
};

interface Seen {
  method: string;
  url: string;
}

describe("the access of the administration under 1024 px", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    queryClient.clear();
    resetContext();
    sessionStorage.clear();
    setFetch(null);
    setFrameWidth("wide");
    document.body.innerHTML = "";
  });

  async function mounted(at: string, permissions: string[]) {
    const seen: Seen[] = [];
    setFetch((async (input: RequestInfo | URL, init?: RequestInit) => {
      const call = { method: init?.method ?? "GET", url: String(input) };
      seen.push(call);
      const path = call.url.split("?")[0];
      if (path === "/v1/labels") {
        return new Response(
          JSON.stringify({ lang: "fr", default_lang: "fr", revision: "1", spaces: {} }),
          { headers: { "Content-Type": "application/json" } },
        );
      }
      const body =
        call.method !== "GET"
          ? { state: "applied", secret: { id: "s", value: SECRET, expires_at: null } }
          : path === "/v1/identities/i"
            ? IDENTITY
            : path === "/v1/imports/m"
              ? IMPORT
              : { items: [], links: {} };
      return new Response(JSON.stringify(body), {
        status: call.method === "GET" ? 200 : 201,
        headers: { "Content-Type": "application/json", ETag: '"1"' },
      });
    }) as typeof fetch);
    await loadSession(async (url) =>
      url === "/v1/me"
        ? ({
            identity: "alice",
            kind: "person",
            global_permissions: permissions,
            bindings: [],
          } as unknown as Me)
        : null,
    );
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        // The Détail tab of the right panel, as the frame fills it (`meta.detail`).
        { path: "/detail/identity/:id", component: IdentityDetail, props: true },
        { path: "/detail/import/:id", component: ImportDetail, props: true },
        { path: "/", component: RouterView, children: frameRoutes },
      ],
    });
    await router.push(at);
    const root = document.createElement("div");
    document.body.append(root);
    app = createApp({ render: () => h(RouterView) });
    app.use(createShellI18n("fr")).use(router);
    app.mount(root);
    await router.isReady();
    await nextTick();
    return { seen, root, router };
  }

  /** The administration read first (its catalogue loaded), then the detail `at`. */
  async function detail(at: string, permissions: string[]) {
    const mount = await mounted("/admin/accounts", permissions);
    await expect.poll(() => button(mount.root, "Créer")).toBeDefined();
    await mount.router.push(at);
    return mount;
  }

  const button = (within: ParentNode, name: string) =>
    [...within.querySelectorAll<HTMLButtonElement>("button")].find((b) =>
      b.textContent?.trim().startsWith(name),
    );
  const reasonOf = (element: Element) =>
    document.getElementById(element.getAttribute("aria-describedby") ?? "")?.textContent;
  const writesSent = (seen: Seen[]) => seen.filter((s) => s.method !== "GET");
  const typed = (input: HTMLInputElement, text: string) => {
    input.value = text;
    input.dispatchEvent(new Event("input", { bubbles: true }));
  };
  const submitted = async (dialog: HTMLElement) => {
    dialog.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    await nextTick();
    await nextTick();
  };
  const dialogNow = () => document.querySelector<HTMLElement>("[role='dialog']");
  /** The name of the account and the motive: the first field and the last one. */
  const fields = (dialog: HTMLElement) => {
    const inputs = [...dialog.querySelectorAll<HTMLInputElement>("input")];
    return [inputs[0]!, inputs.at(-1)!] as const;
  };
  /** What the polite alerts of `within` say. */
  const said = (within: ParentNode) =>
    [...within.querySelectorAll(".k-inline-alert[role='status']")]
      .map((s) => s.textContent ?? "")
      .join(" ");

  // The lasting reason first (the permission lacking), then the gate: the key and the
  // palette say what the button says, in the four cases.
  const ADMIN = ["core.account.admin"];
  const READER = ["core.rbac.read"];
  const LACKING = "Il faut core.account.admin";
  const CASES = [
    { name: "nothing refused", permissions: ADMIN, width: "wide", says: null },
    { name: "the right alone", permissions: READER, width: "wide", says: LACKING },
    { name: "the width alone", permissions: ADMIN, width: "narrow", says: NARROW },
    { name: "both", permissions: READER, width: "narrow", says: LACKING },
  ] as const;
  const t = (key: string) => createShellI18n("fr").global.t(key);
  const writesNow = () => writesOf(timeOf({}), frameWidth.value);
  const statusOf = (root: HTMLElement) =>
    root.querySelector("span.k-visually-hidden[role='status']")?.textContent ?? "";

  for (const c of CASES) {
    it(`« Créer » and \`c\` of the service accounts say the same, ${c.name}`, async () => {
      const { root } = await mounted("/admin/accounts", [...c.permissions]);
      await expect.poll(() => button(root, "Créer")).toBeDefined();
      setFrameWidth(c.width);
      await nextTick();
      const create = button(root, "Créer")!;
      expect(play("create", writesNow(), t)).toBe(true);
      await nextTick();
      await nextTick();
      if (c.says === null) {
        expect(create.getAttribute("aria-disabled")).toBeNull();
        await expect.poll(dialogNow).not.toBeNull();
      } else {
        expect(create.getAttribute("aria-disabled")).toBe("true");
        expect(reasonOf(create)).toContain(c.says);
        if (c.says === LACKING) expect(reasonOf(create)).not.toContain(NARROW);
        expect(statusOf(root)).toBe(c.says);
        expect(dialogNow()).toBeNull();
      }
    });
  }

  it("keeps « Créer un compte de service » in the palette when the width alone holds it", async () => {
    const { root } = await mounted("/admin/accounts", ADMIN);
    await expect.poll(() => button(root, "Créer")).toBeDefined();
    const command = offered("create", timeOf({}), "narrow", t);
    expect(command?.label()).toBe("Créer un compte de service");
    command!.run();
    await nextTick();
    await nextTick();
    expect(statusOf(root)).toBe(NARROW);
    expect(dialogNow()).toBeNull();
  });

  it("takes « Créer un compte de service » out of the palette when the right lacks", async () => {
    const { root } = await mounted("/admin/accounts", READER);
    await expect.poll(() => button(root, "Créer")).toBeDefined();
    expect(offered("create", timeOf({}), "narrow", t)).toBeNull();
    expect(offered("create", timeOf({}), "wide", t)).toBeNull();
  });

  it("the creation of an account left open as the window narrows sends nothing", async () => {
    const { seen, root } = await mounted("/admin/accounts", ADMIN);
    await expect.poll(() => button(root, "Créer")).toBeDefined();
    button(root, "Créer")!.click();
    await expect.poll(() => dialogNow()?.querySelector("input")).toBeTruthy();
    const dialog = dialogNow()!;
    const [name, motive] = fields(dialog);
    typed(name, "sync");
    typed(motive, "la synchronisation");
    await nextTick();
    const send = () => button(dialog, "Créer")!;

    setFrameWidth("narrow");
    await nextTick();
    expect(reasonOf(send())).toContain(NARROW);
    send().click();
    await submitted(dialog);
    expect(writesSent(seen)).toEqual([]);
    expect(name.value).toBe("sync");
    expect(motive.value).toBe("la synchronisation");
    expect(dialogNow()).not.toBeNull();
    expect(said(dialog)).toContain(NARROW);

    setFrameWidth("standard");
    await nextTick();
    expect(said(dialog)).not.toContain(NARROW);
    await submitted(dialog);
    await expect
      .poll(() => writesSent(seen).map((s) => s.url.split("?")[0]))
      .toEqual(["/v1/service-accounts"]);
  });

  it("a secret shown stays readable as the window narrows, nothing made again", async () => {
    const { seen, root } = await mounted("/admin/accounts", ADMIN);
    await expect.poll(() => button(root, "Créer")).toBeDefined();
    button(root, "Créer")!.click();
    await expect.poll(() => dialogNow()?.querySelector("input")).toBeTruthy();
    const dialog = dialogNow()!;
    const [name, motive] = fields(dialog);
    typed(name, "sync");
    typed(motive, "la synchronisation");
    await nextTick();
    await submitted(dialog);
    await expect.poll(() => dialogNow()?.textContent ?? "").toContain(SECRET);

    setFrameWidth("narrow");
    await nextTick();
    // A read: the secret, its copy and « J'ai copié le secret », no reason of the width.
    expect(dialogNow()!.textContent).toContain(SECRET);
    expect(dialogNow()!.textContent).not.toContain(NARROW);
    const copied = button(dialogNow()!, "J'ai copié le secret")!;
    expect(copied.getAttribute("aria-disabled")).toBeNull();
    expect(writesSent(seen)).toHaveLength(1);
    copied.click();
    await expect.poll(dialogNow).toBeNull();
    expect(writesSent(seen)).toHaveLength(1);
  });

  it("the pseudonymisation confirmed as the window narrowed sends nothing", async () => {
    const { seen, root } = await detail("/detail/identity/i", [
      "core.account.admin",
      "core.identity.pseudonymise",
    ]);
    await expect.poll(() => button(root, "Pseudonymiser")).toBeDefined();
    button(root, "Pseudonymiser")!.click();
    await expect.poll(() => dialogNow()?.querySelector("input")).toBeTruthy();
    const dialog = dialogNow()!;
    const motive = dialog.querySelector<HTMLInputElement>("input")!;
    typed(motive, "demande de la personne");
    await nextTick();
    const send = () => button(dialog, "Pseudonymiser")!;

    setFrameWidth("narrow");
    await nextTick();
    // The toolbar refuses it too, after the permission it holds.
    expect(reasonOf(button(root, "Pseudonymiser")!)).toContain(NARROW);
    expect(reasonOf(send())).toContain(NARROW);
    send().click();
    await submitted(dialog);
    expect(writesSent(seen)).toEqual([]);
    expect(motive.value).toBe("demande de la personne");
    expect(dialogNow()).not.toBeNull();
    expect(said(dialog)).toContain(NARROW);

    setFrameWidth("standard");
    await nextTick();
    await submitted(dialog);
    await expect
      .poll(() => writesSent(seen).map((s) => s.url.split("?")[0]))
      .toEqual(["/v1/identities/i/pseudonymisation"]);
  });

  it("the state of an identity: the permission lacking said before the width", async () => {
    const { root } = await detail("/detail/identity/i", ["core.identity.pseudonymise"]);
    await expect.poll(() => button(root, "Désactiver")).toBeDefined();
    setFrameWidth("narrow");
    await nextTick();
    expect(reasonOf(button(root, "Désactiver")!)).toContain(LACKING);
    expect(reasonOf(button(root, "Désactiver")!)).not.toContain(NARROW);
    expect(reasonOf(button(root, "Pseudonymiser")!)).toContain(NARROW);
  });

  it("an import: its cancellation confirmed as the window narrowed sends nothing", async () => {
    const { seen, root } = await detail("/detail/import/m", ADMIN);
    await expect.poll(() => button(root, "Annuler l'import")).toBeDefined();
    expect(button(root, "Reprendre")!.getAttribute("aria-disabled")).toBeNull();
    button(root, "Annuler l'import")!.click();
    await expect.poll(() => document.querySelector("[role='alertdialog']")).not.toBeNull();

    setFrameWidth("narrow");
    await nextTick();
    expect(reasonOf(button(root, "Reprendre")!)).toContain(NARROW);
    expect(reasonOf(button(root, "Annuler l'import")!)).toContain(NARROW);
    const confirm = document.querySelector<HTMLElement>("[role='alertdialog']")!;
    button(confirm, "Annuler l'import")!.click();
    await nextTick();
    await nextTick();
    expect(writesSent(seen)).toEqual([]);
    await expect.poll(() => said(root)).toContain(NARROW);

    // Widened: the reason goes, « Reprendre » goes through.
    setFrameWidth("standard");
    await nextTick();
    expect(said(root)).not.toContain(NARROW);
    button(root, "Reprendre")!.click();
    await expect
      .poll(() => writesSent(seen).map((s) => s.url.split("?")[0]))
      .toEqual(["/v1/imports/m/resume"]);
  });
});
