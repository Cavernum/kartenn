import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { AuditEvent } from "./audit.ts";
import { useList } from "./reads.ts";

// The words of the audit journal (REQ-021-12): an instant in the language of the moment;
// the actor by the name of its identity when the reader may list the identities, else by
// its identifier (fixed pitch), the system and the unknown in words; an outcome, a kind
// and a criticality by the catalogs of the shell, else their key; the objects named, by
// their kind and their number.

export function useAuditWords(names: () => boolean) {
  const { t, te, locale } = useI18n();
  const known = (key: string, fallback: string) => (te(key) ? t(key) : fallback);
  const format = computed(
    () => new Intl.DateTimeFormat(locale.value, { dateStyle: "short", timeStyle: "medium" }),
  );
  // The identities, read once by whoever may (the actors of the journal are among them).
  const identities = useList("/v1/identities", null, undefined, names);
  const byId = computed(
    () => new Map(identities.rows.value.map((i) => [String(i.id), String(i.name)])),
  );
  const identity = (id: string | null) => (id ? byId.value.get(id) : undefined);
  const kind = (value: string) => known(`admin.audit.actorKind.${value}`, value);

  return {
    when: (iso: string) => {
      const date = new Date(iso);
      return Number.isNaN(date.getTime()) ? iso : format.value.format(date);
    },
    /** The name of an identity, when read. */
    identity,
    /** Whether the actor shows in words (a name, a kind), not by its identifier. */
    actorNamed: (event: AuditEvent) => !event.actor_id || identity(event.actor_id) !== undefined,
    actor: (event: AuditEvent) =>
      identity(event.actor_id) ?? event.actor_id ?? kind(event.actor_kind),
    kind,
    outcome: (value: string) => known(`admin.audit.outcome.${value}`, value),
    criticality: (value: string) => known(`admin.audit.criticality.${value}`, value),
    /** A severity in words (`INFO`, `WARN`, `ERROR`), else its code. */
    severity: (value: string) => known(`admin.audit.severity.${value}`, value),
    severityKnown: (value: string) => te(`admin.audit.severity.${value}`),
    targets: (event: AuditEvent) =>
      event.target_kind
        ? t("admin.audit.targetCount", {
            kind: event.target_kind,
            n: event.target_ids?.length ?? 0,
          })
        : "—",
  };
}
