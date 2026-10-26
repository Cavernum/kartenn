<script setup lang="ts">
// E11 — Flux (design « Maquettes », row E11; 011, REQ-011-1, 2, 5, 14, 15, 20, 21, 24, 25;
// 010, REQ-010-11 to 15, 36, 38): the register of the declared flows, with typed filters,
// state, synthesis against the flow matrix and outcome; the proposal of new flows in a
// changeset, a form in a table (one row per flow, rows added on the fly, each row's
// verdict inline), the chosen row in the bottom panel (the elements used: ends developed
// into machines, interfaces, addresses with their environment and the values of the
// axes cited; and what the row gives against the flow matrix: verdict per address pair,
// decisive element and its text, the cell of the matrix in a small excerpt, conditions
// not met, outcome); the draft of the proposal with its simulated effect and the common
// footer; « Tester un flux », which evaluates without writing (QUERY /v1/verdicts) and
// leads to a prefilled row; the flows of a machine in its information tab, each naming
// its other end by a link to its card. Flows are « conformes » or « non conformes à la
// matrice de flux ». The verdicts are worked out here, from the rules of `donnees.ts`, in
// the order of REQ-010-11, so that every view tells the same story.
import {
  Button,
  CodeBadge,
  Cluster,
  DataGrid,
  EnvChip,
  FilterChip,
  Icon,
  InlineAlert,
  KeyLabel,
  MatrixGrid,
  Stack,
  StatusBadge,
  Tabs,
  TextField,
  Toolbar,
  type CellRef,
  type CellState,
  type GridColumn,
  type GridSort,
  type MatrixAxis,
  type MatrixCell,
  type RowState,
  type State,
  type TabItem,
  type ToolbarAction,
} from "@kartenn/ui";
import { computed, nextTick, ref, watch } from "vue";
import { RouterLink, useRoute, useRouter, type RouteLocationRaw } from "vue-router";
import AVenir from "./AVenir.vue";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import Soumission from "./EcranSoumission.vue";
import {
  CONDITION_JUSTIFICATION,
  DOMAIN,
  ENV,
  FLOWS,
  FLOW_TYPE,
  MACHINES,
  RULES,
  SERVICES,
  type EnvKey,
  type FlowAddress,
  type FlowEnd,
  type FlowRule,
  type FlowState,
  type FlowTypeKey,
  type RegisteredFlow,
} from "./donnees.ts";

const props = defineProps<{ vue: string }>();
const route = useRoute();
const router = useRouter();

// Links between the views keep the switches of the catalogue (mode, density, language).
const SWITCHES = ["mode", "densite", "langue"] as const;
function view(vue: string, extra: Record<string, string> = {}): RouteLocationRaw {
  const query: Record<string, string> = { vue, ...extra };
  for (const key of SWITCHES) {
    const value = route.query[key];
    if (typeof value === "string") query[key] = value;
  }
  return { path: "/_ui/ecrans/E11", query };
}
/** The card of a machine (the view `machine`), or of the prefix (E2). */
function cardOf(end: FlowEnd): RouteLocationRaw {
  if (end.kind === "prefix") {
    const mode = route.query.mode;
    return { path: "/_ui/ecrans/E2", query: typeof mode === "string" ? { mode } : {} };
  }
  return view("machine", { machine: end.machine });
}

// ——— Ends, addresses, verdicts (REQ-010-11, 13, 15, 36) ———

/** The name of an end: a machine, a machine and its interface, a prefix. */
const endName = (end: FlowEnd): string =>
  end.kind === "machine"
    ? end.machine
    : end.kind === "interface"
      ? `${end.machine} ${end.iface}`
      : end.value;
/** An address of an end, with the interface that carries it. */
interface EndAddress {
  address: FlowAddress;
  iface: string | null;
}
/** The addresses of an end at the date (REQ-010-13): a machine gives all of its own. */
function addressesOf(end: FlowEnd): EndAddress[] {
  if (end.kind === "prefix") return [{ address: end.address, iface: null }];
  const m = MACHINES[end.machine]!;
  return m.interfaces
    .filter((i) => end.kind === "machine" || i.name === end.iface)
    .flatMap((i) => i.addresses.map((address) => ({ address, iface: i.name })));
}

/** The state of a cell of the matrix, as E10 shows it. */
type CellVerdict = "allow" | "deny" | "conditional" | "default";
const CELL: Record<CellVerdict, { state: State; label: string }> = {
  allow: { state: "ok", label: "Autoriser" },
  deny: { state: "ko", label: "Interdire" },
  conditional: { state: "warn", label: "Sous motif" },
  default: { state: "neutral", label: "Refus par défaut" },
};
const covers = (rule: FlowRule, type: FlowTypeKey, source: EnvKey, destination: EnvKey) =>
  rule.flowTypes.includes(type) &&
  rule.sources.includes(source) &&
  (rule.destinations.length === 0
    ? destination === source
    : rule.destinations.includes(destination));
/** The rule that decides a cell, in the order of REQ-010-11 (deny, allow, conditional). */
function decide(type: FlowTypeKey, source: EnvKey, destination: EnvKey) {
  const rules = RULES.filter((r) => covers(r, type, source, destination));
  const rule =
    rules.find((r) => r.effect === "deny") ??
    rules.find((r) => r.effect === "allow" && !r.pattern) ??
    rules.find((r) => r.effect === "allow");
  const verdict: CellVerdict = !rule
    ? "default"
    : rule.effect === "deny"
      ? "deny"
      : rule.pattern
        ? "conditional"
        : "allow";
  return { rule, verdict };
}

type PairVerdict = "allowed" | "conditional" | "denied";
const PAIR: Record<PairVerdict, { state: State; label: string }> = {
  allowed: { state: "ok", label: "Autorisé" },
  conditional: { state: "warn", label: "Sous réserve" },
  denied: { state: "ko", label: "Refusé" },
};
/** The decisive element of a verdict: a rule, or a cause of refusal. */
interface Decisive {
  kind: "rule" | "cause";
  key: string;
  text: string;
  /** The combination of values that triggered it (REQ-010-12). */
  combination: string;
}
interface Pair extends Record<string, unknown> {
  key: string;
  source: EndAddress;
  destination: EndAddress;
  verdict: PairVerdict;
  decisive: Decisive;
  cell: { source: EnvKey; destination: EnvKey } | null;
}
const envWord = (env: EnvKey) => ENV[env].label.toLowerCase();
function evaluatePair(type: FlowTypeKey, source: EndAddress, destination: EndAddress): Pair {
  const key = `${source.address.value}>${destination.address.value}`;
  const [a, b] = [source.address, destination.address];
  if (!a.env || !b.env) {
    const lacking = (a.env ? b : a).value;
    return {
      key,
      source,
      destination,
      verdict: "denied",
      decisive: {
        kind: "cause",
        key: "unclassified",
        text: `${lacking} n'a aucune valeur sur l'axe Environnement (policy.environment), que citent les règles de ce type de flux : sans classement, la politique ne tranche pas.`,
        combination: `(${a.env ?? "—"}, ${b.env ?? "—"})`,
      },
      cell: null,
    };
  }
  const { rule, verdict } = decide(type, a.env, b.env);
  const cell = { source: a.env, destination: b.env };
  const combination = `(${a.env}, ${b.env})`;
  if (!rule)
    return {
      key,
      source,
      destination,
      verdict: "denied",
      decisive: {
        kind: "cause",
        key: "no_rule",
        text: `Aucune règle n'autorise le type ${FLOW_TYPE[type]} de ${envWord(a.env)} vers ${envWord(b.env)} : le refus par défaut s'applique.`,
        combination,
      },
      cell,
    };
  return {
    key,
    source,
    destination,
    verdict: verdict === "deny" ? "denied" : verdict === "conditional" ? "conditional" : "allowed",
    decisive: { kind: "rule", key: rule.key, text: rule.text, combination },
    cell,
  };
}

/** The synthesis of a flow (REQ-010-15), « sous réserve » for an `allowed` under reserve. */
type Synthesis = "allowed" | "conditional" | "partial" | "denied";
const SYNTHESIS: Record<Synthesis, { state: State; label: string }> = {
  allowed: { state: "ok", label: "Conforme" },
  conditional: { state: "warn", label: "Sous réserve" },
  partial: { state: "warn", label: "Partielle" },
  denied: { state: "ko", label: "Non conforme" },
};
/** The outcome (REQ-010-36), the most unfavourable of the pairs'. */
type Outcome = "automatic" | "review" | "noncompliant";
const OUTCOME: Record<Outcome, { state: State; label: string }> = {
  automatic: { state: "ok", label: "Automatique" },
  review: { state: "review", label: "Instruction" },
  noncompliant: { state: "ko", label: "Non conforme" },
};
const NEXT: Record<Outcome, string> = {
  automatic: "Enregistré à l'application du changeset, sans instruction.",
  review:
    "Proposition à instruire : un instructeur l'approuve avec une date de fin, ou la rejette avec son motif.",
  noncompliant:
    "Proposition à instruire, approuvable seulement avec une dérogation (flow.waiver.required) ; ou corriger la ligne avant l'envoi.",
};
/** A condition of eligibility and how it stands (REQ-010-36, 38). */
interface Condition {
  key: string;
  text: string;
  state: "unmet" | "unevaluated";
}
interface Evaluation {
  type: FlowTypeKey;
  pairs: Pair[];
  synthesis: Synthesis;
  outcome: Outcome;
  /** The pair whose verdict weighs most: a refusal, then a reserve. */
  decisive: Pair;
  conditions: Condition[];
  /** Three discarded rules at most, and why (REQ-010-12). */
  discarded: { key: string; why: string }[];
}
const WEIGHT: Record<PairVerdict, number> = { denied: 2, conditional: 1, allowed: 0 };
/**
 * Evaluates a flow (010): a verdict per pair of addresses, synthesis, outcome. `dossier`:
 * whether a justification is given; null without dossier (« Tester un flux »): the
 * conditions on the dossier are then not evaluated, and the outcome is never automatic.
 */
function evaluate(
  source: FlowEnd,
  destination: FlowEnd,
  service: string,
  dossier: boolean | null,
): Evaluation {
  const type = SERVICES[service]!.flowType;
  const pairs = addressesOf(source).flatMap((s) =>
    addressesOf(destination).map((d) => evaluatePair(type, s, d)),
  );
  const decisive = pairs.reduce((a, b) => (WEIGHT[b.verdict] > WEIGHT[a.verdict] ? b : a));
  const allowed = pairs.filter((p) => p.verdict !== "denied");
  const synthesis: Synthesis =
    allowed.length === 0
      ? "denied"
      : allowed.length < pairs.length
        ? "partial"
        : allowed.some((p) => p.verdict === "conditional")
          ? "conditional"
          : "allowed";
  const crosses = pairs.some((p) => p.cell && p.cell.source !== p.cell.destination);
  const conditions: Condition[] =
    crosses && dossier !== true
      ? [{ ...CONDITION_JUSTIFICATION, state: dossier === null ? "unevaluated" : "unmet" }]
      : [];
  const outcome: Outcome =
    synthesis === "denied" || synthesis === "partial"
      ? "noncompliant"
      : synthesis === "conditional" || conditions.length > 0 || dossier === null
        ? "review"
        : "automatic";
  const cell = decisive.cell;
  const discarded = cell
    ? RULES.filter((r) => r.flowTypes.includes(type) && r.key !== decisive.decisive.key)
        .filter((r) => !covers(r, type, cell.source, cell.destination))
        .slice(0, 3)
        .map((r) => ({
          key: r.key,
          why: !r.sources.includes(cell.source)
            ? `La source n'est pas en ${r.sources.map(envWord).join(", ")}.`
            : `La destination n'est pas en ${(r.destinations.length ? r.destinations : r.sources).map(envWord).join(", ")}.`,
        }))
    : [];
  return { type, pairs, synthesis, outcome, decisive, conditions, discarded };
}
const pairsAllowed = (e: Evaluation) => e.pairs.filter((p) => p.verdict !== "denied").length;
const pairsSentence = (e: Evaluation) =>
  `${pairsAllowed(e)} paire${pairsAllowed(e) > 1 ? "s" : ""} autorisée${pairsAllowed(e) > 1 ? "s" : ""} sur ${e.pairs.length}`;

// ——— The register (REQ-011-5, 14) ———

const STATE: Record<FlowState, { state: State; label: string }> = {
  requested: { state: "review", label: "Demandé" },
  active: { state: "ok", label: "Actif" },
  noncompliant: { state: "ko", label: "Non conforme" },
  waived: { state: "warn", label: "Dérogé" },
  expired: { state: "neutral", label: "Échu" },
};
interface RegisterRow extends Record<string, unknown> {
  key: string;
  flow: RegisteredFlow;
  source: string;
  destination: string;
  service: string;
  flowType: string;
  state: string;
  synthesis: Synthesis;
  outcome: Outcome;
  term: string;
}
const registerRows: RegisterRow[] = FLOWS.map((flow) => {
  const e = evaluate(flow.source, flow.destination, flow.service, true);
  return {
    key: flow.key,
    flow,
    source: endName(flow.source),
    destination: endName(flow.destination),
    service: flow.service,
    flowType: FLOW_TYPE[e.type],
    state: STATE[flow.state].label,
    synthesis: e.synthesis,
    outcome: e.outcome,
    term: flow.waiver ? `Dérogation ${flow.waiver}` : flow.end ? `Fin ${flow.end}` : "",
  };
});
const registerColumns: GridColumn[] = [
  { id: "source", label: "Source", width: "md", mono: true, sortable: true, filterable: true },
  {
    id: "destination",
    label: "Destination",
    width: "md",
    mono: true,
    sortable: true,
    filterable: true,
  },
  { id: "service", label: "Service", width: "lg", sortable: true, filterable: true, priority: 2 },
  { id: "flowType", label: "Type de flux", width: "md", filterable: true, priority: 3 },
  { id: "state", label: "État", width: "md", sortable: true, filterable: true },
  { id: "synthesis", label: "Synthèse", width: "md", sortable: true, filterable: true },
  { id: "outcome", label: "Issue", width: "md", filterable: true, priority: 2 },
  { id: "term", label: "Dérogation, échéance", width: "md", sortable: true, priority: 3 },
];
const registerSort = ref<GridSort | undefined>({ column: "source", direction: "ascending" });
const registerActions: ToolbarAction[] = [
  {
    id: "propose",
    label: "Proposer des flux",
    icon: "plus",
    variant: "primary",
    shortcut: ["c"],
    priority: 3,
  },
  { id: "test", label: "Tester un flux", icon: "search", priority: 2 },
  { id: "export", label: "Exporter", variant: "ghost" },
];
function onRegisterAction(id: string): void {
  if (id === "propose") void router.push(view("proposition"));
  else if (id === "test") void router.push(view("tester"));
}
const count = (state: FlowState) => FLOWS.filter((f) => f.state === state).length;

// ——— The proposal (REQ-011-1, 21): a form in a table ———

type ObjectKind = "service" | "port";
interface Proposal {
  key: string;
  source: FlowEnd | null;
  destination: FlowEnd | null;
  /** The object of the row: a service, or a transport/port pair that names one. */
  object: { kind: ObjectKind; value: string; service: string } | null;
  reference: string;
  justification: string;
}
const machineEnd = (machine: string): FlowEnd => ({ kind: "machine", machine });
const ifaceEnd = (machine: string, iface: string): FlowEnd => ({
  kind: "interface",
  machine,
  iface,
});
/** The row prefilled by « Déclarer ce flux » (the test of the view `tester`). */
const TESTED = {
  source: machineEnd("srv-web-02"),
  destination: machineEnd("srv-app-01"),
  object: { kind: "port" as const, value: "tcp/443", service: "https" },
};
const BASE: Proposal[] = [
  {
    key: "ligne-1",
    source: machineEnd("srv-web-02"),
    destination: ifaceEnd("srv-app-01", "eth0"),
    object: { kind: "service", value: "api-interne", service: "api-interne" },
    reference: "ticketing:CHG-2041",
    justification: "Appels du portail vers l'API métier",
  },
  {
    key: "ligne-2",
    source: machineEnd("srv-rec-03"),
    destination: ifaceEnd("srv-app-01", "eth0"),
    object: { kind: "service", value: "https", service: "https" },
    reference: "ticketing:CHG-2042",
    justification: "",
  },
  {
    key: "ligne-3",
    source: machineEnd("dev-web-01"),
    destination: machineEnd("srv-db-02"),
    object: { kind: "service", value: "postgres", service: "postgres" },
    reference: "ticketing:CHG-2043",
    justification: "Jeu de données pour les tests de charge",
  },
  {
    key: "ligne-4",
    source: machineEnd("srv-new-05"),
    destination: ifaceEnd("srv-app-01", "eth0"),
    object: { kind: "port", value: "tcp/8443", service: "api-interne" },
    reference: "ticketing:CHG-2044",
    justification: "Nouveau collecteur de journaux",
  },
];
const prefilled = route.query.depuis === "test";
const proposals = ref<Proposal[]>(
  prefilled
    ? [...BASE, { key: "ligne-5", ...TESTED, reference: "", justification: "" }]
    : [...BASE],
);
/** The evaluation of a row, once its source, destination and object are given. */
const evaluationOf = (p: Proposal): Evaluation | null =>
  p.source && p.destination && p.object
    ? evaluate(p.source, p.destination, p.object.service, p.justification !== "")
    : null;
interface ProposalRow extends Record<string, unknown> {
  key: string;
  proposal: Proposal;
  evaluation: Evaluation | null;
  source: string;
  destination: string;
  object: string;
  flowType: string;
  reference: string;
  justification: string;
  verdict: string;
  outcome: string;
}
const proposalRows = computed<ProposalRow[]>(() =>
  proposals.value.map((p) => {
    const evaluation = evaluationOf(p);
    return {
      key: p.key,
      proposal: p,
      evaluation,
      source: p.source ? endName(p.source) : "",
      destination: p.destination ? endName(p.destination) : "",
      object: p.object?.value ?? "",
      flowType: p.object ? FLOW_TYPE[SERVICES[p.object.service]!.flowType] : "",
      reference: p.reference,
      justification: p.justification,
      verdict: evaluation ? SYNTHESIS[evaluation.synthesis].label : "",
      outcome: evaluation ? OUTCOME[evaluation.outcome].label : "",
    };
  }),
);
const proposalColumns: GridColumn[] = [
  { id: "source", label: "Source", width: "md", mono: true },
  { id: "destination", label: "Destination", width: "md", mono: true },
  { id: "object", label: "Service ou port", width: "lg", priority: 2 },
  // The verdict before the fields of the dossier, in view without scrolling the grid; the
  // flow type of the row is in its cell of the matrix (bottom panel) and in the draft.
  { id: "verdict", label: "Verdict", width: "fill" },
  { id: "reference", label: "Référence", width: "md", mono: true, priority: 3 },
  { id: "justification", label: "Justification", width: "md", priority: 3 },
];
/** The chosen row, shown in the bottom panel. */
const chosen = ref<string[]>([prefilled ? "ligne-5" : "ligne-2"]);
const bottomOpen = ref(true);
watch(chosen, () => (bottomOpen.value = true));
const chosenRow = computed(() => proposalRows.value.find((r) => r.key === chosen.value[0]));
/** The focus of the grid; the static view edits the missing justification of row 2. */
const proposalActive = ref<CellRef | undefined>(
  prefilled ? { row: "ligne-5", column: "source" } : { row: "ligne-2", column: "justification" },
);
const editing = ref<CellRef | undefined>(
  prefilled || props.vue !== "proposition"
    ? undefined
    : { row: "ligne-2", column: "justification" },
);
/** Enter on a cell, or a click on a row, chooses it and opens the bottom panel. */
function choose(cell: CellRef): void {
  chosen.value = [cell.row];
  bottomOpen.value = true;
}
function onGridClick(event: MouseEvent): void {
  const target = event.target as HTMLElement;
  if (target.closest("a, button, input")) return;
  const row = target.closest<HTMLElement>("[data-row]")?.dataset.row;
  if (row) choose({ row, column: "source" });
}
let added = 0;
/** « Ajouter un flux »: an empty row at the end, the focus on its source. */
async function addRow(): Promise<void> {
  added += 1;
  const key = `nouvelle-${added}`;
  proposals.value = [
    ...proposals.value,
    { key, source: null, destination: null, object: null, reference: "", justification: "" },
  ];
  editing.value = undefined;
  proposalActive.value = { row: key, column: "source" };
  await nextTick();
  document
    .querySelector<HTMLElement>(`[role="gridcell"][data-row="${key}"][data-column="source"]`)
    ?.focus();
}
const proposalRowState = (): RowState => "draft";
const proposalCellState = (row: ProposalRow, column: GridColumn): CellState | undefined =>
  row.key === "ligne-2" && column.id === "justification" && !row.justification
    ? "draft"
    : undefined;
const BOTTOM: TabItem[] = [{ value: "ligne", label: "Flux choisi" }];
/** The flows of the proposal, counted by outcome (the simulated effect). */
const byOutcome = computed(() => {
  const counts: Record<Outcome, number> = { automatic: 0, review: 0, noncompliant: 0 };
  for (const row of proposalRows.value) if (row.evaluation) counts[row.evaluation.outcome] += 1;
  return counts;
});
const incomplete = computed(() => proposalRows.value.filter((r) => !r.evaluation).length);

/** The environments of the excerpt of the matrix: the usual three, and the cell's. */
const EXCERPT: EnvKey[] = ["prod", "rece", "deve"];
function excerptOf(cell: { source: EnvKey; destination: EnvKey }): MatrixAxis[] {
  const ids = [...new Set([...EXCERPT, cell.source, cell.destination])];
  return ids.map((id) => ({ id, label: ENV[id].label }));
}
const excerptActive = computed<MatrixCell | undefined>(() => {
  const cell = chosenRow.value?.evaluation?.decisive.cell;
  return cell ? { row: cell.source, column: cell.destination } : undefined;
});
const pairColumns: GridColumn[] = [
  { id: "source", label: "Adresse source", width: "md", mono: true },
  { id: "destination", label: "Adresse destination", width: "md", mono: true },
  { id: "verdict", label: "Verdict", width: "md" },
  { id: "decisive", label: "Élément décisif", width: "fill" },
];

// ——— « Tester un flux » (REQ-010-14, 38): no write ———

const tested = evaluate(TESTED.source, TESTED.destination, TESTED.object.service, null);
type TestObject = "service" | "port" | "type";
const testObject = ref<TestObject>("port");
const OBJECTS: { id: TestObject; label: string }[] = [
  { id: "service", label: "Service" },
  { id: "port", label: "Transport et port" },
  { id: "type", label: "Type de flux" },
];
/** The decisive elements of the test, each once, with the pairs it decided. */
const testDecisive = computed(() => {
  const seen = new Map<string, { decisive: Decisive; pairs: number; verdict: PairVerdict }>();
  for (const p of tested.pairs) {
    const known = seen.get(p.decisive.key);
    if (known) known.pairs += 1;
    else seen.set(p.decisive.key, { decisive: p.decisive, pairs: 1, verdict: p.verdict });
  }
  return [...seen.values()];
});
/** The addresses of the test, each with the values of the axes cited and their origin. */
const testAddresses = computed(() =>
  [
    ...addressesOf(TESTED.source).map((a) => ({ ...a, end: "Source", machine: "srv-web-02" })),
    ...addressesOf(TESTED.destination).map((a) => ({
      ...a,
      end: "Destination",
      machine: "srv-app-01",
    })),
  ].map((a) => ({ ...a, key: a.address.value })),
);
const resolvedColumns: GridColumn[] = [
  { id: "address", label: "Adresse", width: "md", mono: true },
  { id: "end", label: "Extrémité", width: "md", priority: 2 },
  { id: "env", label: "Environnement", width: "lg" },
  { id: "sensitivity", label: "Sensibilité", width: "fill", priority: 3 },
];
const declare = () => void router.push(view("proposition", { depuis: "test" }));

// ——— A machine's card: its flows in the information tab (REQ-011-15) ———

const machineKey = computed(() => {
  const asked = route.query.machine;
  return typeof asked === "string" && MACHINES[asked] ? asked : "srv-app-01";
});
const shownMachine = computed(() => MACHINES[machineKey.value]!);
const machineTab = ref("info");
const machineTabs = computed<TabItem[]>(() => [
  { value: "info", label: "Information" },
  { value: "interfaces", label: "Interfaces", count: shownMachine.value.interfaces.length },
  { value: "history", label: "Historique", count: 3 },
]);
const touches = (end: FlowEnd, machine: string) => end.kind !== "prefix" && end.machine === machine;
/** A flow of the machine: the other end, the service, the conformity. */
interface MachineFlow {
  key: string;
  flow: RegisteredFlow;
  own: FlowEnd;
  other: FlowEnd;
  synthesis: Synthesis;
}
function machineFlows(direction: "in" | "out"): MachineFlow[] {
  return FLOWS.filter((f) =>
    touches(direction === "in" ? f.destination : f.source, machineKey.value),
  ).map((flow) => ({
    key: flow.key,
    flow,
    own: direction === "in" ? flow.destination : flow.source,
    other: direction === "in" ? flow.source : flow.destination,
    synthesis: evaluate(flow.source, flow.destination, flow.service, true).synthesis,
  }));
}
const incoming = computed(() => machineFlows("in"));
const outgoing = computed(() => machineFlows("out"));
const addressCount = computed(() =>
  shownMachine.value.interfaces.reduce((n, i) => n + i.addresses.length, 0),
);
</script>

<template>
  <!-- The register of the flows. -->
  <Cadre
    v-if="props.vue === 'registre'"
    title="Flux"
    active="flows"
    space="flows"
    :crumbs="[{ label: 'Flux', to: '/x/flow' }, { label: 'Registre' }]"
  >
    <Panneau title="Flux" :detail="String(FLOWS.length)" main flush>
      <template #actions>
        <Toolbar
          label="Actions sur les flux"
          align="end"
          :actions="registerActions"
          @action="onRegisterAction"
        />
      </template>
      <template #toolbar>
        <Cluster gap="2">
          <FilterChip property="Domaine" operator="=" :value="DOMAIN.label" />
          <FilterChip property="Extrémité" operator="⊂" value="192.0.2.0/24" mono />
          <FilterChip property="Sorte" operator="=" value="Déclaré" />
          <FilterChip
            property="État"
            operator="∈"
            value="Demandé, Actif, Non conforme, Dérogé, Échu"
            remove-label="Retirer le filtre État"
          />
          <AVenir component="FilterPicker" task="19" inline>
            <Button variant="ghost" size="sm" icon="funnel" :shortcut="['/']"
              >Ajouter un filtre</Button
            >
          </AVenir>
        </Cluster>
      </template>
      <DataGrid
        v-model:sort="registerSort"
        label="Flux déclarés et ouverts, filtrés"
        :columns="registerColumns"
        :rows="registerRows"
        row-key="key"
        :filtered="['source', 'destination', 'state']"
      >
        <template #cell="{ row, column }">
          <RouterLink
            v-if="column.id === 'source' || column.id === 'destination'"
            :to="cardOf(column.id === 'source' ? row.flow.source : row.flow.destination)"
            >{{ row[column.id] }}</RouterLink
          >
          <KeyLabel
            v-else-if="column.id === 'service'"
            :value="row.service"
            :label="SERVICES[row.service]!.label"
            show-key
          />
          <StatusBadge
            v-else-if="column.id === 'state'"
            :state="STATE[row.flow.state].state"
            :label="STATE[row.flow.state].label"
          />
          <StatusBadge
            v-else-if="column.id === 'synthesis'"
            :state="SYNTHESIS[row.synthesis].state"
            :label="SYNTHESIS[row.synthesis].label"
          />
          <StatusBadge
            v-else-if="column.id === 'outcome'"
            :state="OUTCOME[row.outcome].state"
            :label="OUTCOME[row.outcome].label"
          />
          <span v-else-if="column.id === 'term' && !row.term" class="k-ecran-muted">—</span>
          <template v-else>{{ row[column.id] }}</template>
        </template>
      </DataGrid>
      <template #footer>
        <span class="k-num"
          >{{ FLOWS.length }} flux · {{ count("requested") }} demandé ·
          {{ count("noncompliant") }} non conforme à la matrice · {{ count("waived") }} dérogé</span
        >
        <span class="k-ecran-muted">Synthèse et issue lues contre la matrice de flux du jour</span>
      </template>
    </Panneau>
  </Cadre>

  <!-- The proposal: a form in a table, the chosen row in the bottom panel; `envoi`, its
       draft and the common footer. -->
  <Cadre
    v-else-if="props.vue === 'proposition' || props.vue === 'envoi'"
    v-model:bottom-open="bottomOpen"
    title="Proposer des flux"
    active="flows"
    space="flows"
    :draft="proposals.length"
    :panel="props.vue === 'envoi' ? 'draft' : undefined"
    :bottom="props.vue === 'proposition' ? BOTTOM : undefined"
    bottom-label="Flux choisi"
    :crumbs="[
      { label: 'Flux', to: '/x/flow' },
      { label: 'Registre', to: '/x/flow/flows' },
      { label: 'Proposer des flux' },
    ]"
  >
    <Panneau title="Proposer des flux" detail="une ligne par flux" main flush>
      <template #actions>
        <Button icon="plus" :shortcut="['n']" @click="addRow">Ajouter un flux</Button>
      </template>
      <DataGrid
        v-model:active="proposalActive"
        v-model:selected="chosen"
        label="Flux proposés dans le changeset"
        :columns="proposalColumns"
        :rows="proposalRows"
        row-key="key"
        selectable="single"
        :editing="editing"
        :row-state="proposalRowState"
        :cell-state="proposalCellState"
        @activate="choose"
        @click="onGridClick"
      >
        <template #cell="{ row, column, value }">
          <span v-if="!value && column.id !== 'verdict'" class="k-ecran-muted">—</span>
          <template v-else-if="column.id === 'object'">
            <KeyLabel
              v-if="row.proposal.object?.kind === 'service'"
              :value="row.proposal.object.service"
              :label="SERVICES[row.proposal.object.service]!.label"
              show-key
            />
            <span v-else class="k-e11-port"
              ><code>{{ row.proposal.object?.value }}</code
              ><Icon name="arrow-right" class="k-e11-muted-icon" /><code>{{
                row.proposal.object?.service
              }}</code></span
            >
          </template>
          <span v-else-if="column.id === 'verdict'" class="k-e11-verdict">
            <template v-if="row.evaluation">
              <StatusBadge
                :state="SYNTHESIS[row.evaluation.synthesis].state"
                :label="SYNTHESIS[row.evaluation.synthesis].label"
              />
              <CodeBadge :code="row.evaluation.decisive.decisive.key" />
            </template>
            <StatusBadge v-else state="neutral" label="À évaluer" />
          </span>
          <template v-else>{{ value }}</template>
        </template>
      </DataGrid>
      <template #footer>
        <span class="k-num"
          >{{ proposals.length }} flux · {{ byOutcome.automatic }} automatique ·
          {{ byOutcome.review }} à instruire · {{ byOutcome.noncompliant }} non conformes à la
          matrice<template v-if="incomplete"> · {{ incomplete }} à compléter</template></span
        >
        <span class="k-ecran-muted">Entrée : le flux choisi en bas</span>
      </template>
    </Panneau>

    <!-- The chosen row: the elements used, then what it gives against the matrix. -->
    <template #bottom-ligne>
      <Stack v-if="chosenRow" gap="3">
        <h2 class="k-ecran-subtitle k-e11-title">
          <span class="k-mono">{{ chosenRow.source || "—" }}</span>
          <Icon name="arrow-right" class="k-e11-muted-icon" />
          <span class="k-mono">{{ chosenRow.destination || "—" }}</span>
          <span v-if="chosenRow.object" class="k-ecran-muted">· {{ chosenRow.object }}</span>
        </h2>
        <InlineAlert
          v-if="!chosenRow.evaluation"
          tone="info"
          title="Ligne à compléter : source, destination et service (ou transport et port) l'évaluent contre la matrice de flux."
        />
        <div v-else class="k-e11-split">
          <Stack as="section" gap="2" aria-labelledby="k-e11-used">
            <h3 id="k-e11-used" class="k-ecran-subtitle">Éléments utilisés</h3>
            <div v-for="side in ['source', 'destination'] as const" :key="side" class="k-e11-end">
              <span class="k-ecran-muted">{{ side === "source" ? "Source" : "Destination" }}</span>
              <ul class="k-e11-tree">
                <li v-if="chosenRow.proposal[side]!.kind === 'prefix'">
                  <code>{{ endName(chosenRow.proposal[side]!) }}</code> préfixe
                </li>
                <li v-else>
                  <span class="k-e11-node">
                    <Icon name="server" class="k-e11-muted-icon" />
                    <RouterLink :to="cardOf(chosenRow.proposal[side]!)" class="k-mono">{{
                      MACHINES[(chosenRow.proposal[side] as { machine: string }).machine]!.key
                    }}</RouterLink>
                    <span class="k-ecran-muted">machine</span>
                  </span>
                  <ul>
                    <li
                      v-for="i in MACHINES[
                        (chosenRow.proposal[side] as { machine: string }).machine
                      ]!.interfaces.filter(
                        (i) =>
                          chosenRow!.proposal[side]!.kind === 'machine' ||
                          (chosenRow!.proposal[side] as { iface: string }).iface === i.name,
                      )"
                      :key="i.name"
                    >
                      <span class="k-e11-node">
                        <Icon name="interface" class="k-e11-muted-icon" />
                        <code>{{ i.name }}</code>
                        <span class="k-ecran-muted">{{ i.role }}</span>
                      </span>
                      <ul>
                        <li v-for="a in i.addresses" :key="a.value" class="k-e11-address">
                          <span class="k-e11-node">
                            <Icon name="address" class="k-e11-muted-icon" />
                            <code>{{ a.value }}</code>
                            <EnvChip v-if="a.env" v-bind="ENV[a.env]" />
                            <StatusBadge v-else state="warn" label="Sans environnement" />
                          </span>
                          <span class="k-ecran-muted k-e11-axes"
                            >Environnement {{ a.envFrom }} · Sensibilité {{ a.sensitivity }}
                            {{ a.sensitivityFrom }}</span
                          >
                        </li>
                      </ul>
                    </li>
                  </ul>
                </li>
              </ul>
            </div>
          </Stack>
          <Stack as="section" gap="2" aria-labelledby="k-e11-matrix">
            <h3 id="k-e11-matrix" class="k-ecran-subtitle">Contre la matrice de flux</h3>
            <Attributs
              :items="[
                { key: 'synthesis', label: 'Synthèse' },
                { key: 'outcome', label: 'Issue' },
                { key: 'decisive', label: 'Élément décisif' },
                { key: 'cell', label: 'Cellule' },
              ]"
            >
              <template #synthesis>
                <Cluster gap="2">
                  <StatusBadge
                    :state="SYNTHESIS[chosenRow.evaluation.synthesis].state"
                    :label="SYNTHESIS[chosenRow.evaluation.synthesis].label"
                  />
                  <span class="k-num k-ecran-muted">{{ pairsSentence(chosenRow.evaluation) }}</span>
                </Cluster>
              </template>
              <template #outcome>
                <StatusBadge
                  :state="OUTCOME[chosenRow.evaluation.outcome].state"
                  :label="OUTCOME[chosenRow.evaluation.outcome].label"
                />
              </template>
              <template #decisive>
                <Cluster gap="2">
                  <CodeBadge :code="chosenRow.evaluation.decisive.decisive.key" />
                  <span class="k-ecran-muted">{{
                    chosenRow.evaluation.decisive.decisive.kind === "rule" ? "règle" : "cause"
                  }}</span>
                </Cluster>
              </template>
              <template #cell>
                <span v-if="chosenRow.evaluation.decisive.cell" class="k-e11-node">
                  <EnvChip v-bind="ENV[chosenRow.evaluation.decisive.cell.source]" />
                  <Icon name="arrow-right" class="k-e11-muted-icon" />
                  <span>{{ FLOW_TYPE[chosenRow.evaluation.type] }}</span>
                  <Icon name="arrow-right" class="k-e11-muted-icon" />
                  <EnvChip v-bind="ENV[chosenRow.evaluation.decisive.cell.destination]" />
                </span>
                <span v-else class="k-ecran-muted">aucune : une extrémité sans environnement</span>
              </template>
            </Attributs>
            <p class="k-e11-text">{{ chosenRow.evaluation.decisive.decisive.text }}</p>
            <MatrixGrid
              v-if="chosenRow.evaluation.decisive.cell"
              :active="excerptActive"
              :label="`Extrait de la matrice ${FLOW_TYPE[chosenRow.evaluation.type]} : source en ligne, destination en colonne`"
              corner="Source → destination"
              :axes="{ rows: 'Source', columns: 'Destination' }"
              :rows="excerptOf(chosenRow.evaluation.decisive.cell)"
              :columns="excerptOf(chosenRow.evaluation.decisive.cell)"
              class="k-e11-excerpt"
            >
              <template #cell="{ row, column }">
                <StatusBadge
                  :state="
                    CELL[
                      decide(chosenRow.evaluation.type, row.id as EnvKey, column.id as EnvKey)
                        .verdict
                    ].state
                  "
                  :label="
                    CELL[
                      decide(chosenRow.evaluation.type, row.id as EnvKey, column.id as EnvKey)
                        .verdict
                    ].label
                  "
                />
              </template>
            </MatrixGrid>
            <DataGrid
              :label="`Verdict par paire d'adresses : ${pairsSentence(chosenRow.evaluation)}`"
              :columns="pairColumns"
              :rows="chosenRow.evaluation.pairs"
              row-key="key"
            >
              <template #cell="{ row, column }">
                <template v-if="column.id === 'source'">{{ row.source.address.value }}</template>
                <template v-else-if="column.id === 'destination'">{{
                  row.destination.address.value
                }}</template>
                <StatusBadge
                  v-else-if="column.id === 'verdict'"
                  :state="PAIR[row.verdict].state"
                  :label="PAIR[row.verdict].label"
                />
                <code v-else>{{ row.decisive.key }}</code>
              </template>
            </DataGrid>
            <Stack as="section" gap="1" aria-labelledby="k-e11-conditions">
              <h4 id="k-e11-conditions" class="k-e11-h4">Conditions non remplies</h4>
              <ul v-if="chosenRow.evaluation.conditions.length" class="k-e11-list">
                <li v-for="c in chosenRow.evaluation.conditions" :key="c.key">
                  <CodeBadge :code="c.key" /> {{ c.text }}
                </li>
              </ul>
              <p v-else class="k-ecran-muted">Aucune.</p>
            </Stack>
            <p class="k-e11-text">
              <strong>Suite : </strong>{{ NEXT[chosenRow.evaluation.outcome] }}
            </p>
          </Stack>
        </div>
      </Stack>
    </template>

    <!-- The draft of the proposal: the flows, the simulated effect, the common footer. -->
    <template #draft>
      <Stack gap="4">
        <Stack as="section" gap="2" aria-labelledby="k-e11-draft">
          <Cluster justify="between" :wrap="false">
            <h2 id="k-e11-draft" class="k-ecran-subtitle">Flux proposés</h2>
            <span class="k-ecran-muted k-num">{{ proposals.length }} créations</span>
          </Cluster>
          <Stack as="ol" gap="3" class="k-ecran-list">
            <li v-for="row in proposalRows" :key="row.key" class="k-e11-item">
              <Stack gap="1">
                <span class="k-e11-node k-mono">
                  {{ row.source || "—" }}
                  <Icon name="arrow-right" class="k-e11-muted-icon" />
                  <span class="k-visually-hidden">vers</span>
                  {{ row.destination || "—" }}
                </span>
                <Cluster gap="2">
                  <span v-if="row.object">{{ row.object }} · {{ row.flowType }}</span>
                  <template v-if="row.evaluation">
                    <StatusBadge
                      :state="SYNTHESIS[row.evaluation.synthesis].state"
                      :label="SYNTHESIS[row.evaluation.synthesis].label"
                    />
                    <CodeBadge :code="row.evaluation.decisive.decisive.key" />
                  </template>
                </Cluster>
              </Stack>
            </li>
          </Stack>
        </Stack>
        <Stack as="section" gap="2" aria-labelledby="k-e11-effect">
          <h2 id="k-e11-effect" class="k-ecran-subtitle">Effet simulé</h2>
          <Attributs
            :items="[
              {
                key: 'automatic',
                label: 'Enregistrés sans instruction',
                value: String(byOutcome.automatic),
                mono: true,
              },
              {
                key: 'review',
                label: 'À instruire',
                value: String(byOutcome.review),
                mono: true,
              },
              {
                key: 'noncompliant',
                label: 'Non conformes à la matrice',
                value: String(byOutcome.noncompliant),
                mono: true,
              },
            ]"
          />
          <InlineAlert
            tone="warn"
            :title="`${byOutcome.noncompliant} flux non conformes à la matrice de flux`"
          >
            Le changeset part en instruction : ces flux n'y sont approuvables qu'avec une
            dérogation. Les corriger, ou retirer leur ligne, avant l'envoi.
          </InlineAlert>
        </Stack>
      </Stack>
    </template>
    <template #draft-footer>
      <Soumission held policy="Instruction des flux" summary="Flux du portail et de la recette" />
    </template>
  </Cadre>

  <!-- « Tester un flux »: evaluates without writing. -->
  <Cadre
    v-else-if="props.vue === 'tester'"
    title="Tester un flux"
    active="flows"
    space="flows"
    :crumbs="[
      { label: 'Flux', to: '/x/flow' },
      { label: 'Registre', to: '/x/flow/flows' },
      { label: 'Tester un flux' },
    ]"
  >
    <Panneau title="Tester un flux" detail="sans écriture">
      <div class="k-e11-form">
        <TextField
          label="Source"
          model-value="srv-web-02"
          mono
          hint="Machine, interface, adresse ou préfixe"
        />
        <TextField
          label="Destination"
          model-value="srv-app-01"
          mono
          hint="Machine, interface, adresse ou préfixe"
        />
        <fieldset class="k-e11-object">
          <legend>Objet</legend>
          <Cluster gap="1">
            <Button
              v-for="o in OBJECTS"
              :key="o.id"
              size="sm"
              :pressed="testObject === o.id"
              @click="testObject = o.id"
              >{{ o.label }}</Button
            >
          </Cluster>
          <TextField label="Transport et port" model-value="tcp/443" mono hide-label />
        </fieldset>
        <TextField
          label="Référence du dossier"
          model-value=""
          mono
          placeholder="ticketing:CHG-…"
          hint="Facultatif : sans dossier, ses conditions ne sont pas évaluées"
        />
      </div>
      <Cluster gap="2">
        <Button variant="primary" icon="search" :shortcut="['Control', 'Enter']">Évaluer</Button>
        <span class="k-ecran-muted">Aucune écriture : la même évaluation qu'une déclaration.</span>
      </Cluster>
    </Panneau>
    <Panneau title="Résultat" detail="srv-web-02 → srv-app-01 · tcp/443" main>
      <template #actions>
        <Button variant="primary" icon="plus" @click="declare">Déclarer ce flux</Button>
      </template>
      <Stack gap="4">
        <Attributs
          columns
          :items="[
            { key: 'synthesis', label: 'Synthèse' },
            { key: 'outcome', label: 'Issue' },
            { key: 'service', label: 'Service' },
            { key: 'type', label: 'Type de flux', value: FLOW_TYPE[tested.type] },
            { key: 'at', label: 'Lu à', value: '2026-01-15, politique r42' },
          ]"
        >
          <template #synthesis>
            <Cluster gap="2">
              <StatusBadge
                :state="SYNTHESIS[tested.synthesis].state"
                :label="SYNTHESIS[tested.synthesis].label"
              />
              <span class="k-num k-ecran-muted">{{ pairsSentence(tested) }}</span>
            </Cluster>
          </template>
          <template #outcome>
            <StatusBadge
              :state="OUTCOME[tested.outcome].state"
              :label="OUTCOME[tested.outcome].label"
            />
          </template>
          <template #service>
            <span class="k-e11-port"
              ><code>tcp/443</code><Icon name="arrow-right" class="k-e11-muted-icon" /><KeyLabel
                value="https"
                :label="SERVICES.https!.label"
                show-key
            /></span>
          </template>
        </Attributs>
        <InlineAlert
          tone="warn"
          title="Avertissement"
          :items="[
            {
              message: `Synthèse partielle : ${pairsSentence(tested)}.`,
              code: 'flow.verdict.partial',
            },
          ]"
        />
        <Stack as="section" gap="2" aria-labelledby="k-e11-pairs">
          <h3 id="k-e11-pairs" class="k-ecran-subtitle">Verdict par paire d'adresses</h3>
          <DataGrid
            :label="`Verdict par paire d'adresses : ${pairsSentence(tested)}`"
            :columns="pairColumns"
            :rows="tested.pairs"
            row-key="key"
          >
            <template #cell="{ row, column }">
              <template v-if="column.id === 'source'">{{ row.source.address.value }}</template>
              <template v-else-if="column.id === 'destination'"
                >{{ row.destination.address.value }}
                <span class="k-ecran-muted">{{ row.destination.iface }}</span></template
              >
              <StatusBadge
                v-else-if="column.id === 'verdict'"
                :state="PAIR[row.verdict].state"
                :label="PAIR[row.verdict].label"
              />
              <span v-else class="k-e11-node"
                ><code>{{ row.decisive.key }}</code
                ><span class="k-ecran-muted">{{ row.decisive.combination }}</span></span
              >
            </template>
          </DataGrid>
        </Stack>
        <Stack as="section" gap="2" aria-labelledby="k-e11-why">
          <h3 id="k-e11-why" class="k-ecran-subtitle">Explication</h3>
          <ul class="k-e11-list">
            <li v-for="d in testDecisive" :key="d.decisive.key">
              <Cluster gap="2">
                <StatusBadge :state="PAIR[d.verdict].state" :label="PAIR[d.verdict].label" />
                <CodeBadge :code="d.decisive.key" />
                <span class="k-ecran-muted"
                  >{{ d.decisive.kind === "rule" ? "règle" : "cause" }}, combinaison
                  <code>{{ d.decisive.combination }}</code> · {{ d.pairs }} paires</span
                >
              </Cluster>
              <p class="k-e11-text">{{ d.decisive.text }}</p>
            </li>
          </ul>
          <h4 class="k-e11-h4">Valeurs résolues des axes cités</h4>
          <DataGrid
            label="Valeurs résolues des axes cités, par adresse"
            :columns="resolvedColumns"
            :rows="testAddresses"
            row-key="key"
          >
            <template #cell="{ row, column }">
              <template v-if="column.id === 'address'">{{ row.address.value }}</template>
              <template v-else-if="column.id === 'end'"
                >{{ row.end }}
                <span class="k-ecran-muted">{{ row.machine }} {{ row.iface }}</span></template
              >
              <span v-else-if="column.id === 'env'" class="k-e11-node">
                <EnvChip v-if="row.address.env" v-bind="ENV[row.address.env]" />
                <span class="k-ecran-muted">{{ row.address.envFrom }}</span>
              </span>
              <template v-else
                >{{ row.address.sensitivity }}
                <span class="k-ecran-muted">{{ row.address.sensitivityFrom }}</span></template
              >
            </template>
          </DataGrid>
          <h4 class="k-e11-h4">Règles écartées</h4>
          <ul class="k-e11-list">
            <li v-for="r in tested.discarded" :key="r.key">
              <code>{{ r.key }}</code> <span class="k-ecran-muted">{{ r.why }}</span>
            </li>
          </ul>
          <h4 class="k-e11-h4">Conditions</h4>
          <ul class="k-e11-list">
            <li v-for="c in tested.conditions" :key="c.key">
              <Cluster gap="2">
                <StatusBadge state="neutral" label="Non évaluée : sans dossier" />
                <CodeBadge :code="c.key" />
              </Cluster>
              <p class="k-e11-text">{{ c.text }}</p>
            </li>
          </ul>
        </Stack>
        <InlineAlert tone="warn" title="Suite attendue">
          Déclaré tel quel, ce flux part en instruction : les 2 paires vers l'interface
          d'administration de srv-app-01 n'y sont approuvables qu'avec une dérogation. Viser
          l'interface eth0 le rend conforme à la matrice de flux.
        </InlineAlert>
      </Stack>
    </Panneau>
  </Cadre>

  <!-- A machine's card: its flows in the information tab, the other end as a link. -->
  <Cadre
    v-else
    :title="`Machine ${shownMachine.key}`"
    active="machines"
    space="hosts"
    :crumbs="[
      { label: 'Machines', to: '/x/machines' },
      { label: 'Machines', to: '/x/machines/machines' },
      { label: shownMachine.key, mono: true },
    ]"
  >
    <Panneau
      :title="shownMachine.key"
      :detail="`machine · ${shownMachine.env ? ENV[shownMachine.env].label : 'sans environnement'} · ${shownMachine.site}`"
      main
    >
      <Tabs v-model="machineTab" :tabs="machineTabs" label="Sections de la fiche">
        <template #info>
          <Stack gap="4">
            <Attributs
              columns
              :items="[
                { key: 'env', label: 'Environnement déclaré' },
                { key: 'site', label: 'Site', value: shownMachine.site, mono: true },
                { key: 'owner', label: 'Propriétaire', value: shownMachine.owner },
                { key: 'system', label: 'Système', value: shownMachine.system },
                {
                  key: 'interfaces',
                  label: 'Interfaces',
                  value: shownMachine.interfaces.map((i) => `${i.name} (${i.role})`).join(', '),
                },
                { key: 'addresses', label: 'Adresses', value: String(addressCount), mono: true },
              ]"
            >
              <template #env>
                <EnvChip v-if="shownMachine.env" v-bind="ENV[shownMachine.env]" />
                <StatusBadge v-else state="warn" label="Sans environnement" />
              </template>
            </Attributs>
            <Stack as="section" gap="2" aria-labelledby="k-e11-flows">
              <Cluster gap="2" align="baseline">
                <h3 id="k-e11-flows" class="k-ecran-subtitle">Flux</h3>
                <span class="k-ecran-muted">conformité à la matrice de flux du jour</span>
              </Cluster>
              <div
                v-for="group in [
                  { id: 'in', label: 'Entrants', word: 'depuis', flows: incoming },
                  { id: 'out', label: 'Sortants', word: 'vers', flows: outgoing },
                ]"
                :key="group.id"
                class="k-e11-flows"
              >
                <h4 :id="`k-e11-flows-${group.id}`" class="k-e11-h4">
                  {{ group.label }}
                  <span class="k-ecran-muted k-num">{{ group.flows.length }}</span>
                </h4>
                <ul
                  v-if="group.flows.length"
                  class="k-e11-flow-list"
                  :aria-labelledby="`k-e11-flows-${group.id}`"
                >
                  <li v-for="f in group.flows" :key="f.key" class="k-e11-flow">
                    <span class="k-e11-flow__end">
                      <span class="k-ecran-muted">{{ group.word }}</span>
                      <RouterLink :to="cardOf(f.other)" class="k-mono">{{
                        endName(f.other)
                      }}</RouterLink>
                      <span v-if="f.own.kind === 'interface'" class="k-ecran-muted"
                        >par {{ f.own.iface }}</span
                      >
                    </span>
                    <KeyLabel
                      :value="f.flow.service"
                      :label="SERVICES[f.flow.service]!.label"
                      show-key
                    />
                    <span>{{ FLOW_TYPE[SERVICES[f.flow.service]!.flowType] }}</span>
                    <span class="k-e11-flow__state">
                      <StatusBadge
                        :state="SYNTHESIS[f.synthesis].state"
                        :label="SYNTHESIS[f.synthesis].label"
                      />
                      <StatusBadge
                        v-if="f.flow.state !== 'active'"
                        :state="STATE[f.flow.state].state"
                        :label="STATE[f.flow.state].label"
                      />
                    </span>
                  </li>
                </ul>
                <p v-else class="k-ecran-muted">Aucun flux {{ group.label.toLowerCase() }}.</p>
              </div>
            </Stack>
          </Stack>
        </template>
        <template #interfaces>
          <ul class="k-e11-list">
            <li v-for="i in shownMachine.interfaces" :key="i.name">
              <code>{{ i.name }}</code> {{ i.role }} ·
              <code>{{ i.addresses.map((a) => a.value).join(", ") }}</code>
            </li>
          </ul>
        </template>
        <template #history>
          <p class="k-ecran-muted">Versions de la machine et changesets.</p>
        </template>
      </Tabs>
    </Panneau>
  </Cadre>
</template>

<style>
@layer components {
  .k-e11-muted-icon {
    flex: none;
    color: var(--k-text-muted);
  }

  .k-e11-port,
  .k-e11-verdict,
  .k-e11-node,
  .k-e11-title {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--k-space-1);
    align-items: center;
    min-inline-size: 0;
  }

  /* In a cell of a grid, on one line (the cell clips, the row keeps its height). */
  .k-data-grid__cell .k-e11-verdict,
  .k-data-grid__cell .k-e11-port {
    flex-wrap: nowrap;
  }

  /* A flow of the draft tab: source → destination, then its verdict. */
  .k-e11-item {
    padding-block-end: var(--k-space-3);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  /* The bottom panel: the elements used, then the verdict, side by side when it holds. */
  .k-e11-split {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, var(--k-size-panel-md)), 1fr));
    gap: var(--k-space-4);
    align-items: start;
  }

  .k-e11-end {
    display: grid;
    gap: var(--k-space-1);
  }

  .k-e11-tree,
  .k-e11-tree ul {
    display: grid;
    gap: var(--k-space-1);
    list-style: none;
  }

  .k-e11-tree ul {
    padding-inline-start: var(--k-space-4);
    border-inline-start: var(--k-border-width) solid var(--k-border);
    margin-inline-start: var(--k-space-2);
  }

  .k-e11-address {
    display: grid;
  }

  .k-e11-axes {
    padding-inline-start: var(--k-space-5);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
  }

  .k-e11-text {
    max-inline-size: var(--k-measure-prose);
  }

  .k-e11-h4 {
    font-weight: var(--k-weight-semibold);
  }

  .k-e11-list {
    display: grid;
    gap: var(--k-space-2);
    list-style: none;
  }

  .k-e11-excerpt {
    max-inline-size: max-content;
  }

  /* The form of the test: fields side by side while the panel holds them. */
  .k-e11-form {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, var(--k-size-tile-min)), 1fr));
    gap: var(--k-space-3);
    align-items: start;
  }

  .k-e11-object {
    display: grid;
    gap: var(--k-space-1);
    min-inline-size: 0;
    padding: 0;
    border: 0;
  }

  .k-e11-object > legend {
    padding: 0;
    margin-block-end: var(--k-space-1);
  }

  /* The flows of a machine: a compact list, its columns aligned by a subgrid. */
  .k-e11-flows {
    display: grid;
    gap: var(--k-space-1);
    container-type: inline-size;
  }

  .k-e11-flow-list {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, max-content)) minmax(0, 1fr);
    gap: var(--k-space-1) var(--k-space-5);
    list-style: none;
  }

  .k-e11-flow {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    align-items: center;
    min-block-size: var(--k-row-height);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  .k-e11-flow__end,
  .k-e11-flow__state {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--k-space-1);
    align-items: center;
    min-inline-size: 0;
  }

  @container (width < 640px) {
    .k-e11-flow-list {
      grid-template-columns: minmax(0, 1fr);
    }

    .k-e11-flow {
      gap: var(--k-space-1);
      padding-block: var(--k-space-1);
    }
  }
}
</style>
