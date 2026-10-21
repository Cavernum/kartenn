import { shallowRef, type ShallowRef } from "vue";
import { shellFetch } from "../session/index.ts";

// Reads of the shell's own screens: through `shellFetch` (a 401 opens « Session
// expirée »), with the problem of a refusal kept for display.

/** A problem of the API (RFC 9457), as the shell shows it. */
export interface Problem {
  type: string;
  title: string;
  detail: string | null;
  status: number;
}

/** An answer: the body and its `ETag` (the `If-Match` of a later write), or the problem. */
export type Read<T> =
  | { ok: true; status: number; body: T; etag?: string | null }
  | { ok: false; status: number; problem: Problem };

const NETWORK: Problem = { type: "about:blank", title: "network", detail: null, status: 0 };

/** The URL `path` with its parameters, empty ones left out. */
export function withQuery(path: string, params: Record<string, string | null | undefined>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== null && value !== undefined && value !== "") search.set(key, value);
  }
  const text = search.toString();
  return text ? `${path}${path.includes("?") ? "&" : "?"}${text}` : path;
}

async function problemOf(response: Response): Promise<Problem> {
  const body = (await response.json().catch(() => null)) as Partial<Problem> | null;
  return {
    type: body?.type ?? "about:blank",
    title: body?.title ?? response.statusText,
    detail: body?.detail ?? null,
    status: response.status,
  };
}

/**
 * The body of a successful answer, or `undefined` when reading it fails: a navigation that
 * leaves the page cuts the body short (Firefox then rejects `text()`), and an error left
 * to the caller would reach Vue's handler, logged as an uncaught error of the screen.
 */
async function bodyText(response: Response): Promise<string | undefined> {
  return response.text().catch(() => undefined);
}

/** A read of the API; a failure of the network is a problem of status 0. */
export async function read<T>(url: string): Promise<Read<T>> {
  const response = await shellFetch(url).catch(() => null);
  if (!response) return { ok: false, status: 0, problem: NETWORK };
  if (!response.ok)
    return { ok: false, status: response.status, problem: await problemOf(response) };
  const text = await bodyText(response);
  if (text === undefined) return { ok: false, status: 0, problem: NETWORK };
  return {
    ok: true,
    status: response.status,
    body: JSON.parse(text) as T,
    etag: response.headers.get("ETag"),
  };
}

/** A write of the API with a JSON body (`POST`), its problem kept. */
export async function write<T>(url: string, body: unknown): Promise<Read<T | null>> {
  const response = await shellFetch(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  }).catch(() => null);
  if (!response) return { ok: false, status: 0, problem: NETWORK };
  if (!response.ok)
    return { ok: false, status: response.status, problem: await problemOf(response) };
  const text = await bodyText(response);
  if (text === undefined) return { ok: false, status: 0, problem: NETWORK };
  return { ok: true, status: response.status, body: text ? (JSON.parse(text) as T) : null };
}

/** An answer of `send`, with the `ETag` of the response (the version to send in `If-Match`). */
export type Answer<T> = Read<T | null> & { etag: string | null };

/**
 * A request of the API with any method: a JSON body when `body` is given (`content-type`
 * overridable, `application/merge-patch+json` for `PATCH`), headers such as `If-Match`.
 */
export async function send<T>(
  method: string,
  url: string,
  body?: unknown,
  headers: Record<string, string> = {},
): Promise<Answer<T>> {
  const init: RequestInit = { method, headers: { ...headers } };
  if (body !== undefined) {
    init.headers = { "content-type": "application/json", ...headers };
    init.body = JSON.stringify(body);
  }
  const response = await shellFetch(url, init).catch(() => null);
  if (!response) return { ok: false, status: 0, problem: NETWORK, etag: null };
  const etag = response.headers.get("etag");
  if (!response.ok)
    return { ok: false, status: response.status, problem: await problemOf(response), etag };
  const text = await bodyText(response);
  if (text === undefined) return { ok: false, status: 0, problem: NETWORK, etag };
  return { ok: true, status: response.status, body: text ? (JSON.parse(text) as T) : null, etag };
}

/** A page of a list of the contract (REQ-006-7). */
export interface Listing<T> {
  items: T[];
  links?: { next?: string | null };
}

export const PAGE_SIZE = 100;
const INVALID_CURSOR = "urn:kartenn:problem:invalid-cursor";

/**
 * The pages of a list, read one after the other as the view asks (continuous scrolling).
 * The cursor is never in the URL of the shell (it is signed and may expire): a refused
 * cursor starts again from the first page (design « Écrans et routes du client »).
 */
export class Pager<T> {
  readonly items: ShallowRef<T[]> = shallowRef([]);
  readonly done = shallowRef(false);
  readonly loading = shallowRef(false);
  /** The list started again after a refused cursor. */
  readonly restarted = shallowRef(false);
  readonly problem = shallowRef<Problem | null>(null);
  private next: string | null;
  private generation = 0;

  constructor(
    private readonly first: string,
    private readonly reader: <B>(url: string) => Promise<Read<B>> = read,
  ) {
    this.next = first;
  }

  /** Reads the next page, if any and if none is being read. */
  async more(): Promise<void> {
    if (this.loading.value || this.done.value || this.next === null) return;
    const generation = this.generation;
    this.loading.value = true;
    const answer = await this.reader<Listing<T>>(this.next);
    if (generation !== this.generation) return;
    this.loading.value = false;
    if (!answer.ok) {
      if (answer.problem.type === INVALID_CURSOR && this.next !== this.first) {
        this.restart();
        this.restarted.value = true;
        return this.more();
      }
      this.problem.value = answer.problem;
      this.done.value = true;
      return;
    }
    this.items.value = [...this.items.value, ...answer.body.items];
    this.next = answer.body.links?.next ?? null;
    this.done.value = this.next === null;
  }

  /** Back to an empty list, at the first page. */
  restart(): void {
    this.generation += 1;
    this.items.value = [];
    this.next = this.first;
    this.done.value = false;
    this.loading.value = false;
    this.problem.value = null;
  }
}
