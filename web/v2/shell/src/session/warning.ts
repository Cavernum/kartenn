import type { Toasts } from "@kartenn/ui";
import { watch, type WatchSource } from "vue";
import { toastCodes } from "../problems/index.ts";
import { createExpiry, readEnd, type EndReading, type ExpiryDeps } from "./expiry.ts";
import { expired } from "./index.ts";

// The warning of the end of the session in the frame (REQ-021-14): the ends the session
// gives (`me.session.expires_at`, `absolute_expires_at`) watched, a `warn` toast that stays
// until the next activity, and « Rester connecté », which reads the ends again
// (`readEnd`): an end that moved clears the warning; a 401 opens « Session expirée »; any
// other failure keeps the session as it is and says so by `problems/`: the sentence of its
// type, its code and its `Request-Id`. The absolute end (021/A14) is said two minutes
// before it, without « Rester connecté », which would not push it back.

export interface WarningDeps {
  toasts: Toasts;
  t: (key: string, params?: Record<string, unknown>) => string;
  /** A problem in plain words (`useProblems().sentence`, `problems/`). */
  sentence: (error: unknown) => string;
  /** The language the time of the end is written in. */
  locale: () => string;
  /** The end the session gives, reactive (`expires_at`). */
  end: WatchSource<string | null | undefined>;
  /** The end of its lifetime alone, reactive (`absolute_expires_at`). */
  absoluteEnd: WatchSource<string | null | undefined>;
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
    if (reading.kind === "end") expiry.read(reading.expiresAt, reading.absoluteAt);
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
          title: t(extendable ? "session.ending" : "session.endingAbsolute", {
            time: timeOf(end),
          }),
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
  const stop = watch(
    [deps.end, deps.absoluteEnd],
    ([end, absoluteEnd]) => expiry.read(end, absoluteEnd),
    { immediate: true },
  );
  return () => {
    stop();
    expiry.stop();
  };
}
