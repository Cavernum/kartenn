// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * What an error state shows of a problem of the API (RFC 9457; REQ-020-22): its `type`
 * (`urn:kartenn:problem:<slug>`), the `Request-Id` of the answer, and the violations, each
 * its rule (the code) and its message. An `ApiProblem` of the SDK is one as it is.
 */
export interface ProblemFacts {
  readonly type: string;
  readonly requestId?: string | null;
  readonly violations?: readonly { readonly rule: string; readonly detail: string }[];
}
