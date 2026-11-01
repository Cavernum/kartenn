// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The common part of the elements of the plugin, as in the example plugin (spec 021, task
// 19): an open shadow root, the sheet of @kartenn/ui then the plugin's, nothing drawn before
// them; reads through the shared cache (`queryClient`, `reads`), an observer per query held
// while the element is connected, moved to the dates of the shell when they change; the
// element draws what the cache holds, it invalidates nothing itself.

import {
  context,
  InfiniteQueryObserver,
  QueryObserver,
  queryClient,
  type Entry,
  type Key,
  type ListOptions,
  type ObjectRef,
  type ReadOptions,
} from "@kartenn/sdk";
import { arrival, defined, el, Texts } from "./dom.ts";

/** A read held by an element: its state, read again once invalidated. */
export class Followed<T> {
  readonly observer: QueryObserver<Entry<T>, Error>;

  /** `options`: the options of `reads` at the dates of now (typed by `follow`). */
  constructor(private readonly options: () => unknown) {
    this.observer = new QueryObserver<Entry<T>, Error>(queryClient, options() as never);
  }

  retime(): void {
    this.observer.setOptions(this.options() as never);
  }

  get data(): T | undefined {
    return this.observer.getCurrentResult().data?.data;
  }

  /** The failure of the last read, `null` if it was read. */
  get error(): unknown {
    const result = this.observer.getCurrentResult();
    return result.isError ? result.error : null;
  }
}

type Paged<T> = { items: T[]; links: { next?: string | null } };

/** A list held by an element, page by page: the items of the pages read. */
export class FollowedList<T> {
  readonly observer: InfiniteQueryObserver<Entry<Paged<T>>, Error>;

  /** `options`: the options of `reads.list` at the dates of now (typed by `followList`). */
  constructor(private readonly options: () => unknown) {
    this.observer = new InfiniteQueryObserver(queryClient, options() as never);
  }

  retime(): void {
    this.observer.setOptions(this.options() as never);
  }

  /** The items of the pages read, `undefined` before the first. */
  get items(): T[] | undefined {
    const pages = this.observer.getCurrentResult().data?.pages;
    return pages?.flatMap((page) => page.data.items);
  }

  get error(): unknown {
    const result = this.observer.getCurrentResult();
    return result.isError ? result.error : null;
  }

  /** Whether a next page is there to read. */
  get more(): boolean {
    return this.observer.getCurrentResult().hasNextPage;
  }

  get fetching(): boolean {
    return this.observer.getCurrentResult().isFetchingNextPage;
  }

  next(): void {
    void this.observer.fetchNextPage();
  }
}

/** What an element holds of a read: its observer, moved to other dates by `retime`. */
interface Held {
  readonly observer: { subscribe(listener: () => void): () => void };
  retime(): void;
}

/**
 * An element of the plugin: `build` draws its page once its sheets are there, `update`
 * draws the states of its reads (in `Area`s, again only when what they say changes). Once
 * connected, it follows the context of the shell (language, dates) and its reads;
 * disconnected, it lets go of them all.
 */
export abstract class MachinesElement extends HTMLElement {
  /** The object of the record (tab, action) or of the row (column). */
  object: ObjectRef | null = null;
  private readonly root: ShadowRoot = this.attachShadow({ mode: "open" });
  /**
   * What the element draws, beside its sheets and never them: a `<link>` moved in the tree
   * would be read again, its rules gone meanwhile.
   */
  private readonly content: HTMLDivElement = this.root.appendChild(
    el("div", undefined, "k-machines-content"),
  );
  private sheets: Promise<unknown> | null = null;
  private stops: (() => void)[] = [];
  /** Counts the connections: a later one, or a disconnection, voids a pending start. */
  private connection = 0;
  private built = false;
  /** The reads held, each with the function that lets it go while connected. */
  private readonly held = new Map<Held, (() => void) | null>();
  private connected = false;
  /** The texts drawn once, relabelled in another language. */
  protected readonly texts = new Texts();

  connectedCallback(): void {
    // After a microtask: an element already in the page is upgraded within
    // `customElements.define`, before `definePlugin` has given back the handle.
    this.sheets ??= Promise.resolve().then(() => this.link());
    const connection = ++this.connection;
    void this.sheets.then(() => {
      if (connection !== this.connection) return;
      if (!this.built) {
        this.built = true;
        this.content.replaceChildren(...this.build());
      }
      this.connected = true;
      for (const read of this.held.keys()) this.hold(read);
      this.stops.push(context.subscribe(() => this.changed()));
      this.changed();
      // Reached by a link of the plugin (focus 2.4.3): on the heading of the new page.
      const heading = this.root.querySelector<HTMLElement>("h1");
      if (arrival.byLink && heading) {
        arrival.byLink = false;
        heading.focus({ preventScroll: true });
      }
    });
  }

  disconnectedCallback(): void {
    this.connection++;
    this.connected = false;
    for (const stop of this.stops.splice(0)) stop();
    for (const [read, stop] of this.held) {
      stop?.();
      this.held.set(read, null);
    }
  }

  /** Subscribes to a read: each of its states draws the element again. */
  private hold(read: Held): void {
    this.held.get(read)?.();
    this.held.set(
      read,
      read.observer.subscribe(() => this.update()),
    );
  }

  /** The sheet of @kartenn/ui then the plugin's, settled either way, said if refused. */
  private async link(): Promise<void> {
    const handle = defined.handle;
    if (!handle) {
      console.warn("kartenn: machines: no handle, the elements are drawn without their sheets");
      return;
    }
    const linked = await Promise.allSettled([
      handle.styles(this.root, "@kartenn/ui/style.css"),
      handle.styles(this.root, "styles.css"),
    ]);
    for (const outcome of linked) {
      if (outcome.status === "rejected") {
        console.warn(`kartenn: machines: ${this.localName}: sheet refused:`, outcome.reason);
      }
    }
  }

  /** The context changed, or the element connects: the reads move to its dates. */
  private changed(): void {
    for (const read of this.held.keys()) read.retime();
    this.texts.relabel();
    this.update();
  }

  /**
   * A read the element follows while connected: from `build`, or later (a read that depends
   * on another, a search), when it is followed at once.
   */
  protected follow<T, K extends Key>(options: () => ReadOptions<T, K>): Followed<T> {
    return this.add(new Followed<T>(options));
  }

  /** A list the element follows while connected, page by page. */
  protected followList<T, K extends Key>(options: () => ListOptions<T, K>): FollowedList<T> {
    return this.add(new FollowedList<T>(options));
  }

  private add<R extends Held>(read: R): R {
    this.held.set(read, null);
    if (this.connected) this.hold(read);
    return read;
  }

  /** Lets go of a read the element no longer shows. */
  protected drop(read: Held): void {
    this.held.get(read)?.();
    this.held.delete(read);
  }

  /** Its page, drawn once: headings, forms, and the `Area`s of its states. */
  protected abstract build(): Node[];

  /** Draws the states of its reads. */
  protected abstract update(): void;
}

/** Whether the shell looks at the present: writes are offered only then (W9). */
export function present(): boolean {
  return context.get().time.present;
}
