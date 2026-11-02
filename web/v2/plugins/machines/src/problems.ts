// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What the Vue elements of the plugin say of a problem of the API (skill kartenn-ui, contract
// 3): the lines of an `InlineAlert`, each rule broken with its code, the problem's own code
// and its `Request-Id`, copiable; no answer at all, said in words.

import { ApiProblem } from "@kartenn/sdk";

/** A line of an alert: a readable message, and the code it stands for. */
export interface AlertLine {
  message: string;
  code?: string;
}

/** The lines of a refusal or a failed read: its rules with their codes, its code, its request. */
export function linesOf(
  problem: unknown,
  t: (key: string, params?: Record<string, unknown>) => string,
): AlertLine[] {
  if (!(problem instanceof ApiProblem)) return [{ message: t("network") }];
  const lines: AlertLine[] = problem.violations.map((v) => ({ message: v.detail, code: v.rule }));
  if (problem.detail) lines.unshift({ message: problem.detail });
  lines.push({ message: t("code"), code: problem.code });
  if (problem.requestId) lines.push({ message: t("request"), code: problem.requestId });
  return lines;
}
