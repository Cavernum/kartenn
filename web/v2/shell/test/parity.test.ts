import { existsSync, readdirSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// The parity list of the switch (spec 021, task 26; REQ-021-31; design « Migration »),
// against the journeys: each test of the old shell (web/e2e/journeys) has its row in
// « Parcours de bout en bout » of web/v2/PARITE.md, and no row names a test that is gone;
// the pendant a row names under web/v2/e2e exists and holds the tests it quotes, and a
// ticked row names one; everywhere in the list, a row to port names its task (or
// « aucune tâche »), a withdrawn row its reference. The journeys still without a pendant
// are named, one todo each, and so is every test still to port. The switch (task 27)
// removes web/e2e, and this test with it.

const ROOT = new URL("../../../../", import.meta.url);
const read = (path: string) => readFileSync(new URL(path, ROOT), "utf8");

const OLD = "web/e2e/journeys/";
const NEW = "web/v2/e2e/";
const JOURNEYS = "Parcours de bout en bout";

type State = "x" | " " | "-";

interface Row {
  section: string;
  state: State;
  /** The cells after the state: old, new, what is left. */
  cells: string[];
}

/** The rows of the tables of `markdown` that start with a state, with their section. */
function rowsOf(markdown: string): Row[] {
  let section = "";
  const rows: Row[] = [];
  for (const line of markdown.split("\n")) {
    if (line.startsWith("## ")) section = line.slice(3).trim();
    const state = /^\|\s*\[([ x-])\]\s*\|/.exec(line)?.[1];
    if (state === undefined) continue;
    const cells = line
      .split("|")
      .slice(2, -1)
      .map((cell) => cell.trim());
    rows.push({ section, state: state as State, cells });
  }
  return rows;
}

/** The tests of a journey: `test("…"` or `test.fail("…"` at the start of a line. */
function testsOf(source: string): string[] {
  return [...source.matchAll(/^\s*test(?:\.fail)?\(\s*"([^"]+)"/gm)].map((m) => m[1]!);
}

const quoted = (cell: string) => [...cell.matchAll(/"([^"]+)"/g)].map((m) => m[1]!);
const specs = (cell: string, prefix: string) =>
  [...cell.matchAll(/`([^`]+\.spec\.ts)`/g)].map((m) => m[1]!).filter((p) => p.startsWith(prefix));

/** The old test a row of the journeys names: its file (from `OLD`) and its title. */
function oldTest(row: Row): string {
  const [file] = specs(row.cells[0] ?? "", OLD);
  const [title] = quoted(row.cells[0] ?? "");
  return `${file?.slice(OLD.length) ?? "?"} "${title ?? "?"}"`;
}

/**
 * The old journeys (file names) without a pendant: one of their rows at least is still to
 * port, and none names a journey under `NEW`, not even in part.
 */
function withoutPendant(rows: Row[]): string[] {
  const byFile = new Map<string, Row[]>();
  for (const row of rows) {
    const file = oldTest(row).split(" ")[0]!;
    byFile.set(file, [...(byFile.get(file) ?? []), row]);
  }
  return [...byFile]
    .filter(
      ([, of]) =>
        of.some((row) => row.state === " ") &&
        of.every((row) => specs(row.cells[1] ?? "", NEW).length === 0),
    )
    .map(([file]) => file)
    .sort();
}

/** The places a row to port, or withdrawn, fails to name. */
function unnamed(rows: Row[]): string[] {
  return rows.flatMap((row) => {
    const text = row.cells.join(" ");
    if (row.state === " " && !/\btâches? \d+[a-z]*\b|aucune tâche/.test(text)) {
      return [`${row.section}: ${row.cells[0]} names no task`];
    }
    if (row.state === "-" && !/REQ-\d{3}-\d+/.test(text)) {
      return [`${row.section}: ${row.cells[0]} names no reference`];
    }
    return [];
  });
}

const rows = rowsOf(read("web/v2/PARITE.md"));
const journeys = rows.filter((row) => row.section === JOURNEYS);
const files = readdirSync(new URL(OLD, ROOT)).filter((name) => name.endsWith(".spec.ts"));

describe("the parity list (web/v2/PARITE.md)", () => {
  it("has one row for each test of web/e2e/journeys, and none for a test gone", () => {
    expect(files.length).toBeGreaterThan(10);
    const tests = files.flatMap((file) =>
      testsOf(read(`${OLD}${file}`)).map((title) => `${file} "${title}"`),
    );
    expect(tests.length).toBeGreaterThan(30);
    expect(journeys.map(oldTest).sort()).toEqual(tests.sort());
  });

  it("a ticked row names its pendant; each pendant named exists and holds the tests quoted", () => {
    const failures = journeys.flatMap((row) => {
      const named = specs(row.cells[1] ?? "", NEW);
      if (row.state === "x" && named.length === 0) return [`${oldTest(row)}: no pendant`];
      const missing = named.filter((path) => !existsSync(new URL(path, ROOT)));
      if (missing.length > 0) return missing.map((path) => `${oldTest(row)}: ${path} missing`);
      const sources = named.map(read).join("\n");
      return quoted(row.cells[1] ?? "")
        .filter((title) => !sources.includes(`"${title}"`))
        .map((title) => `${oldTest(row)}: "${title}" in none of ${named.join(", ")}`);
    });
    expect(failures).toEqual([]);
  });

  it("a row to port names its task or « aucune tâche », a withdrawn row its reference", () => {
    expect(rows.length).toBeGreaterThan(journeys.length);
    expect(unnamed(rows)).toEqual([]);
  });

  it("names the journeys without a pendant", () => {
    // The check itself, on a list of four journeys: two ported in part (one test ported,
    // or one test in part), one withdrawn, one not ported at all.
    const list = rowsOf(
      [
        `## ${JOURNEYS}`,
        `| [x] | \`${OLD}a.spec.ts\` : "one" | \`${NEW}journeys/a.spec.ts\` : "one" | — |`,
        `| [ ] | \`${OLD}a.spec.ts\` : "two" | — | tâche 12 |`,
        `| [-] | \`${OLD}b.spec.ts\` : "three" | — | REQ-024-8 |`,
        `| [ ] | \`${OLD}c.spec.ts\` : "four" | — | aucune tâche |`,
        `| [ ] | \`${OLD}c.spec.ts\` : "five" | — | — |`,
        `| [ ] | \`${OLD}d.spec.ts\` : "six" | \`${NEW}journeys/d.spec.ts\` : "6" | tâche 14 |`,
      ].join("\n"),
    );
    expect(withoutPendant(list)).toEqual(["c.spec.ts"]);
    expect(unnamed(list)).toEqual([`${JOURNEYS}: \`${OLD}c.spec.ts\` : "five" names no task`]);
  });

  for (const file of withoutPendant(journeys)) {
    const left = [
      ...new Set(
        journeys.filter((row) => oldTest(row).startsWith(`${file} `)).map((row) => row.cells[2]),
      ),
    ].join(" ; ");
    it.todo(`${OLD}${file} has no pendant under ${NEW} (${left})`);
  }
  for (const row of journeys.filter((row) => row.state === " ")) {
    it.todo(`to port: ${oldTest(row)} (${row.cells[2]})`);
  }
});
