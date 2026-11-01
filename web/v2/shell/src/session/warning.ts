import type { Toasts } from "@kartenn/ui";
import { watch, type WatchSource } from "vue";
import { toastCodes } from "../problems/index.ts";
import { createExpiry, readEnd, type EndReading, type ExpiryDeps } from "./expiry.ts";
import { expired } from "./index.ts";

// The warning of the end of the session in the frame (REQ-021-14): the end the session
// gives (`me.session.expires_at`) watched, a `warn` toast that stays until the next
// activity, and « Rester connecté », which reads the end again (`readEnd`): an end that
// moved clears the warning; one that did not is fixed, said again without the action; a
// 401 opens « Session expirée »; any other failure keeps the session as it is and says so
// by `problems/`: the sentence of its type, its code and its `Request-Id`.

export interface WarningDeps {
  toasts: Toasts;
  t: (key: string, params?: Record<string, unknown>) => string;
  /** A problem in plain words (`useProblems().sentence`, `problems/`). */
  sentence: (error: unknown) => string;
  /** The language the time of the end is written in. */
  locale: () => string;
  /** The end the session gives, reactive. */
  end: WatchSource<string | null | undefined>;
  readEnd?: () => Promise<EndReading>;
  expiry?: ExpiryDeps;
}

/** Watches the end of the session for the frame; returns how to stop. */
export function watchSessionEnd(deps: WarningDeps): () => void {
  const { toasts, t } = deps;
  let ending: string | null = null;
  const timeOf = (at: number) =>
    new Intl.DateTimeFormat(deps.locale(), { timeStyle: "short" }).format(new Date(at));

  async function stay(): Promise<void> {
    const reading = await (deps.readEnd ?? readEnd)();
    if (reading.kind === "end") expiry.read(reading.expiresAt);
    else if (reading.kind === "expired") expired.value = true;
    else {
      const problem = reading.problem;
      toasts.show({
        tone: "ko",
        title: t("session.stayFailed"),
        description: t("session.stayFailedText", { why: deps.sentence(problem) }),
        ...toastCodes(problem),
        action: { label: t("session.retry"), run: () => void stay() },
      });
    }
  }

  const expiry = createExpiry(
    {
      warn(end, extendable) {
        if (ending) toasts.dismiss(ending);
        ending = toasts.show({
          tone: "warn",
          title: t("session.ending", { time: timeOf(end) }),
          description: t(extendable ? "session.endingText" : "session.endingFixed"),
          ...(extendable ? { action: { label: t("session.stay"), run: () => void stay() } } : {}),
        });
      },
      clear() {
        if (ending) toasts.dismiss(ending);
        ending = null;
      },
    },
    deps.expiry,
  );
  const stop = watch(deps.end, (end) => expiry.read(end), { immediate: true });
  return () => {
    stop();
    expiry.stop();
  };
}
