import { expect, test, type Locator, type Page } from "@playwright/test";
import {
  arm,
  benchPage,
  framed,
  heap,
  load,
  measured,
  Passes,
  quantile,
  scroll,
  settle,
  timed,
  type Budget,
} from "./measure.ts";
// The names the bench hosts share with the bench (no Vue in that module).
import { ANSWER_EVENT, lockedTarget, STATE_MEASURE } from "../../ui/src/demo/benchNames.ts";

// The component benches of @kartenn/ui (spec 020, REQ-020-29, design « Budgets »; B29):
// budgets = twice the medians of the prototype D-1. The trees (`Tree`, `TreeGrid`) on
// 10⁵ nodes, the grid on 10⁴ rows (hosts of `ui/src/demo/bench.ts` on /_ui/bancs/), the
// layers on their catalogue states. Five passes each, on a new page; the median is
// checked, the worst pass for a budget of « none ». Every measure prints
// `PASS|FAIL|INFO <measure>: <value> (limit <limit>) [the five passes]`; one budget
// exceeded fails the bench.

const PASSES = 5;
/** Presses of a key whose latency is read at its 75th percentile. */
const PRESSES = 100;
/** Moves of a node in a tree of 10⁵ rows open, the host answering each after 300 ms. */
const MOVES = 10;
/** Changes of `moveState` alone, their flush read at its 75th percentile. */
const STATE_CHANGES = 20;
/** The status region of the trees, where a refusal is said. */
const STATUS = ".k-tree-frame > [role='status'], .k-tree-grid-frame > [role='status']";

test.describe.configure({ mode: "serial" });

/** Presses `key` (Tab, Shift+Tab) until the focus is inside `target`, six times at most. */
async function tabTo(page: Page, target: Locator, key = "Tab"): Promise<void> {
  for (let i = 0; i < 6; i++) {
    if (await target.evaluate((el) => el.contains(document.activeElement))) return;
    await page.keyboard.press(key);
  }
  throw new Error(`${key}: the focus never came into ${target}`);
}

/** The row or cell of the focus, as the trees and the grid name them. */
const focusedNode = (page: Page) =>
  page.evaluate(() => {
    const el = document.activeElement as HTMLElement;
    return el.dataset.node ?? el.dataset.row ?? "";
  });

/** The 75th percentile of `PRESSES` presses of `key`, each until the focus moves. */
async function presses(page: Page, key: string): Promise<number> {
  const latencies: number[] = [];
  for (let i = 0; i < PRESSES; i++) {
    const { latency } = await timed(page, { on: ["keydown"], until: { focusMoved: true } }, () =>
      page.keyboard.press(key),
    );
    latencies.push(latency);
  }
  return quantile(latencies, 0.75);
}

/**
 * The 75th percentile of the flush of Vue after a change of `moveState` alone, as the host
 * measures it (`STATE_MEASURE`): `refusedKeys` over every row, and the window drawn again.
 */
async function stateFlush(page: Page): Promise<number> {
  await page.evaluate((name) => performance.clearMeasures(name), STATE_MEASURE);
  const change = page.getByRole("button", { name: "Changer l'état d'un nœud" });
  for (let i = 0; i < STATE_CHANGES; i++) {
    await change.click();
    await settle(page);
  }
  const flushes = await page.evaluate(
    (name) => performance.getEntriesByName(name).map((entry) => entry.duration),
    STATE_MEASURE,
  );
  expect(flushes).toHaveLength(STATE_CHANGES);
  return quantile(flushes, 0.75);
}

const treeBudgets = (name: string): Record<string, Budget> => ({
  first: { name: `${name} 10⁵: first row`, unit: "ms", limit: 300 },
  heapLoaded: { name: `${name} 10⁵: heap, loaded`, unit: "MB", limit: 40 },
  expand: { name: `${name}: open 1 000 children`, unit: "ms", limit: 90 },
  down: { name: `${name}: Down, p75 of ${PRESSES}`, unit: "ms", limit: 32 },
  scrollDropped: { name: `${name}: scroll 5 000 rows, dropped frames`, unit: "%", limit: 0.01 },
  scrollTasks: {
    name: `${name}: scroll 5 000 rows, long tasks (worst pass)`,
    unit: "",
    limit: 0,
    worst: true,
  },
  heapScrolled: { name: `${name} 10⁵: heap, after the scroll`, unit: "MB", limit: 50 },
  // B29: a tree of 10⁵ at most 5 % of dropped frames while it moves nodes, counted where
  // it works: from the paste to the pending row drawn, from the answer to its rendering.
  movesDropped: {
    name: `${name} 10⁵ open: ${MOVES} moves, dropped frames while it works`,
    unit: "%",
    limit: 0.05,
  },
  movesDroppedAll: {
    name: `${name} 10⁵ open: ${MOVES} moves, dropped frames over the whole run`,
    unit: "%",
  },
  movesTasks: { name: `${name} 10⁵ open: ${MOVES} moves, long tasks`, unit: "" },
  moveAsked: { name: `${name} 10⁵ open: Ctrl+V to the pending row, p75`, unit: "ms" },
  moveAnswered: { name: `${name} 10⁵ open: answer of the host drawn, p75`, unit: "ms" },
  moveLocked: { name: `${name} 10⁵ open: Ctrl+V on a locked target, refusal said`, unit: "ms" },
  flushFive: { name: `${name} 5 100 rows: moveState changed, flush p75`, unit: "ms" },
  flushAll: { name: `${name} 100 100 rows: moveState changed, flush p75`, unit: "ms" },
});

for (const component of ["Tree", "TreeGrid"] as const) {
  test(`${component}: 10⁵ nodes`, async ({ page }) => {
    const cdp = await benchPage(page);
    const passes = new Passes(treeBudgets(component));
    const role = component === "Tree" ? "tree" : "treegrid";
    for (let pass = 0; pass < PASSES; pass++) {
      await load(page, `/_ui/bancs/${component}`, "main button");
      // The model is built; the tree mounts at the click, all closed.
      const show = page.getByRole("button", { name: "Afficher l'arbre" });
      const first = await timed(page, { on: ["click"], until: { shown: "[data-node]" } }, () =>
        show.click(),
      );
      passes.add("first", first.latency);
      await settle(page);
      passes.add("heapLoaded", await heap(cdp));

      const tree = page.getByRole(role, { name: "Cent mille nœuds" });
      await tabTo(page, tree);
      expect(await focusedNode(page)).toBe("v1");
      const expand = await timed(
        page,
        { on: ["keydown"], until: { shown: "[data-node='v1-1']" } },
        () => page.keyboard.press("ArrowRight"),
      );
      passes.add("expand", expand.latency);
      passes.add("down", await presses(page, "ArrowDown"));
      expect(await focusedNode(page)).toBe(`v1-${PRESSES}`);

      // Five domains open: 5 100 rows, scrolled from the top over 5 s.
      await page.getByRole("button", { name: "Ouvrir cinq domaines" }).click();
      await settle(page);
      const scrolled = await scroll(page, `[role='${role}']`, 5_000, 5_000);
      expect(scrolled.rows).toBeGreaterThanOrEqual(5_000);
      passes.add("scrollDropped", scrolled.dropped);
      passes.add("scrollTasks", scrolled.longTasks);
      await settle(page);
      passes.add("heapScrolled", await heap(cdp));
      passes.add("flushFive", await stateFlush(page));

      // Every domain open (100 100 rows): the row of the focus cut, pasted on the next one;
      // a locked target refused by the check of the tree, then the next one asked; the host
      // refuses a part of what it is asked (`moveState` refused, said once).
      await page.getByRole("button", { name: "Tout ouvrir" }).click();
      await settle(page);
      await tabTo(page, tree, "Shift+Tab");
      const asked: number[] = [];
      const answered: number[] = [];
      const said: number[] = [];
      const moves = await framed(page, async () => {
        const windows: [number, number][] = [];
        for (let i = 0; i < MOVES; i++) {
          const node = await focusedNode(page);
          await page.keyboard.press("Control+x");
          await page.keyboard.press("ArrowDown");
          if (lockedTarget(await focusedNode(page))) {
            const refusal = await timed(
              page,
              { on: ["keydown"], until: { text: [STATUS, "Déplacement refusé"] } },
              () => page.keyboard.press("Control+v"),
            );
            said.push(refusal.latency);
            await page.keyboard.press("ArrowDown");
          }
          const pendingRow = `[data-node='${node}'][data-move='pending']`;
          // The answer listened to before the paste that sets it off.
          await arm(page, { on: [ANSWER_EVENT], until: { gone: pendingRow } }, "answer");
          const ask = await timed(page, { on: ["keydown"], until: { shown: pendingRow } }, () =>
            page.keyboard.press("Control+v"),
          );
          const answer = await measured(page, "answer");
          asked.push(ask.latency);
          answered.push(answer.latency);
          windows.push([ask.start, ask.end], [answer.start, answer.end]);
          await settle(page);
        }
        return windows;
      });
      passes.add("movesDropped", moves.windowDropped!);
      passes.add("movesDroppedAll", moves.dropped);
      passes.add("movesTasks", moves.longTasks);
      passes.add("moveAsked", quantile(asked, 0.75));
      passes.add("moveAnswered", quantile(answered, 0.75));
      passes.add("moveLocked", quantile(said, 0.5));
      passes.add("flushAll", await stateFlush(page));
    }
    passes.check();
  });
}

test("DataGrid: 10⁴ rows", async ({ page }) => {
  const cdp = await benchPage(page);
  const passes = new Passes({
    heap: { name: "DataGrid 10⁴: heap", unit: "MB", limit: 45 },
    down: { name: `DataGrid 10⁴: Down, p75 of ${PRESSES}`, unit: "ms", limit: 32 },
    edit: { name: "DataGrid 10⁴: edit committed", unit: "ms", limit: 30 },
    sort: { name: "DataGrid 10⁴: sort", unit: "ms", limit: 30 },
    filter: { name: "DataGrid 10⁴: filter", unit: "ms", limit: 60 },
  });
  for (let pass = 0; pass < PASSES; pass++) {
    await load(page, "/_ui/bancs/DataGrid", "[role='grid'] [data-row]");
    passes.add("heap", await heap(cdp));
    const grid = page.getByRole("grid", { name: "Dix mille préfixes" });
    await tabTo(page, grid);
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowRight");
    passes.add("down", await presses(page, "ArrowDown"));

    // The label of the row of the focus: Enter opens its editor, Enter commits.
    const row = await focusedNode(page);
    await page.keyboard.press("Enter");
    await expect(grid.getByRole("textbox")).toBeFocused();
    await page.keyboard.press("Control+a");
    await page.keyboard.type(`Renommé ${pass}`);
    const edit = await timed(
      page,
      {
        on: ["keydown"],
        until: { text: [`[data-row='${row}'][data-column='label']`, `Renommé ${pass}`] },
      },
      () => page.keyboard.press("Enter"),
    );
    passes.add("edit", edit.latency);

    // The first header: Enter sorts by prefix.
    await page.keyboard.press("Control+Home");
    const sort = await timed(
      page,
      {
        on: ["keydown"],
        until: { attr: ["[role='columnheader'][data-column='prefix']", "aria-sort", "ascending"] },
      },
      () => page.keyboard.press("Enter"),
    );
    passes.add("sort", sort.latency);

    // The field over the labels: one in five rows left.
    const filter = await timed(
      page,
      { on: ["input"], until: { attr: ["[role='grid']", "aria-rowcount", "2001"] } },
      () => page.getByRole("searchbox", { name: "Filtrer les libellés" }).fill("Supervision"),
    );
    passes.add("filter", filter.latency);
  }
  passes.check();
});

/** The layers, each on the `keyboard` state of its catalogue page: its opener, its key. */
const LAYERS = [
  { component: "Dialog", opener: ["button", "Nouveau préfixe"], key: "Enter", layer: "dialog" },
  { component: "Drawer", opener: ["button", "Modifier le site"], key: "Enter", layer: "dialog" },
  { component: "Popover", opener: ["button", "Héritée"], key: "Enter", layer: "dialog" },
  { component: "Menu", opener: ["button", "Actions"], key: "Enter", layer: "menu" },
  { component: "Select", opener: ["combobox", "Usage"], key: "ArrowDown", layer: "listbox" },
  { component: "Combobox", opener: ["combobox", "Groupe"], key: "Alt+ArrowDown", layer: "listbox" },
  {
    component: "CommandPalette",
    opener: ["button", "Ouvrir la palette"],
    key: "Enter",
    layer: "dialog",
  },
] as const;

test("layers: opened without a long task", async ({ page }) => {
  await benchPage(page);
  const budgets: Record<string, Budget> = {};
  for (const { component } of LAYERS) {
    budgets[`${component}-tasks`] = {
      name: `${component}: open, long tasks (worst pass)`,
      unit: "",
      limit: 0,
      worst: true,
    };
    budgets[`${component}-latency`] = { name: `${component}: open`, unit: "ms" };
  }
  const passes = new Passes(budgets);
  for (let pass = 0; pass < PASSES; pass++) {
    for (const { component, opener, key, layer } of LAYERS) {
      await load(page, `/_ui/c/${component}?etat=keyboard`, "[data-state-name]");
      await page.getByRole(opener[0], { name: opener[1], exact: true }).focus();
      const open = await timed(
        page,
        { on: ["keydown"], until: { shown: `[role='${layer}']` } },
        () => page.keyboard.press(key),
      );
      passes.add(`${component}-tasks`, open.longTasks);
      passes.add(`${component}-latency`, open.latency);
    }
  }
  passes.check();
});
