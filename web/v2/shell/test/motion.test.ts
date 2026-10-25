// @vitest-environment happy-dom
import { DataGrid, SidePanel, applyMotion, currentMotion, type RowState } from "@kartenn/ui";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, h, nextTick, ref, type App } from "vue";

// The motion of @kartenn/ui in a document (spec 020, « Mouvement »): the user's setting on
// the root, the focus never waiting for the end of a transition, the highlight of a row
// that joins a draft. The library's own tests render on the server; these need a DOM.

let app: App | null = null;
afterEach(() => {
  app?.unmount();
  app = null;
  document.body.innerHTML = "";
  applyMotion("system");
});

function mount(render: () => ReturnType<typeof h>): HTMLElement {
  const root = document.createElement("div");
  document.body.append(root);
  app = createApp({ render });
  app.mount(root);
  return root;
}

describe("motion setting (SCR40)", () => {
  it("sets and removes data-motion on the root", () => {
    expect(currentMotion()).toBe("system");
    applyMotion("reduce");
    expect(document.documentElement.dataset.motion).toBe("reduce");
    expect(currentMotion()).toBe("reduce");
    applyMotion("system");
    expect(document.documentElement.hasAttribute("data-motion")).toBe(false);
  });
});

describe("SidePanel, animated", () => {
  const tabs = [{ value: "detail", label: "Détail" }];

  it("gives the focus back at once on closing, and to its tab at once on reopening", async () => {
    const open = ref(true);
    const root = mount(() =>
      h(SidePanel, {
        label: "Panneau",
        tabs,
        rail: true,
        open: open.value,
        "onUpdate:open": (value: boolean) => (open.value = value),
      }),
    );
    await nextTick();
    root.querySelector<HTMLElement>(".k-side-panel__close")!.focus();
    root.querySelector<HTMLElement>(".k-side-panel__close")!.click();
    await nextTick();
    await nextTick();
    // The strip has the focus while the panel may still be leaving.
    expect(document.activeElement?.getAttribute("aria-label")).toBe("Ouvrir le panneau");

    (document.activeElement as HTMLElement).click();
    await nextTick();
    await nextTick();
    expect(document.activeElement?.getAttribute("role")).toBe("tab");
  });
});

describe("DataGrid, a row that joins a draft", () => {
  type Row = { id: string; state?: RowState };
  const columns = [{ id: "id", label: "Id" }];

  it("is highlighted once, never at the first render nor when shown again", async () => {
    const rows = ref<Row[]>([{ id: "a", state: "draft" }, { id: "b" }]);
    const root = mount(() =>
      h(DataGrid, {
        label: "Grille",
        columns,
        rows: rows.value,
        rowKey: "id",
        rowState: (row: Record<string, unknown>) => row.state as RowState | undefined,
      }),
    );
    await nextTick();
    const row = (key: string) =>
      root.querySelector<HTMLElement>(`[data-row="${key}"]`)!.closest<HTMLElement>('[role="row"]')!;
    expect(root.querySelectorAll("[data-fresh]")).toHaveLength(0);

    rows.value = [
      { id: "a", state: "draft" },
      { id: "b", state: "draft" },
      { id: "c", state: "draft" },
    ];
    await nextTick();
    expect(row("b").dataset.fresh).toBe("true");
    expect(row("c").dataset.fresh).toBe("true");
    expect(row("a").hasAttribute("data-fresh")).toBe(false);

    // Its animation over, the mark goes.
    row("b").dispatchEvent(
      Object.assign(new Event("animationend"), { animationName: "k-row-fresh" }),
    );
    await nextTick();
    expect(row("b").hasAttribute("data-fresh")).toBe(false);

    // Filtered out, then shown again: no second highlight.
    rows.value = [{ id: "a", state: "draft" }];
    await nextTick();
    rows.value = [
      { id: "a", state: "draft" },
      { id: "b", state: "draft" },
    ];
    await nextTick();
    expect(row("b").hasAttribute("data-fresh")).toBe(false);
  });

  it("says a row removed in a draft in words", async () => {
    const root = mount(() =>
      h(DataGrid, {
        label: "Grille",
        columns,
        rows: [{ id: "a", state: "removed" }],
        rowKey: "id",
        rowState: (row: Record<string, unknown>) => row.state as RowState | undefined,
      }),
    );
    await nextTick();
    expect(root.querySelector('[data-row-state="removed"]')?.textContent).toContain(
      "retirée dans le brouillon",
    );
  });
});
