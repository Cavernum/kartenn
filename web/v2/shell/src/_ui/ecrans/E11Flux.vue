<script setup lang="ts">
// E11 — Flux (design « Maquettes », row E11; 011, REQ-011-1, 2, 5, 14, 15, 20, 21, 24 to
// 26, F21; 010, REQ-010-11 to 15, 36, 38). An end of a flow is an address, an interface
// or a prefix, never a machine: the machine is only the context of an interface. Views:
// the register of the declared flows (typed filters, state, conformity to the flow matrix
// and outcome, term); the proposal of new flows in a changeset, a form in a table (one row
// per flow, each row's verdict inline), the chosen row in the bottom panel, one subject
// per tab (its ends and their addresses, its cell of the matrix, its verdict per pair of
// addresses), each tab opening on the same one-line answer; the draft of the proposal;
// « Tester une connexion », which answers apart « ouvert » (a flow of the register,
// `active` or `waived`, covers the pair) and « conforme » (the matrix allows it), summed
// up in one of four cases, one view each, without writing; the flows of a machine in its
// information tab, by interface, each naming its other end — or, when the reader does
// not read that end's object, showing its kind and values without link nor panel, the
// card said not accessible with the permission it needs (027). Flows are « conformes » or
// « non conformes à la matrice de flux ». The verdicts are worked out here, from the rules
// of `donnees.ts`, in the order of REQ-010-11, so that every view tells the same story.
import {
  Button,
  CodeBadge,
  Cluster,
  DataGrid,
  EnvChip,
  FilterChip,
  Icon,
  IconButton,
  InlineAlert,
  KeyLabel,
  MatrixGrid,
  Menu,
  Stack,
  StatusBadge,
  Tabs,
  TextField,
  Toolbar,
  Tooltip,
  useTooltip,
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
  type TooltipControl,
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
  INTERFACE_MEDIA,
  INTERFACE_ROLE,
  MACHINES,
  RULES,
  SERVICES,
  addressEnd,
  ifaceEnd,
  type EnvKey,
  type FlowAddress,
  type FlowEnd,
  type FlowInterface,
  type InterfaceMedia,
  type InterfaceRole,
  type InterfaceVlan,
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

// ——— Ends and their addresses (REQ-011-2, REQ-010-13, 011/F21) ———

/** The machine and interface that carry an address. */
interface Owner {
  machine: string;
  iface: string;
}
function ownerOf(value: string): Owner | null {
  for (const m of Object.values(MACHINES))
    for (const i of m.interfaces)
      if (i.addresses.some((a) => a.value === value)) return { machine: m.key, iface: i.name };
  return null;
}
function addressOf(value: string): FlowAddress {
  const owner = ownerOf(value);
  const found = owner
    ? MACHINES[owner.machine]!.interfaces.find((i) => i.name === owner.iface)!.addresses.find(
        (a) => a.value === value,
      )
    : undefined;
  return (
    found ?? {
      value,
      env: null,
      envFrom: "aucune valeur",
      sensitivity: "—",
      sensitivityFrom: "aucune valeur",
    }
  );
}
/** The name of an end: an address, an interface (`machine · interface`), a prefix. */
const endName = (end: FlowEnd): string =>
  end.kind === "interface" ? `${end.machine} · ${end.iface}` : end.value;
const KIND: Record<FlowEnd["kind"], string> = {
  address: "Adresse",
  interface: "Interface",
  prefix: "Préfixe",
};
/** The machine an end belongs to (an interface's, an address's); none for a prefix. */
function machineOf(end: FlowEnd): string | null {
  if (end.kind === "interface") return end.machine;
  if (end.kind === "address") return ownerOf(end.value)?.machine ?? null;
  return null;
}
/** The card of a machine (the view `machine`). */
const machineCard = (machine: string): RouteLocationRaw => view("machine", { machine });
/** The card of an interface (the view `interface`): every mention of an interface leads there. */
const ifaceCard = (machine: string, iface: string): RouteLocationRaw =>
  view("interface", { machine, iface });
/**
 * The card of an end: an interface's own, the machine of an address, the prefix (E2); none
 * for a lone address.
 */
function cardOf(end: FlowEnd): RouteLocationRaw | null {
  if (end.kind === "prefix") {
    const mode = route.query.mode;
    return { path: "/_ui/ecrans/E2", query: typeof mode === "string" ? { mode } : {} };
  }
  if (end.kind === "interface") return ifaceCard(end.machine, end.iface);
  const machine = machineOf(end);
  return machine ? machineCard(machine) : null;
}
/** The interface that carries an address end, as context: « srv-db-02 · eth0 ». */
function contextOf(end: FlowEnd): string | null {
  if (end.kind !== "address") return null;
  const owner = ownerOf(end.value);
  return owner ? `${owner.machine} · ${owner.iface}` : null;
}
/** The card of the interface that carries an address end. */
function contextCard(end: FlowEnd): RouteLocationRaw {
  const owner = end.kind === "address" ? ownerOf(end.value) : null;
  return owner ? ifaceCard(owner.machine, owner.iface) : "";
}
const interfaceOf = (machine: string, name: string): FlowInterface | undefined =>
  MACHINES[machine]?.interfaces.find((i) => i.name === name);
/** The role of an interface end (Service, Administration…). */
const roleOf = (end: FlowEnd): string => {
  const i = end.kind === "interface" ? interfaceOf(end.machine, end.iface) : undefined;
  return i ? INTERFACE_ROLE[i.role] : "";
};
/**
 * The values of an end whose object the reader does not read (027): the address or the
 * prefix, the addresses an interface carries at the date; never the names of the
 * interface and of its machine, which come from the objects.
 */
const valuesOf = (end: FlowEnd): string[] =>
  end.kind === "interface"
    ? (interfaceOf(end.machine, end.iface)?.addresses.map((a) => a.value) ?? [])
    : [end.value];
/** The permission that opens the card of an end (027): its object's read. */
const READ: Record<FlowEnd["kind"], string> = {
  address: "core.address.read",
  interface: "machines.read",
  prefix: "core.prefix.read",
};
/**
 * Why an end has no link (retour du 2026-10-28): inline, its values and a lock alone; the
 * lock opens a tooltip (APG Tooltip, `useTooltip`: hover, focus, a tap; Escape closes)
 * that says « Fiche non accessible » and the permission. One tooltip per kind of end,
 * its text fixed, shared by every lock of that kind (each names it by `aria-describedby`).
 */
const KINDS: FlowEnd["kind"][] = ["address", "interface", "prefix"];
const UNREADABLE_TIP: Record<FlowEnd["kind"], TooltipControl> = {
  address: useTooltip(),
  interface: useTooltip(),
  prefix: useTooltip(),
};
const UNREADABLE_ON = Object.fromEntries(
  KINDS.map((kind) => [kind, UNREADABLE_TIP[kind].listeners(() => true)]),
) as Record<FlowEnd["kind"], Record<string, (event: Event) => void>>;
/** The source or the destination of a flow, by the id of its column. */
const endOf = (flow: RegisteredFlow, side: string): FlowEnd =>
  side === "source" ? flow.source : flow.destination;
/** An end whose card is not accessible: its kind and values, in one line. */
const unreadableName = (end: FlowEnd): string =>
  `${KIND[end.kind].toLowerCase()} ${valuesOf(end).join(", ")}`;
/** An address of an end, with the interface that carries it. */
interface EndAddress {
  address: FlowAddress;
  owner: Owner | null;
}
/** The addresses of an end at the date (REQ-010-13); a prefix is evaluated on its own. */
function addressesOf(end: FlowEnd): EndAddress[] {
  if (end.kind === "prefix") return [{ address: end.address, owner: null }];
  if (end.kind === "address") return [{ address: addressOf(end.value), owner: ownerOf(end.value) }];
  const i = MACHINES[end.machine]!.interfaces.find((x) => x.name === end.iface)!;
  return i.addresses.map((address) => ({
    address,
    owner: { machine: end.machine, iface: end.iface },
  }));
}
const ipv4 = (value: string) => value.split(".").reduce((n, part) => n * 256 + Number(part), 0);
/** Whether an end contains an address: one of its own, or within the prefix. */
function contains(end: FlowEnd, value: string): boolean {
  if (end.kind !== "prefix") return addressesOf(end).some((a) => a.address.value === value);
  const [network, length] = end.value.split("/");
  const base = ipv4(network!);
  return ipv4(value) >= base && ipv4(value) < base + 2 ** (32 - Number(length));
}

// ——— Verdicts (REQ-010-11, 12, 15, 36) ———

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
}
const WEIGHT: Record<PairVerdict, number> = { denied: 2, conditional: 1, allowed: 0 };
/**
 * Evaluates a flow (010): a verdict per pair of addresses, synthesis, outcome. `dossier`:
 * whether a justification is given; null without dossier (a test): the conditions on the
 * dossier are then not evaluated, and the outcome is never automatic.
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
  return { type, pairs, synthesis, outcome, decisive, conditions };
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
    source: flow.unreadable === "source" ? unreadableName(flow.source) : endName(flow.source),
    destination:
      flow.unreadable === "destination"
        ? unreadableName(flow.destination)
        : endName(flow.destination),
    service: flow.service,
    flowType: FLOW_TYPE[e.type],
    state: STATE[flow.state].label,
    synthesis: e.synthesis,
    outcome: e.outcome,
    term: flow.waiver ?? flow.end ?? "",
  };
});
// Widths that hold at 1440 px beside the expanded sidebar (the surface is about 1170 px):
// the conformity and the outcome share a column (the outcome of a non-conforming flow is
// non-conforming, said once), the term is a date whose kind the state tells (« Dérogé »:
// the end of the waiver). Under a container `lg`, the type and the term leave first.
const registerColumns: GridColumn[] = [
  // An end whose card the reader cannot open (027) says so under its values: the ends
  // wrap, the row grows.
  {
    id: "source",
    label: "Source",
    width: "md",
    mono: true,
    wrap: true,
    sortable: true,
    filterable: true,
  },
  {
    id: "destination",
    label: "Destination",
    width: "md",
    mono: true,
    wrap: true,
    sortable: true,
    filterable: true,
  },
  { id: "service", label: "Service", width: "md", sortable: true, filterable: true, priority: 2 },
  { id: "flowType", label: "Type de flux", width: "md", filterable: true, priority: 3 },
  { id: "state", label: "État", width: "md", sortable: true, filterable: true },
  { id: "synthesis", label: "Conformité, issue", width: "lg", sortable: true, filterable: true },
  { id: "term", label: "Échéance", width: "sm", mono: true, sortable: true, priority: 3 },
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
  { id: "test", label: "Tester une connexion", icon: "search", priority: 2 },
  { id: "export", label: "Exporter", variant: "ghost" },
];
function onRegisterAction(id: string): void {
  if (id === "propose") void router.push(view("proposition"));
  else if (id === "test") void router.push(view("tester"));
}
const count = (state: FlowState) => FLOWS.filter((f) => f.state === state).length;

// ——— « Tester une connexion » (REQ-011-26, REQ-010-14, 38): ouvert, conforme; no write ———

/** The four answers of a test (REQ-011-26): text, icon and colour. */
type TestCase = "open-conform" | "open-nonconform" | "closed-conform" | "closed-nonconform";
const CASE: Record<TestCase, { state: State; label: string }> = {
  "open-conform": { state: "ok", label: "Ouvert et conforme" },
  "open-nonconform": { state: "ko", label: "Ouvert mais non conforme" },
  "closed-conform": { state: "info", label: "Non ouvert mais conforme" },
  "closed-nonconform": { state: "neutral", label: "Non ouvert et non conforme" },
};
const OPEN = {
  true: { state: "info" as State, label: "Ouvert" },
  false: { state: "neutral" as State, label: "Non ouvert" },
};
const CONFORM = {
  true: { state: "ok" as State, label: "Conforme" },
  false: { state: "ko" as State, label: "Non conforme" },
};
/** A test of the mock-up: what is typed, what it resolves to. */
interface Test {
  view: string;
  case: TestCase;
  source: string;
  sourceEnd: FlowEnd;
  /** A machine typed as source: the interface or address to pick (011/F21). */
  pick?: string;
  destination: string;
  destinationEnd: FlowEnd;
  object: { kind: "service" | "port"; value: string; service: string };
}
const TESTS: Test[] = [
  {
    view: "tester",
    case: "open-conform",
    source: "192.0.2.20",
    sourceEnd: addressEnd("192.0.2.20"),
    destination: "srv-app-01 · eth0",
    destinationEnd: ifaceEnd("srv-app-01", "eth0"),
    object: { kind: "port", value: "tcp/443", service: "https" },
  },
  {
    view: "tester-derogation",
    case: "open-nonconform",
    source: "dev-web-01 · eth0",
    sourceEnd: ifaceEnd("dev-web-01", "eth0"),
    destination: "192.0.2.30",
    destinationEnd: addressEnd("192.0.2.30"),
    object: { kind: "service", value: "postgres", service: "postgres" },
  },
  {
    view: "tester-a-declarer",
    case: "closed-conform",
    source: "srv-web-02",
    sourceEnd: ifaceEnd("srv-web-02", "eth0"),
    pick: "srv-web-02",
    destination: "192.0.2.30",
    destinationEnd: addressEnd("192.0.2.30"),
    object: { kind: "port", value: "tcp/5432", service: "postgres" },
  },
  {
    view: "tester-ferme",
    case: "closed-nonconform",
    source: "srv-app-01 · eth0",
    sourceEnd: ifaceEnd("srv-app-01", "eth0"),
    destination: "srv-rec-03 · eth0",
    destinationEnd: ifaceEnd("srv-rec-03", "eth0"),
    object: { kind: "service", value: "https", service: "https" },
  },
];
const test = TESTS.find((t) => t.view === props.vue) ?? TESTS[0]!;
/** The test that « Déclarer ce flux » turns into a row of the proposal. */
const DECLARED = TESTS.find((t) => t.case === "closed-conform")!;
const OBJECTS = [
  { id: "service", label: "Service" },
  { id: "port", label: "Transport et port" },
  { id: "type", label: "Type de flux" },
] as const;
const testObject = ref<string>(test.object.kind);
const tested = evaluate(test.sourceEnd, test.destinationEnd, test.object.service, null);
/** The flows of the register that cover a pair: open (`active`, `waived`) or not. */
function coverOf(pair: Pair) {
  const covering = FLOWS.filter(
    (f) =>
      f.service === test.object.service &&
      contains(f.source, pair.source.address.value) &&
      contains(f.destination, pair.destination.address.value),
  );
  const opening = covering.filter((f) => f.state === "active" || f.state === "waived");
  return { opening, others: covering.filter((f) => !opening.includes(f)) };
}
interface TestPair extends Record<string, unknown> {
  key: string;
  pair: Pair;
  opening: RegisteredFlow[];
  open: boolean;
  conform: boolean;
}
const testPairs: TestPair[] = tested.pairs.map((pair) => {
  const { opening } = coverOf(pair);
  return {
    key: pair.key,
    pair,
    opening,
    open: opening.length > 0,
    conform: pair.verdict !== "denied",
  };
});
/** The flows that open the pairs, each once; the ones that cover them without opening. */
const openingFlows = [...new Set(testPairs.flatMap((p) => p.opening))];
const otherFlows = [...new Set(tested.pairs.flatMap((p) => coverOf(p).others))];
const allOpen = testPairs.every((p) => p.open);
const allConform = testPairs.every((p) => p.conform);
const testCase: TestCase = allOpen
  ? allConform
    ? "open-conform"
    : "open-nonconform"
  : allConform
    ? "closed-conform"
    : "closed-nonconform";
/** What the answer means and what to do (REQ-011-26, REQ-010-36). */
const testNext = computed(() => {
  const rule = tested.decisive.decisive.key;
  const flow = openingFlows[0];
  switch (testCase) {
    case "open-conform":
      return `Couvert par ${openingFlows.map((f) => f.key).join(", ")} et autorisé par ${rule} : rien à faire.`;
    case "open-nonconform":
      return flow?.state === "waived"
        ? `Ouvert par la dérogation de ${flow.key} jusqu'au ${flow.waiver}, alors que ${rule} l'interdit : recertifier la dérogation avant son échéance, ou clore le flux.`
        : `Ouvert par ${flow?.key}, que la politique interdit depuis : le flux est non conforme, à régulariser par une dérogation ou à clore.`;
    case "closed-conform":
      return `Aucun flux du registre ne couvre ces paires ; ${rule} les autorise : déclarer ce flux l'ouvre, après instruction si son issue le demande.`;
    default:
      return `Aucun flux du registre ne couvre ces paires et ${rule} les interdit : une déclaration serait non conforme, approuvable seulement avec une dérogation.`;
  }
});
const pairCount = `${testPairs.length} paire${testPairs.length > 1 ? "s" : ""} d'adresses`;
const testPairColumns: GridColumn[] = [
  { id: "source", label: "Adresse source", width: "md", mono: true },
  { id: "destination", label: "Adresse destination", width: "md", mono: true },
  { id: "open", label: "Ouvert", width: "lg" },
  { id: "conform", label: "Conforme", width: "fill" },
];
const declare = () => void router.push(view("proposition", { depuis: "test" }));

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
const BASE: Proposal[] = [
  {
    key: "ligne-1",
    source: ifaceEnd("srv-web-02", "eth0"),
    destination: ifaceEnd("srv-app-01", "eth0"),
    object: { kind: "service", value: "api", service: "api" },
    reference: "ticketing:CHG-2041",
    justification: "Appels du portail vers l'API métier",
  },
  {
    key: "ligne-2",
    source: ifaceEnd("srv-rec-03", "eth0"),
    destination: ifaceEnd("srv-app-01", "eth0"),
    object: { kind: "service", value: "https", service: "https" },
    reference: "ticketing:CHG-2042",
    justification: "",
  },
  {
    key: "ligne-3",
    source: ifaceEnd("dev-web-01", "eth0"),
    destination: ifaceEnd("srv-db-02", "eth0"),
    object: { kind: "service", value: "postgres", service: "postgres" },
    reference: "ticketing:CHG-2043",
    justification: "Jeu de données pour les tests de charge",
  },
  {
    key: "ligne-4",
    source: addressEnd("203.0.113.50"),
    destination: ifaceEnd("srv-app-01", "eth0"),
    object: { kind: "port", value: "tcp/8443", service: "api" },
    reference: "ticketing:CHG-2044",
    justification: "Nouveau collecteur de journaux",
  },
];
const prefilled = route.query.depuis === "test";
const proposals = ref<Proposal[]>(
  prefilled
    ? [
        ...BASE,
        {
          key: "ligne-5",
          source: DECLARED.sourceEnd,
          destination: DECLARED.destinationEnd,
          object: DECLARED.object,
          reference: "",
          justification: "",
        },
      ]
    : [...BASE],
);
/** The input refused by a cell (`row|column`), kept with its error until written again. */
const refused = ref<Record<string, { text: string; error: string }>>({});
const cellKey = (row: string, column: string) => `${row}|${column}`;
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
}
const proposalRows = computed<ProposalRow[]>(() =>
  proposals.value.map((p) => {
    const evaluation = evaluationOf(p);
    return {
      key: p.key,
      proposal: p,
      evaluation,
      // A refused input stays in its cell, with its error, until it is written again.
      source: p.source ? endName(p.source) : (refused.value[cellKey(p.key, "source")]?.text ?? ""),
      destination: p.destination
        ? endName(p.destination)
        : (refused.value[cellKey(p.key, "destination")]?.text ?? ""),
      object: p.object?.value ?? refused.value[cellKey(p.key, "object")]?.text ?? "",
      flowType: p.object ? FLOW_TYPE[SERVICES[p.object.service]!.flowType] : "",
      reference: p.reference,
      justification: p.justification,
      verdict: evaluation ? SYNTHESIS[evaluation.synthesis].label : "",
    };
  }),
);
const proposalColumns: GridColumn[] = [
  // The source carries the mark of the draft row: wider, so that an interface holds.
  { id: "source", label: "Source", width: "lg", mono: true },
  { id: "destination", label: "Destination", width: "md", mono: true },
  // The object leaves under a container `lg` before the verdict: the bottom panel names it.
  { id: "object", label: "Service ou port", width: "md", priority: 3 },
  // The verdict before the fields of the dossier, in view without scrolling the grid; the
  // flow type of the row is in the tab « Matrice » of the bottom panel and in the draft.
  // The verdict and the justification wrap rather than clip once the rows stack (`stack`,
  // under a container `sm`: the draft beside the grid, a phone): no label cut.
  { id: "verdict", label: "Verdict", width: "fill", wrap: true },
  { id: "reference", label: "Référence", width: "md", mono: true, priority: 3 },
  { id: "justification", label: "Justification", width: "md", priority: 3, wrap: true },
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
/*
 * Editing a cell, in the mockup's own state (no server): F2 or a double click opens the
 * editor of an editable cell, Enter or leaving it writes the row, Escape drops the input.
 * The row's evaluation, its verdict and the bottom panel follow the written values.
 */
type EditableColumn = "source" | "destination" | "object" | "reference" | "justification";
const EDITABLE: readonly string[] = [
  "source",
  "destination",
  "object",
  "reference",
  "justification",
] satisfies EditableColumn[];
const IPV4 = /^(?:(?:25[0-5]|2[0-4]\d|1?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|1?\d?\d)$/;
type PrefixEnd = Extract<FlowEnd, { kind: "prefix" }>;
const KNOWN_PREFIXES = FLOWS.flatMap((f) => [f.source, f.destination]).filter(
  (e): e is PrefixEnd => e.kind === "prefix",
);
/** An end typed in a cell: `machine · interface`, an address, a prefix; an error otherwise. */
function parseEnd(text: string): FlowEnd | { error: string } {
  const named = /^(\S+)\s*·\s*(\S+)$/.exec(text);
  if (named) {
    const [, machine, iface] = named;
    return interfaceOf(machine!, iface!)
      ? ifaceEnd(machine!, iface!)
      : { error: `Interface inconnue : ${text}.` };
  }
  if (IPV4.test(text)) return addressEnd(text);
  const [network, length] = text.split("/");
  if (network && IPV4.test(network) && length && /^\d+$/.test(length) && Number(length) <= 32)
    return (
      KNOWN_PREFIXES.find((p) => p.value === text) ?? {
        kind: "prefix",
        value: text,
        address: addressOf(text),
      }
    );
  return { error: "Une adresse, une interface (machine · interface) ou un préfixe." };
}
/** The object typed in a cell: a service key, or a transport/port pair of a service. */
function parseObject(text: string): NonNullable<Proposal["object"]> | { error: string } {
  if (SERVICES[text]) return { kind: "service", value: text, service: text };
  const service = Object.values(SERVICES).find((s) => s.ports.includes(text));
  if (service) return { kind: "port", value: text, service: service.key };
  return { error: `Ni un service ni un port connu : ${text}.` };
}
/** Writes the text of the editor to the row; an empty end or object empties the cell. */
function write(cell: CellRef, raw: string): void {
  const text = raw.trim();
  const column = cell.column as EditableColumn;
  const others = { ...refused.value };
  delete others[cellKey(cell.row, column)];
  let patch: Partial<Proposal> = {};
  if (column === "reference" || column === "justification") patch = { [column]: text };
  else if (text === "") patch = { [column]: null };
  else {
    const parsed = column === "object" ? parseObject(text) : parseEnd(text);
    if ("error" in parsed) {
      others[cellKey(cell.row, column)] = { text, error: parsed.error };
      patch = { [column]: null };
    } else patch = { [column]: parsed };
  }
  refused.value = others;
  proposals.value = proposals.value.map((p) => (p.key === cell.row ? { ...p, ...patch } : p));
}
const cellOf = (cell: CellRef) =>
  document.querySelector<HTMLElement>(
    `[role="gridcell"][data-row="${cell.row}"][data-column="${cell.column}"]`,
  );
async function startEditing(cell: CellRef): Promise<void> {
  if (!EDITABLE.includes(cell.column)) return;
  editing.value = cell;
  proposalActive.value = cell;
  await nextTick();
  const input = cellOf(cell)?.querySelector("input");
  input?.focus();
  input?.select();
}
/** Leaves the editor, writing its text or not; the focus goes back to the cell. */
async function stopEditing(input: HTMLInputElement, keep: boolean): Promise<void> {
  const cell = editing.value;
  if (!cell) return;
  editing.value = undefined;
  if (keep) write(cell, input.value);
  await nextTick();
  cellOf(cell)?.focus();
}
function onGridKeydown(event: KeyboardEvent): void {
  const target = event.target as HTMLElement;
  if (target instanceof HTMLInputElement && target.closest(".k-data-grid__editor")) {
    if (event.key === "Enter" || event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      void stopEditing(target, event.key === "Enter");
    }
    return;
  }
  const { row, column } = target.dataset;
  if (event.key === "F2" && row && column && target.getAttribute("role") === "gridcell")
    void startEditing({ row, column });
}
function onGridDblclick(event: MouseEvent): void {
  const target = event.target as HTMLElement;
  if (target.closest(".k-data-grid__editor")) return;
  const cell = target.closest<HTMLElement>('[role="gridcell"]');
  const { row, column } = cell?.dataset ?? {};
  if (row && column) void startEditing({ row, column });
}
/** Leaving the editor (a click elsewhere, Tab) writes it, as Enter does. */
function onGridFocusout(event: FocusEvent): void {
  const target = event.target as HTMLElement;
  if (target instanceof HTMLInputElement && target.closest(".k-data-grid__editor"))
    void stopEditing(target, true);
}
const proposalRowState = (): RowState => "draft";
const proposalCellState = (row: ProposalRow, column: GridColumn): CellState | undefined => {
  const refusal = refused.value[cellKey(row.key, column.id)];
  if (refusal) return { error: refusal.error };
  return row.key === "ligne-2" && column.id === "justification" && !row.justification
    ? "draft"
    : undefined;
};
/** One subject per tab of the bottom panel, each slot named `bottom-<value>`. */
const BOTTOM = computed<TabItem[]>(() => {
  const pairs = chosenRow.value?.evaluation?.pairs.length;
  return [
    { value: "extremites", label: "Extrémités" },
    { value: "matrice", label: "Matrice" },
    pairs === undefined
      ? { value: "verdicts", label: "Verdicts" }
      : { value: "verdicts", label: "Verdicts", count: pairs },
  ];
});
const BOTTOM_SLOTS = ["extremites", "matrice", "verdicts"].map((value) => ({
  value,
  slot: `bottom-${value}`,
}));
const SIDES = [
  { id: "source", label: "Source" },
  { id: "destination", label: "Destination" },
] as const;
/** The flows of the proposal, counted by outcome (the simulated effect). */
const byOutcome = computed(() => {
  const counts: Record<Outcome, number> = { automatic: 0, review: 0, noncompliant: 0 };
  for (const row of proposalRows.value) if (row.evaluation) counts[row.evaluation.outcome] += 1;
  return counts;
});
const incomplete = computed(() => proposalRows.value.filter((r) => !r.evaluation).length);
const EFFECT: { key: Outcome; label: string }[] = [
  { key: "automatic", label: "Enregistrés sans instruction" },
  { key: "review", label: "À instruire" },
  { key: "noncompliant", label: "Non conformes à la matrice" },
];

/**
 * The excerpt of the matrix: the environments of the decisive cell, and a neighbour when
 * both are the same; two by two at most, so that it holds in the panel.
 */
const NEIGHBOUR: EnvKey[] = ["rece", "prod", "deve"];
function excerptOf(cell: { source: EnvKey; destination: EnvKey }): MatrixAxis[] {
  const ids =
    cell.source === cell.destination
      ? [cell.source, NEIGHBOUR.find((env) => env !== cell.source)!]
      : [cell.source, cell.destination];
  return ids.map((id) => ({ id, label: ENV[id].label }));
}
const excerptActive = computed<MatrixCell | undefined>(() => {
  const cell = chosenRow.value?.evaluation?.decisive.cell;
  return cell ? { row: cell.source, column: cell.destination } : undefined;
});

// ——— A machine's card: the flows of its interfaces in the information tab (REQ-011-15) ———

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
/**
 * How an end reaches the machine (or only the shown interface, on its card): by one of its
 * interfaces, by an address of one, or by a prefix that contains one of its addresses; null
 * when it does not.
 */
function reach(end: FlowEnd): { by: string; through?: string } | null {
  const own = props.vue === "interface" ? [shownIface.value] : shownMachine.value.interfaces;
  if (end.kind === "interface")
    return end.machine === machineKey.value && own.some((i) => i.name === end.iface)
      ? { by: end.iface }
      : null;
  for (const i of own)
    for (const a of i.addresses)
      if (end.kind === "address" ? end.value === a.value : contains(end, a.value))
        return end.kind === "address"
          ? { by: i.name, through: a.value }
          : { by: i.name, through: end.value };
  return null;
}
/** A flow of the machine: by which interface, the other end, the service, the conformity. */
interface MachineFlow {
  key: string;
  flow: RegisteredFlow;
  by: { by: string; through?: string };
  other: FlowEnd;
  /** The reader does not read the other end's object: no link nor panel (027). */
  unreadable: boolean;
  synthesis: Synthesis;
}
function machineFlows(direction: "in" | "out"): MachineFlow[] {
  return FLOWS.flatMap((flow) => {
    const own = direction === "in" ? flow.destination : flow.source;
    const by = reach(own);
    if (!by) return [];
    return [
      {
        key: flow.key,
        flow,
        by,
        other: direction === "in" ? flow.source : flow.destination,
        unreadable: flow.unreadable === (direction === "in" ? "source" : "destination"),
        synthesis: evaluate(flow.source, flow.destination, flow.service, true).synthesis,
      },
    ];
  });
}
const incoming = computed(() => machineFlows("in"));
const outgoing = computed(() => machineFlows("out"));
const addressCount = computed(() =>
  shownMachine.value.interfaces.reduce((n, i) => n + i.addresses.length, 0),
);
const FLOW_GROUPS = computed(() => [
  { id: "in", label: "Entrants", word: "depuis", flows: incoming.value },
  { id: "out", label: "Sortants", word: "vers", flows: outgoing.value },
]);

// ——— An interface's card (009, REQ-009-6, 8, 11, 12, 26; 011, REQ-011-15) ———

const shownIface = computed<FlowInterface>(() => {
  const asked = route.query.iface;
  const all = shownMachine.value.interfaces;
  return all.find((i) => i.name === asked) ?? all[0]!;
});
const ifaceTab = ref("info");
const ifaceTabs: TabItem[] = [
  { value: "info", label: "Information" },
  { value: "history", label: "Historique", count: 3 },
];
/** The 802.1Q mode in words (REQ-009-12). */
function vlanOf(vlan: InterfaceVlan): string {
  if (vlan.mode === "none") return "Aucun (trames sans étiquette)";
  if (vlan.mode === "access") return `Accès · VLAN ${vlan.access}`;
  const native = vlan.native === undefined ? "sans natif" : `natif ${vlan.native}`;
  return `Trunk · étiquetés ${vlan.tagged.join(", ")} · ${native}`;
}
/**
 * The medium and the role, closed lists edited in place (a menu of their values, key and
 * label): a change goes to the draft, the value it replaces stays said.
 */
const media = ref<InterfaceMedia | null>(null);
const role = ref<InterfaceRole>("service");
watch(
  shownIface,
  (i) => {
    media.value = i.media;
    role.value = i.role;
  },
  { immediate: true },
);
const mediaChanged = computed(() => media.value !== shownIface.value.media);
const roleChanged = computed(() => role.value !== shownIface.value.role);
const choices = <K extends string>(labels: Record<K, string>, current: K | null) =>
  (Object.keys(labels) as K[]).map((key) => ({
    kind: "radio" as const,
    id: key,
    label: `${labels[key]} (${key})`,
    checked: key === current,
  }));
const mediaChoices = computed(() => choices(INTERFACE_MEDIA, media.value));
const roleChoices = computed(() => choices(INTERFACE_ROLE, role.value));
/** The operations of the draft: the frame's three, and each property changed here. */
const ifaceDraft = computed(() => 3 + Number(mediaChanged.value) + Number(roleChanged.value));
/** The versions of the interface (cheap: the last three). */
const IFACE_HISTORY = [
  {
    at: "2026-10-20",
    change: "Média posé : sfp (était ethernet)",
    changeset: "CHG-2031",
  },
  { at: "2026-09-02", change: "Adresse 192.0.2.14 rattachée", changeset: "CHG-1980" },
  {
    at: "2026-06-15",
    change: "Interface créée, rôle service, VLAN 120 en accès",
    changeset: "CHG-1702",
  },
];
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
          <template v-if="column.id === 'source' || column.id === 'destination'">
            <span v-if="row.flow.unreadable === column.id" class="k-e11-unreadable"
              ><span class="k-e11-unreadable__kind">{{
                KIND[endOf(row.flow, column.id).kind]
              }}</span>
              <span
                >{{ valuesOf(endOf(row.flow, column.id)).join(", ") }}
                <IconButton
                  icon="lock"
                  size="sm"
                  label="Fiche non accessible"
                  :aria-describedby="UNREADABLE_TIP[endOf(row.flow, column.id).kind].id"
                  v-on="UNREADABLE_ON[endOf(row.flow, column.id).kind]" /></span
            ></span>
            <RouterLink
              v-else-if="cardOf(column.id === 'source' ? row.flow.source : row.flow.destination)"
              :to="cardOf(column.id === 'source' ? row.flow.source : row.flow.destination)!"
              >{{ row[column.id] }}</RouterLink
            >
            <template v-else>{{ row[column.id] }}</template>
          </template>
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
          <span v-else-if="column.id === 'synthesis'" class="k-e11-verdict">
            <StatusBadge
              :state="SYNTHESIS[row.synthesis].state"
              :label="SYNTHESIS[row.synthesis].label"
            />
            <StatusBadge
              v-if="row.outcome !== 'noncompliant'"
              :state="OUTCOME[row.outcome].state"
              :label="OUTCOME[row.outcome].label"
            />
          </span>
          <span v-else-if="column.id === 'term' && !row.term" class="k-ecran-muted">—</span>
          <template v-else>{{ row[column.id] }}</template>
        </template>
      </DataGrid>
      <Tooltip v-for="kind in KINDS" :key="kind" :control="UNREADABLE_TIP[kind]"
        >Fiche non accessible · <code>{{ READ[kind] }}</code></Tooltip
      >
      <template #footer>
        <span class="k-num"
          >{{ FLOWS.length }} flux · {{ count("requested") }} demandé ·
          {{ count("noncompliant") }} non conforme à la matrice · {{ count("waived") }} dérogé</span
        >
        <span class="k-ecran-muted"
          >Échéance : fin de la dérogation d'un flux dérogé, sinon fin du flux</span
        >
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
        stack
        :editing="editing"
        :row-state="proposalRowState"
        :cell-state="proposalCellState"
        @activate="choose"
        @click="onGridClick"
        @dblclick="onGridDblclick"
        @keydown="onGridKeydown"
        @focusout="onGridFocusout"
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
        <span class="k-ecran-muted">Entrée : le flux choisi en bas · F2 : modifier</span>
      </template>
    </Panneau>

    <!-- The chosen row, one subject per tab; each opens on the same one-line answer. -->
    <template v-for="tab in BOTTOM_SLOTS" :key="tab.value" #[tab.slot]>
      <div v-if="chosenRow" class="k-e11-bottom">
        <div class="k-e11-answer">
          <h2 class="k-ecran-subtitle k-e11-title">
            <span class="k-mono">{{ chosenRow.source || "—" }}</span>
            <Icon name="arrow-right" class="k-e11-muted-icon" />
            <span class="k-visually-hidden">vers</span>
            <span class="k-mono">{{ chosenRow.destination || "—" }}</span>
            <span v-if="chosenRow.object" class="k-ecran-muted"
              >· {{ chosenRow.object }} · {{ chosenRow.flowType }}</span
            >
          </h2>
          <p v-if="chosenRow.evaluation" class="k-e11-verdict k-e11-summary">
            <StatusBadge
              :state="SYNTHESIS[chosenRow.evaluation.synthesis].state"
              :label="SYNTHESIS[chosenRow.evaluation.synthesis].label"
            />
            <StatusBadge
              v-if="chosenRow.evaluation.outcome !== 'noncompliant'"
              :state="OUTCOME[chosenRow.evaluation.outcome].state"
              :label="OUTCOME[chosenRow.evaluation.outcome].label"
            />
            <CodeBadge :code="chosenRow.evaluation.decisive.decisive.key" />
            <span class="k-num k-ecran-muted">{{ pairsSentence(chosenRow.evaluation) }}</span>
          </p>
        </div>
        <InlineAlert
          v-if="!chosenRow.evaluation"
          tone="info"
          title="Ligne à compléter : source, destination et service (ou transport et port) l'évaluent contre la matrice de flux."
        />

        <!-- Extrémités: each end, its kind, its context, its addresses and their values. -->
        <div v-else-if="tab.value === 'extremites'" class="k-e11-ends">
          <section
            v-for="side in SIDES"
            :key="side.id"
            class="k-e11-end"
            :aria-labelledby="`k-e11-end-${side.id}`"
          >
            <h3 :id="`k-e11-end-${side.id}`" class="k-e11-h4">
              {{ side.label }}
              <span class="k-ecran-muted">{{
                KIND[chosenRow.proposal[side.id]!.kind].toLowerCase()
              }}</span>
            </h3>
            <p class="k-e11-node">
              <RouterLink
                v-if="chosenRow.proposal[side.id]!.kind === 'interface'"
                :to="cardOf(chosenRow.proposal[side.id]!)!"
                class="k-mono"
                >{{ endName(chosenRow.proposal[side.id]!) }}</RouterLink
              >
              <template v-if="chosenRow.proposal[side.id]!.kind === 'prefix'">
                <span class="k-ecran-muted">évalué sur ses propres valeurs</span>
              </template>
              <template v-else-if="machineOf(chosenRow.proposal[side.id]!)">
                <span v-if="chosenRow.proposal[side.id]!.kind === 'address'" class="k-ecran-muted"
                  >sur
                  <RouterLink :to="contextCard(chosenRow.proposal[side.id]!)" class="k-mono">{{
                    contextOf(chosenRow.proposal[side.id]!)
                  }}</RouterLink>
                  ·</span
                >
                <span v-else class="k-ecran-muted"
                  >{{ roleOf(chosenRow.proposal[side.id]!) }} ·</span
                >
                <RouterLink :to="machineCard(machineOf(chosenRow.proposal[side.id]!)!)"
                  >fiche de {{ machineOf(chosenRow.proposal[side.id]!) }}</RouterLink
                >
              </template>
            </p>
            <ul class="k-e11-addresses">
              <li
                v-for="a in addressesOf(chosenRow.proposal[side.id]!)"
                :key="a.address.value"
                class="k-e11-address"
              >
                <span class="k-e11-node">
                  <code>{{ a.address.value }}</code>
                  <EnvChip v-if="a.address.env" v-bind="ENV[a.address.env]" />
                  <StatusBadge v-else state="warn" label="Sans environnement" />
                </span>
                <span class="k-ecran-muted k-e11-axes"
                  >Environnement {{ a.address.envFrom }} · Sensibilité {{ a.address.sensitivity }}
                  {{ a.address.sensitivityFrom }}</span
                >
              </li>
            </ul>
          </section>
        </div>

        <!-- Matrice: the decisive cell, its rule and text, a two-by-two excerpt. -->
        <div v-else-if="tab.value === 'matrice'" class="k-e11-matrix">
          <Attributs
            :items="[
              { key: 'cell', label: 'Cellule' },
              { key: 'decisive', label: 'Élément décisif' },
              {
                key: 'combination',
                label: 'Combinaison',
                value: chosenRow.evaluation.decisive.decisive.combination,
                mono: true,
              },
            ]"
          >
            <template #cell>
              <span v-if="chosenRow.evaluation.decisive.cell" class="k-e11-node">
                <EnvChip v-bind="ENV[chosenRow.evaluation.decisive.cell.source]" />
                <Icon name="arrow-right" class="k-e11-muted-icon" />
                <span class="k-visually-hidden">vers</span>
                <EnvChip v-bind="ENV[chosenRow.evaluation.decisive.cell.destination]" />
                <span class="k-ecran-muted">{{ FLOW_TYPE[chosenRow.evaluation.type] }}</span>
              </span>
              <span v-else class="k-ecran-muted">aucune : une adresse sans environnement</span>
            </template>
            <template #decisive>
              <span class="k-e11-node">
                <CodeBadge :code="chosenRow.evaluation.decisive.decisive.key" />
                <span class="k-ecran-muted">{{
                  chosenRow.evaluation.decisive.decisive.kind === "rule" ? "règle" : "cause"
                }}</span>
              </span>
            </template>
          </Attributs>
          <p class="k-e11-text">{{ chosenRow.evaluation.decisive.decisive.text }}</p>
          <MatrixGrid
            v-if="chosenRow.evaluation.decisive.cell"
            :active="excerptActive"
            :label="`Extrait de la matrice ${FLOW_TYPE[chosenRow.evaluation.type]} : source en ligne, destination en colonne`"
            corner="Source → dest."
            :axes="{ rows: 'Source', columns: 'Destination' }"
            :rows="excerptOf(chosenRow.evaluation.decisive.cell)"
            :columns="excerptOf(chosenRow.evaluation.decisive.cell)"
            class="k-e11-excerpt"
          >
            <template #cell="{ row, column }">
              <StatusBadge
                :state="
                  CELL[
                    decide(chosenRow.evaluation.type, row.id as EnvKey, column.id as EnvKey).verdict
                  ].state
                "
                :label="
                  CELL[
                    decide(chosenRow.evaluation.type, row.id as EnvKey, column.id as EnvKey).verdict
                  ].label
                "
              />
            </template>
          </MatrixGrid>
        </div>

        <!-- Verdicts: one line per pair of addresses, then the conditions and what follows. -->
        <div v-else class="k-e11-verdicts">
          <ul class="k-e11-pairs" aria-label="Verdict par paire d'adresses">
            <li v-for="p in chosenRow.evaluation.pairs" :key="p.key" class="k-e11-pair">
              <span class="k-e11-node k-mono"
                >{{ p.source.address.value
                }}<Icon name="arrow-right" class="k-e11-muted-icon" /><span
                  class="k-visually-hidden"
                  >vers</span
                >{{ p.destination.address.value }}</span
              >
              <StatusBadge :state="PAIR[p.verdict].state" :label="PAIR[p.verdict].label" />
              <CodeBadge :code="p.decisive.key" />
            </li>
          </ul>
          <Stack gap="1">
            <p class="k-e11-text">
              <strong>Conditions non remplies : </strong>
              <template v-if="chosenRow.evaluation.conditions.length">
                <template v-for="c in chosenRow.evaluation.conditions" :key="c.key">
                  <code>{{ c.key }}</code> — {{ c.text }}
                </template>
              </template>
              <template v-else>aucune.</template>
            </p>
            <p class="k-e11-text">
              <strong>Suite : </strong>{{ NEXT[chosenRow.evaluation.outcome] }}
            </p>
          </Stack>
        </div>
      </div>
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
            <li v-for="row in proposalRows" :key="row.key" class="k-e11-item k-ecran-draft-item">
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
          <!-- Counts: each label takes its width, the number at the end of its line. -->
          <dl class="k-e11-counts">
            <div v-for="item in EFFECT" :key="item.key" class="k-e11-counts__item">
              <dt>{{ item.label }}</dt>
              <dd class="k-num">{{ byOutcome[item.key] }}</dd>
            </div>
          </dl>
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

  <!-- « Tester une connexion »: ouvert and conforme, apart; no write. -->
  <Cadre
    v-else-if="props.vue.startsWith('tester')"
    title="Tester une connexion"
    active="flows"
    space="flows"
    :crumbs="[
      { label: 'Flux', to: '/x/flow' },
      { label: 'Registre', to: '/x/flow/flows' },
      { label: 'Tester une connexion' },
    ]"
  >
    <Panneau title="Tester une connexion" detail="sans écriture">
      <nav class="k-e11-cases" aria-label="Cas de la maquette">
        <span class="k-ecran-muted">Cas de la maquette :</span>
        <RouterLink
          v-for="t in TESTS"
          :key="t.view"
          :to="view(t.view)"
          :aria-current="t.view === test.view ? 'page' : undefined"
          class="k-e11-case"
          >{{ CASE[t.case].label }}</RouterLink
        >
      </nav>
      <div class="k-e11-form">
        <Stack gap="2">
          <TextField
            label="Source"
            :model-value="test.source"
            mono
            hint="Adresse, interface ou préfixe"
          />
          <fieldset v-if="test.pick" class="k-e11-object">
            <legend>{{ test.pick }} est une machine : choisir son interface ou une adresse</legend>
            <!-- The card of the interface beside its label, not in it: the radio keeps its
                 name, the link its own. -->
            <span v-for="i in MACHINES[test.pick]!.interfaces" :key="i.name" class="k-e11-choice">
              <label class="k-e11-choice">
                <input type="radio" name="k-e11-pick" checked />
                <code>{{ test.pick }} · {{ i.name }}</code>
                <span class="k-ecran-muted">{{ i.addresses.map((a) => a.value).join(", ") }}</span>
              </label>
              <RouterLink :to="ifaceCard(test.pick, i.name)"
                >fiche<span class="k-visually-hidden">
                  de l'interface {{ test.pick }} · {{ i.name }}</span
                ></RouterLink
              >
            </span>
            <template v-for="i in MACHINES[test.pick]!.interfaces" :key="`${i.name}-a`">
              <label v-for="a in i.addresses" :key="a.value" class="k-e11-choice">
                <input type="radio" name="k-e11-pick" />
                <code>{{ a.value }}</code>
                <span class="k-ecran-muted">{{ i.name }}</span>
              </label>
            </template>
          </fieldset>
        </Stack>
        <TextField
          label="Destination"
          :model-value="test.destination"
          mono
          hint="Adresse, interface ou préfixe"
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
          <TextField
            :label="test.object.kind === 'port' ? 'Transport et port' : 'Service'"
            :model-value="test.object.value"
            mono
            hide-label
          />
        </fieldset>
        <TextField
          label="Date"
          model-value="2026-01-15"
          mono
          hint="Ouvert et conforme à cette date"
        />
      </div>
      <Cluster gap="2">
        <Button variant="primary" icon="search" :shortcut="['Control', 'Enter']">Tester</Button>
        <span class="k-ecran-muted">Aucune écriture : registre et matrice lus à la date.</span>
      </Cluster>
    </Panneau>
    <Panneau
      title="Résultat"
      :detail="`${endName(test.sourceEnd)} → ${endName(test.destinationEnd)} · ${test.object.value}`"
      main
    >
      <template v-if="testCase === 'closed-conform'" #actions>
        <Button variant="primary" icon="plus" @click="declare">Déclarer ce flux</Button>
      </template>
      <Stack gap="4">
        <!-- The answer, on one line, then what it means. -->
        <div class="k-e11-answer">
          <p class="k-e11-verdict k-e11-summary">
            <StatusBadge :state="CASE[testCase].state" :label="CASE[testCase].label" />
            <span class="k-num k-ecran-muted"
              >{{ pairCount }} ·
              <KeyLabel
                :value="test.object.service"
                :label="SERVICES[test.object.service]!.label"
                show-key
              />
              · lu le 2026-01-15, politique r42</span
            >
          </p>
          <p class="k-e11-text">{{ testNext }}</p>
        </div>

        <!-- Two questions, two answers. -->
        <div class="k-e11-questions">
          <section class="k-e11-question" aria-labelledby="k-e11-open">
            <h3 id="k-e11-open" class="k-e11-question__title">
              <span>Ouvert ?</span>
              <StatusBadge
                :state="OPEN[String(allOpen) as 'true' | 'false'].state"
                :label="OPEN[String(allOpen) as 'true' | 'false'].label"
              />
            </h3>
            <p class="k-ecran-muted k-e11-text">
              Un flux du registre, actif ou dérogé, couvre-t-il chaque paire pour ce service ?
            </p>
            <ul v-if="openingFlows.length" class="k-e11-list">
              <li v-for="f in openingFlows" :key="f.key" class="k-e11-node">
                <RouterLink :to="view('registre')" class="k-mono">{{ f.key }}</RouterLink>
                <StatusBadge :state="STATE[f.state].state" :label="STATE[f.state].label" />
                <span class="k-ecran-muted"
                  ><template v-for="(e, n) in [f.source, f.destination]" :key="n"
                    ><template v-if="n"> → </template
                    ><RouterLink v-if="e.kind === 'interface'" :to="cardOf(e)!" class="k-mono">{{
                      endName(e)
                    }}</RouterLink
                    ><code v-else>{{ endName(e) }}</code></template
                  ><template v-if="f.waiver"> · dérogation jusqu'au {{ f.waiver }}</template></span
                >
              </li>
            </ul>
            <p v-else class="k-e11-text">
              Aucun flux actif ou dérogé ne couvre ces paires pour
              <code>{{ test.object.service }}</code
              >.
            </p>
            <ul v-if="otherFlows.length" class="k-e11-list">
              <li v-for="f in otherFlows" :key="f.key" class="k-e11-node">
                <RouterLink :to="view('registre')" class="k-mono">{{ f.key }}</RouterLink>
                <StatusBadge :state="STATE[f.state].state" :label="STATE[f.state].label" />
                <span class="k-ecran-muted">la couvre sans l'ouvrir</span>
              </li>
            </ul>
          </section>
          <section class="k-e11-question" aria-labelledby="k-e11-conform">
            <h3 id="k-e11-conform" class="k-e11-question__title">
              <span>Conforme ?</span>
              <StatusBadge
                :state="CONFORM[String(allConform) as 'true' | 'false'].state"
                :label="CONFORM[String(allConform) as 'true' | 'false'].label"
              />
            </h3>
            <p class="k-ecran-muted k-e11-text">
              La matrice de flux autorise-t-elle chaque paire ?
            </p>
            <p class="k-e11-node">
              <CodeBadge :code="tested.decisive.decisive.key" />
              <span class="k-ecran-muted">{{
                tested.decisive.decisive.kind === "rule" ? "règle décisive" : "cause"
              }}</span>
              <template v-if="tested.decisive.cell">
                <EnvChip v-bind="ENV[tested.decisive.cell.source]" />
                <Icon name="arrow-right" class="k-e11-muted-icon" />
                <span class="k-visually-hidden">vers</span>
                <EnvChip v-bind="ENV[tested.decisive.cell.destination]" />
                <span class="k-ecran-muted">{{ FLOW_TYPE[tested.type] }}</span>
              </template>
            </p>
            <p class="k-e11-text">{{ tested.decisive.decisive.text }}</p>
            <p v-if="allConform" class="k-e11-node">
              <span class="k-ecran-muted">Issue d'une déclaration, dossier non évalué :</span>
              <StatusBadge
                :state="OUTCOME[tested.outcome].state"
                :label="OUTCOME[tested.outcome].label"
              />
            </p>
          </section>
        </div>

        <!-- Per pair of addresses, the two answers. -->
        <Stack as="section" gap="2" aria-labelledby="k-e11-pairs">
          <h3 id="k-e11-pairs" class="k-ecran-subtitle">Par paire d'adresses</h3>
          <DataGrid
            :label="`Ouvert et conforme, par paire d'adresses : ${pairCount}`"
            :columns="testPairColumns"
            :rows="testPairs"
            row-key="key"
            stack
          >
            <template #cell="{ row, column }">
              <template v-if="column.id === 'source'">{{ row.pair.source.address.value }}</template>
              <template v-else-if="column.id === 'destination'">{{
                row.pair.destination.address.value
              }}</template>
              <span v-else-if="column.id === 'open'" class="k-e11-verdict">
                <StatusBadge
                  :state="OPEN[String(row.open) as 'true' | 'false'].state"
                  :label="OPEN[String(row.open) as 'true' | 'false'].label"
                />
                <code v-if="row.opening.length">{{
                  row.opening.map((f: RegisteredFlow) => f.key).join(", ")
                }}</code>
              </span>
              <span v-else class="k-e11-verdict">
                <StatusBadge
                  :state="CONFORM[String(row.conform) as 'true' | 'false'].state"
                  :label="CONFORM[String(row.conform) as 'true' | 'false'].label"
                />
                <code>{{ row.pair.decisive.key }}</code>
              </span>
            </template>
          </DataGrid>
        </Stack>
      </Stack>
    </Panneau>
  </Cadre>

  <!-- A machine's card, or one of its interfaces': the flows in the information tab. -->
  <Cadre
    v-else
    :title="
      props.vue === 'interface'
        ? `Interface ${shownMachine.key} · ${shownIface.name}`
        : `Machine ${shownMachine.key}`
    "
    active="machines"
    space="hosts"
    :draft="ifaceDraft"
    :crumbs="
      props.vue === 'interface'
        ? [
            { label: 'Machines', to: '/x/machines' },
            { label: 'Machines', to: '/x/machines/machines' },
            { label: shownMachine.key, to: `/x/machines/${shownMachine.key}`, mono: true },
            { label: shownIface.name, mono: true },
          ]
        : [
            { label: 'Machines', to: '/x/machines' },
            { label: 'Machines', to: '/x/machines/machines' },
            { label: shownMachine.key, mono: true },
          ]
    "
  >
    <!-- The card of an interface (009, REQ-009-6, 8, 11, 12, 26). -->
    <Panneau
      v-if="props.vue === 'interface'"
      :title="`${shownMachine.key} · ${shownIface.name}`"
      :detail="`interface · ${INTERFACE_ROLE[role].toLowerCase()} · ${media ? INTERFACE_MEDIA[media] : 'média non renseigné'}`"
      main
    >
      <Tabs v-model="ifaceTab" :tabs="ifaceTabs" label="Sections de la fiche">
        <template #info>
          <Stack gap="4">
            <Attributs
              columns
              :items="[
                { key: 'name', label: 'Nom', value: shownIface.name, mono: true },
                { key: 'machine', label: 'Machine' },
                { key: 'mac', label: 'MAC', value: shownIface.mac ?? '—', mono: true },
                { key: 'enabled', label: 'État' },
                { key: 'media', label: 'Média' },
                { key: 'role', label: 'Rôle' },
                { key: 'vlan', label: 'VLAN', value: vlanOf(shownIface.vlan) },
              ]"
            >
              <template #machine>
                <RouterLink :to="machineCard(shownMachine.key)" class="k-mono">{{
                  shownMachine.key
                }}</RouterLink>
              </template>
              <template #enabled>
                <StatusBadge
                  :state="shownIface.enabled ? 'ok' : 'neutral'"
                  :label="shownIface.enabled ? 'Activée' : 'Désactivée'"
                />
              </template>
              <template #media>
                <span class="k-e11-property">
                  <KeyLabel v-if="media" :value="media" :label="INTERFACE_MEDIA[media]" show-key />
                  <span v-else class="k-ecran-muted">non renseigné</span>
                  <Menu
                    label="Modifier le média"
                    :items="mediaChoices"
                    icon="pencil"
                    icon-only
                    variant="ghost"
                    size="sm"
                    @select="(id) => (media = id as InterfaceMedia)"
                  />
                  <template v-if="mediaChanged">
                    <StatusBadge state="neutral" label="En brouillon" />
                    <span class="k-ecran-muted"
                      >était
                      {{
                        shownIface.media ? INTERFACE_MEDIA[shownIface.media] : "non renseigné"
                      }}</span
                    >
                  </template>
                </span>
              </template>
              <template #role>
                <span class="k-e11-property">
                  <KeyLabel :value="role" :label="INTERFACE_ROLE[role]" show-key />
                  <Menu
                    label="Modifier le rôle"
                    :items="roleChoices"
                    icon="pencil"
                    icon-only
                    variant="ghost"
                    size="sm"
                    @select="(id) => (role = id as InterfaceRole)"
                  />
                  <template v-if="roleChanged">
                    <StatusBadge state="neutral" label="En brouillon" />
                    <span class="k-ecran-muted">était {{ INTERFACE_ROLE[shownIface.role] }}</span>
                  </template>
                </span>
              </template>
            </Attributs>
            <Stack as="section" gap="2" aria-labelledby="k-e11-addresses">
              <Cluster gap="2" align="baseline">
                <h3 id="k-e11-addresses" class="k-ecran-subtitle">Adresses rattachées</h3>
                <span class="k-ecran-muted k-num">{{ shownIface.addresses.length }}</span>
              </Cluster>
              <ul v-if="shownIface.addresses.length" class="k-e11-addresses">
                <li v-for="(a, n) in shownIface.addresses" :key="a.value" class="k-e11-address">
                  <span class="k-e11-node">
                    <code>{{ a.value }}</code>
                    <StatusBadge v-if="n === 0" state="info" label="Principale" />
                    <EnvChip v-if="a.env" v-bind="ENV[a.env]" />
                    <StatusBadge v-else state="warn" label="Sans environnement" />
                  </span>
                  <span class="k-ecran-muted k-e11-axes"
                    >Environnement {{ a.envFrom }} · Sensibilité {{ a.sensitivity }}
                    {{ a.sensitivityFrom }}</span
                  >
                </li>
              </ul>
              <p v-else class="k-ecran-muted">Aucune adresse rattachée.</p>
            </Stack>
            <Stack as="section" gap="2" aria-labelledby="k-e11-flows">
              <Cluster gap="2" align="baseline">
                <h3 id="k-e11-flows" class="k-ecran-subtitle">Flux de l'interface</h3>
                <span class="k-ecran-muted">conformité à la matrice de flux du jour</span>
              </Cluster>
              <div class="k-e11-flow-groups">
                <div v-for="group in FLOW_GROUPS" :key="group.id" class="k-e11-flows">
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
                        <span class="k-ecran-muted">par</span>
                        <code v-if="f.by.through">{{ f.by.through }}</code>
                        <span v-else>l'interface</span>
                      </span>
                      <span class="k-e11-flow__end">
                        <span class="k-ecran-muted">{{ group.word }}</span>
                        <template v-if="f.unreadable">
                          <span class="k-ecran-muted">{{ KIND[f.other.kind].toLowerCase() }}</span>
                          <code>{{ valuesOf(f.other).join(", ") }}</code>
                          <IconButton
                            icon="lock"
                            size="sm"
                            label="Fiche non accessible"
                            :aria-describedby="UNREADABLE_TIP[f.other.kind].id"
                            v-on="UNREADABLE_ON[f.other.kind]"
                          />
                        </template>
                        <RouterLink
                          v-else-if="cardOf(f.other)"
                          :to="cardOf(f.other)!"
                          class="k-mono"
                          >{{ endName(f.other) }}</RouterLink
                        >
                        <code v-else>{{ endName(f.other) }}</code>
                        <RouterLink
                          v-if="!f.unreadable && contextOf(f.other)"
                          :to="contextCard(f.other)"
                          class="k-mono"
                          >{{ contextOf(f.other) }}</RouterLink
                        >
                        <span
                          v-else-if="!f.unreadable && f.other.kind === 'prefix'"
                          class="k-ecran-muted"
                          >préfixe</span
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
              </div>
            </Stack>
          </Stack>
        </template>
        <template #history>
          <ol class="k-e11-list">
            <li v-for="h in IFACE_HISTORY" :key="h.at" class="k-e11-node">
              <span class="k-num">{{ h.at }}</span>
              <span>{{ h.change }}</span>
              <code>{{ h.changeset }}</code>
            </li>
          </ol>
        </template>
      </Tabs>
    </Panneau>

    <!-- A machine's card: the flows of its interfaces in the information tab. -->
    <Panneau
      v-else
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
                { key: 'interfaces', label: 'Interfaces' },
                { key: 'addresses', label: 'Adresses', value: String(addressCount), mono: true },
              ]"
            >
              <template #env>
                <EnvChip v-if="shownMachine.env" v-bind="ENV[shownMachine.env]" />
                <StatusBadge v-else state="warn" label="Sans environnement" />
              </template>
              <template #interfaces>
                <span class="k-e11-ifaces">
                  <span v-for="i in shownMachine.interfaces" :key="i.name">
                    <RouterLink :to="ifaceCard(shownMachine.key, i.name)" class="k-mono">{{
                      i.name
                    }}</RouterLink>
                    <span class="k-ecran-muted">
                      {{ i.media ? INTERFACE_MEDIA[i.media] : "—" }} ·
                      {{ INTERFACE_ROLE[i.role].toLowerCase() }}</span
                    >
                  </span>
                </span>
              </template>
            </Attributs>
            <Stack as="section" gap="2" aria-labelledby="k-e11-flows">
              <Cluster gap="2" align="baseline">
                <h3 id="k-e11-flows" class="k-ecran-subtitle">Flux de ses interfaces</h3>
                <span class="k-ecran-muted">conformité à la matrice de flux du jour</span>
              </Cluster>
              <div class="k-e11-flow-groups">
                <div v-for="group in FLOW_GROUPS" :key="group.id" class="k-e11-flows">
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
                        <RouterLink :to="ifaceCard(shownMachine.key, f.by.by)" class="k-mono">{{
                          f.by.by
                        }}</RouterLink>
                        <span v-if="f.by.through" class="k-ecran-muted k-mono">{{
                          f.by.through
                        }}</span>
                      </span>
                      <span class="k-e11-flow__end">
                        <span class="k-ecran-muted">{{ group.word }}</span>
                        <template v-if="f.unreadable">
                          <span class="k-ecran-muted">{{ KIND[f.other.kind].toLowerCase() }}</span>
                          <code>{{ valuesOf(f.other).join(", ") }}</code>
                          <IconButton
                            icon="lock"
                            size="sm"
                            label="Fiche non accessible"
                            :aria-describedby="UNREADABLE_TIP[f.other.kind].id"
                            v-on="UNREADABLE_ON[f.other.kind]"
                          />
                        </template>
                        <RouterLink
                          v-else-if="cardOf(f.other)"
                          :to="cardOf(f.other)!"
                          class="k-mono"
                          >{{ endName(f.other) }}</RouterLink
                        >
                        <code v-else>{{ endName(f.other) }}</code>
                        <RouterLink
                          v-if="!f.unreadable && contextOf(f.other)"
                          :to="contextCard(f.other)"
                          class="k-mono"
                          >{{ contextOf(f.other) }}</RouterLink
                        >
                        <span
                          v-else-if="!f.unreadable && f.other.kind === 'prefix'"
                          class="k-ecran-muted"
                          >préfixe</span
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
              </div>
            </Stack>
          </Stack>
        </template>
        <template #interfaces>
          <ul class="k-e11-list">
            <li v-for="i in shownMachine.interfaces" :key="i.name" class="k-e11-node">
              <RouterLink :to="ifaceCard(shownMachine.key, i.name)" class="k-mono">{{
                i.name
              }}</RouterLink>
              <KeyLabel v-if="i.media" :value="i.media" :label="INTERFACE_MEDIA[i.media]" />
              <span>{{ INTERFACE_ROLE[i.role] }}</span>
              <StatusBadge v-if="!i.enabled" state="neutral" label="Désactivée" />
              <code v-if="i.addresses.length">{{
                i.addresses.map((a) => a.value).join(", ")
              }}</code>
            </li>
          </ul>
        </template>
        <template #history>
          <p class="k-ecran-muted">Versions de la machine et changesets.</p>
        </template>
      </Tabs>
    </Panneau>
    <Tooltip v-for="kind in KINDS" :key="kind" :control="UNREADABLE_TIP[kind]"
      >Fiche non accessible · <code>{{ READ[kind] }}</code></Tooltip
    >
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

  /* In a cell of a grid, on one line (the row keeps its height), unless the column wraps. */
  .k-data-grid__cell:not([data-wrap]) .k-e11-verdict,
  .k-data-grid__cell .k-e11-port {
    flex-wrap: nowrap;
  }

  /* A property of the interface card: its value, the menu of its closed list, the draft. */
  .k-e11-property {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--k-space-1) var(--k-space-2);
    align-items: center;
    min-inline-size: 0;
  }

  /* The interfaces of a machine, each with its medium and role, one per line. */
  .k-e11-ifaces {
    display: grid;
    gap: var(--k-space-1);
  }

  .k-e11-ifaces > span {
    display: flex;
    flex-wrap: wrap;
    column-gap: var(--k-space-2);
  }

  /* The simulated effect: each label on its line at its own width, the count at the end. */
  .k-e11-counts {
    display: grid;
    grid-template-columns: minmax(0, max-content) auto;
    gap: var(--k-space-1) var(--k-space-4);
    margin: 0;
  }

  .k-e11-counts__item {
    display: grid;
    grid-template-columns: subgrid;
    grid-column: 1 / -1;
    align-items: baseline;
  }

  .k-e11-counts__item > dt {
    color: var(--k-text-muted);
  }

  .k-e11-counts__item > dd {
    margin: 0;
    text-align: end;
  }

  /* A flow of the draft tab: source → destination, then its verdict. */
  .k-e11-item {
    padding-block-end: var(--k-space-3);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  /* A tab of the bottom panel: the answer on top, then its one subject; it reads its own
   * width, so that nothing in it is wider than the panel. */
  .k-e11-bottom {
    display: grid;
    gap: var(--k-space-3);
    container-type: inline-size;
  }

  .k-e11-answer {
    display: grid;
    gap: var(--k-space-1);
  }

  .k-e11-summary {
    gap: var(--k-space-2);
  }

  /* Extrémités: source and destination side by side while they hold. */
  .k-e11-ends,
  .k-e11-questions {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, var(--k-size-panel-md)), 1fr));
    gap: var(--k-space-3) var(--k-space-5);
    align-items: start;
  }

  .k-e11-end {
    display: grid;
    gap: var(--k-space-1);
    min-inline-size: 0;
  }

  .k-e11-addresses {
    display: grid;
    gap: var(--k-space-1);
    list-style: none;
  }

  .k-e11-address {
    display: grid;
  }

  .k-e11-axes {
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
  }

  /* Matrice: the attributes and the text, the excerpt beside them while it holds. */
  .k-e11-matrix {
    display: grid;
    grid-template-columns: minmax(0, 1fr) max-content;
    grid-template-areas: "attrs excerpt" "text excerpt";
    gap: var(--k-space-2) var(--k-space-5);
    align-items: start;
  }

  .k-e11-matrix > .k-ecran-attrs {
    grid-area: attrs;
  }

  .k-e11-matrix > .k-e11-text {
    grid-area: text;
  }

  .k-e11-excerpt {
    grid-area: excerpt;
    max-inline-size: max-content;
  }

  @container (width < 720px) {
    .k-e11-matrix {
      grid-template-columns: minmax(0, 1fr);
      grid-template-areas: "attrs" "text" "excerpt";
    }
  }

  /* Under the width of the excerpt, the cell above says it: no matrix wider than the panel. */
  @container (width < 440px) {
    .k-e11-excerpt {
      display: none;
    }
  }

  /* Verdicts: one line per pair, its columns aligned by a subgrid. */
  .k-e11-verdicts {
    display: grid;
    gap: var(--k-space-3);
  }

  .k-e11-pairs {
    display: grid;
    grid-template-columns: repeat(2, max-content) minmax(0, max-content);
    gap: var(--k-space-1) var(--k-space-4);
    list-style: none;
  }

  .k-e11-pair {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    align-items: center;
  }

  @container (width < 440px) {
    .k-e11-pairs {
      grid-template-columns: minmax(0, 1fr);
    }

    .k-e11-pair {
      display: flex;
      flex-wrap: wrap;
      gap: var(--k-space-1);
      padding-block-end: var(--k-space-1);
      border-block-end: var(--k-border-width) solid var(--k-border);
    }
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

  /* The cases of the mock-up: links, the current one marked by weight and underline. */
  .k-e11-cases {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-1) var(--k-space-3);
    align-items: baseline;
  }

  .k-e11-case {
    display: inline-flex;
    align-items: center;
    min-block-size: var(--k-control-height-sm);
  }

  .k-e11-case[aria-current="page"] {
    font-weight: var(--k-weight-semibold);
    color: var(--k-text);
    text-decoration-thickness: var(--k-space-0-5);
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

  .k-e11-choice {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-1) var(--k-space-2);
    align-items: center;
    min-block-size: var(--k-control-height-sm);
  }

  /* The two questions of a test, each in its own block. */
  .k-e11-question {
    display: grid;
    gap: var(--k-space-2);
    align-content: start;
    min-inline-size: 0;
    padding-block-start: var(--k-space-2);
    border-block-start: var(--k-border-width) solid var(--k-border);
  }

  .k-e11-question__title {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    align-items: center;
    font-weight: var(--k-weight-semibold);
  }

  /*
   * The flows of a machine: two compact lists (Entrants, Sortants) on one grid, so that their
   * columns share widths and their rows a baseline; each group, list and row is a subgrid.
   * The badges keep their widest one (`min-content`): the other columns wrap before them.
   */
  .k-e11-flow-groups {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, max-content)) minmax(min-content, 1fr);
    gap: var(--k-space-3) var(--k-space-5);
    container-type: inline-size;
  }

  .k-e11-flows,
  .k-e11-flow-list {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    row-gap: var(--k-space-1);
  }

  .k-e11-flows > * {
    grid-column: 1 / -1;
  }

  .k-e11-flow-list {
    list-style: none;
  }

  .k-e11-flow {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    align-content: center;
    align-items: baseline;
    min-block-size: var(--k-row-height);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  /* An end whose object the reader does not read, in the register (027): its kind over
   * its values without link, the lock at the end of the values, whose tooltip says
   * « Fiche non accessible » and the permission (retour du 2026-10-28). */
  .k-e11-unreadable {
    display: grid;
  }

  .k-e11-unreadable__kind {
    font-family: var(--k-font-sans);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    color: var(--k-text-muted);
  }

  .k-e11-unreadable .k-icon-button {
    vertical-align: middle;
  }

  .k-e11-flow__end,
  .k-e11-flow__state {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--k-space-1);
    align-items: center;
    min-inline-size: 0;
  }

  /* Narrow: the groups leave the shared columns, each row wraps its parts. */
  @container (width < 720px) {
    .k-e11-flows,
    .k-e11-flow-list {
      grid-template-columns: minmax(0, 1fr);
    }

    .k-e11-flow {
      display: flex;
      flex-wrap: wrap;
      gap: var(--k-space-1) var(--k-space-3);
      padding-block: var(--k-space-1);
    }
  }
}
</style>
