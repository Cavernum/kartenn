// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { components } from "./schema";

/**
 * No answer: `fetch` failed (the network, the server unreached, a refused CORS), its cause
 * kept. Apart from a bug of the client, which is neither this nor an `ApiProblem`.
 */
export class NetworkError extends Error {
  constructor(cause: unknown) {
    super("the server could not be reached", { cause });
    this.name = "NetworkError";
  }
}

/** A violation of a rule, as served in a problem (REQ-006-21). */
export type Violation = components["schemas"]["ViolationDto"];

/**
 * An answer of the API outside 2xx, as a problem (RFC 9457): `type` is the class of the
 * problem (`urn:kartenn:problem:<slug>`), `requestId` the `Request-Id` of the answer.
 */
export class ApiProblem extends Error {
  readonly status: number;
  readonly type: string;
  readonly title: string;
  readonly detail: string | null;
  readonly violations: readonly Violation[];
  readonly requestId: string | null;
  /**
   * The seconds the answer asks to wait before trying again (`Retry-After`), `null` without
   * it; `NaN` for any form other than delay-seconds (`1*DIGIT`, RFC 9110 §10.2.3), a wait
   * the cache cannot read and never waits out.
   */
  readonly retryAfter: number | null;

  constructor(init: {
    status: number;
    type: string;
    title: string;
    detail?: string | null;
    violations?: readonly Violation[];
    requestId?: string | null;
    retryAfter?: number | null;
  }) {
    super(init.detail ?? init.title);
    this.name = "ApiProblem";
    this.status = init.status;
    this.type = init.type;
    this.title = init.title;
    this.detail = init.detail ?? null;
    this.violations = init.violations ?? [];
    this.requestId = init.requestId ?? null;
    this.retryAfter = init.retryAfter ?? null;
  }

  /** The code shown (REQ-021-20): the slug of `type` (`urn:kartenn:problem:<slug>`). */
  get code(): string {
    return this.type.replace(/^urn:kartenn:problem:/, "");
  }

  /**
   * The problem of an answer: its `application/problem+json` body if there is one, else
   * `about:blank` with the status text (RFC 9457 §4.2.1).
   */
  static async from(response: Response): Promise<ApiProblem> {
    const requestId = response.headers.get("Request-Id");
    const wait = response.headers.get("Retry-After");
    let body: Record<string, unknown> | null = null;
    if (/json/.test(response.headers.get("Content-Type") ?? "")) {
      try {
        body = (await response.json()) as Record<string, unknown>;
      } catch {
        body = null;
      }
    }
    const text = (key: string): string | null =>
      typeof body?.[key] === "string" ? (body[key] as string) : null;
    return new ApiProblem({
      status: response.status,
      type: text("type") ?? "about:blank",
      title: text("title") ?? response.statusText,
      detail: text("detail"),
      violations: Array.isArray(body?.["violations"]) ? (body["violations"] as Violation[]) : [],
      requestId: requestId ?? text("instance"),
      // Its delay-seconds alone, `1*DIGIT` (RFC 9110 §10.2.3), the only form kartenn
      // sends; any other (an HTTP-date, a sign, a fraction…) is no wait read (`NaN`).
      retryAfter: wait === null ? null : /^\d+$/.test(wait) ? Number(wait) : NaN,
    });
  }
}
