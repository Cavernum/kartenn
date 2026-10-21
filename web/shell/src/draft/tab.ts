// The identifier of a tab (design « Écriture » : brouillon): random, kept in
// `sessionStorage`. A duplicated tab inherits the `sessionStorage` of its original, so the
// two would share one draft: at startup a tab announces its identifier on the channel
// `kartenn-tabs`; a live tab holding the same one answers, and the newcomer takes a new
// identifier, with a copy of the draft.

export const TAB_KEY = "kartenn.tab";
export const CHANNEL = "kartenn-tabs";

type Message =
  { type: "hello"; id: string; nonce: string } | { type: "taken"; id: string; nonce: string };

/** What the tab needs of a `BroadcastChannel`. */
export interface Channel {
  postMessage(message: Message): void;
  onmessage: ((event: { data: Message }) => void) | null;
}

export interface TabOptions {
  storage: Storage | null;
  channel: Channel | null;
  newId?: () => string;
  /** Called once if this tab turns out to be a duplicate: its old and new identifiers. */
  onDuplicate?: (from: string, to: string) => void;
}

/** The tab of this document: its identifier, kept current. */
export class Tab {
  private current: string;
  private readonly nonce: string;
  private readonly newId: () => string;

  constructor(private readonly options: TabOptions) {
    this.newId = options.newId ?? (() => crypto.randomUUID());
    this.nonce = this.newId();
    const stored = options.storage?.getItem(TAB_KEY) ?? null;
    this.current = stored ?? this.newId();
    options.storage?.setItem(TAB_KEY, this.current);
    const channel = options.channel;
    if (!channel) return;
    channel.onmessage = ({ data }) => {
      if (data.type === "hello" && data.id === this.current && data.nonce !== this.nonce) {
        channel.postMessage({ type: "taken", id: data.id, nonce: data.nonce });
      } else if (data.type === "taken" && data.nonce === this.nonce && data.id === this.current) {
        const from = this.current;
        this.current = this.newId();
        options.storage?.setItem(TAB_KEY, this.current);
        options.onDuplicate?.(from, this.current);
      }
    };
    channel.postMessage({ type: "hello", id: this.current, nonce: this.nonce });
  }

  get id(): string {
    return this.current;
  }
}

/** The channel of the browser, if it has one. */
export function browserChannel(): Channel | null {
  try {
    return typeof BroadcastChannel === "function"
      ? (new BroadcastChannel(CHANNEL) as unknown as Channel)
      : null;
  } catch {
    return null;
  }
}
