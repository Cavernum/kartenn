import { shellFetch } from "../session/index.ts";

// A quick edit sent by the route of the resource (REQ-007-25, design « Écriture »): one
// changeset, a motive, the `If-Match` of the read; the answer read as what the form shows.

/** A violation of a rule as the API serves it (REQ-006-21). */
export interface Violation {
  rule: string;
  severity: string;
  detail: string;
  pointer?: string | null;
}

/** What the form does with an answer. */
export type Outcome =
  /** 200, 201: applied; the object to read again, the changeset to link. */
  | { kind: "applied"; changeset: string | null; location: string | null }
  /** 202: waits for an approval. */
  | { kind: "pending"; changeset: string | null }
  /** 412: the object changed since the read. */
  | { kind: "stale" }
  /** 422: violations by field (`/slug` → `slug`), the others for the whole form. */
  | { kind: "invalid"; byField: Record<string, string[]>; general: string[]; type: string }
  /** 403, or another problem: shown at the top, the input kept. */
  | { kind: "refused"; status: number; type: string; detail: string | null; title: string };

export type Method = "POST" | "PATCH" | "DELETE";

export interface Request {
  method: Method;
  url: string;
  body: Record<string, unknown>;
  ifMatch?: string | null | undefined;
  /** Kept across retries of the same creation (REQ-006-15). */
  idempotencyKey?: string;
}

/** The field a JSON Pointer of the request names (`/slug`, `/value`), if one. */
export function fieldOf(pointer: string | null | undefined): string | null {
  const match = /^\/([a-z_]+)$/.exec(pointer ?? "");
  return match ? match[1]! : null;
}

/** The identifier of the changeset of an answer, from its body. */
function changesetOf(body: unknown): string | null {
  const changeset = (body as { changeset?: { id?: unknown } } | null)?.changeset;
  return typeof changeset?.id === "string" ? changeset.id : null;
}

/** The rules that refuse for lack of rights, rather than for the content. */
const FORBIDDEN = /\.forbidden(_effect)?$/;
const FORBIDDEN_TYPE = "urn:kartenn:problem:forbidden";

/** The outcome of an answer of a write by resource. */
export async function outcomeOf(response: Response): Promise<Outcome> {
  const body = (await response.json().catch(() => null)) as Record<string, unknown> | null;
  if (response.status === 200 || response.status === 201) {
    const location = response.headers.get("Location");
    return { kind: "applied", changeset: changesetOf(body), location };
  }
  if (response.status === 202) return { kind: "pending", changeset: changesetOf(body) };
  if (response.status === 412) return { kind: "stale" };
  const type = typeof body?.type === "string" ? body.type : "about:blank";
  if (response.status === 422) {
    const byField: Record<string, string[]> = {};
    const general: string[] = [];
    const violations = Array.isArray(body?.violations) ? (body.violations as Violation[]) : [];
    // Rights missing on the objects themselves (`core.object.forbidden`, REQ-005-22): a
    // refusal like a 403, whatever else the changeset breaks.
    const denied = violations.filter((violation) => FORBIDDEN.test(violation.rule));
    if (denied.length > 0) {
      return {
        kind: "refused",
        status: response.status,
        type: FORBIDDEN_TYPE,
        detail: denied.map((violation) => violation.detail).join(" "),
        title: typeof body?.title === "string" ? body.title : response.statusText,
      };
    }
    for (const violation of violations) {
      const field = fieldOf(violation.pointer);
      if (field) (byField[field] ??= []).push(violation.detail);
      else general.push(violation.detail);
    }
    if (violations.length === 0 && typeof body?.detail === "string") general.push(body.detail);
    return { kind: "invalid", byField, general, type };
  }
  return {
    kind: "refused",
    status: response.status,
    type,
    detail: typeof body?.detail === "string" ? body.detail : null,
    title: typeof body?.title === "string" ? body.title : response.statusText,
  };
}

/** Sends a quick edit; a failure of the network is a refusal of status 0. */
export async function submit(
  request: Request,
  fetcher: typeof fetch = shellFetch,
): Promise<Outcome> {
  const headers: Record<string, string> = {
    Accept: "application/json",
    "Content-Type":
      request.method === "PATCH" ? "application/merge-patch+json" : "application/json",
  };
  if (request.ifMatch) headers["If-Match"] = request.ifMatch;
  if (request.method === "POST")
    headers["Idempotency-Key"] = request.idempotencyKey ?? crypto.randomUUID();
  const response = await fetcher(request.url, {
    method: request.method,
    headers,
    body: JSON.stringify(request.body),
  }).catch(() => null);
  if (!response)
    return { kind: "refused", status: 0, type: "about:blank", detail: null, title: "network" };
  return outcomeOf(response);
}
