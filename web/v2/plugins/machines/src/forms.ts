// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The forms of the plugin: each adds one operation of the module to the active draft of the
// tab (`draft.add`, W10), never a changeset of its own — the draft is reviewed, simulated and
// submitted by the shell, with its motive. Offered at the present only (W9); without the
// permission, the button stays, refused with its reason (`aria-disabled`).

import { ApiProblem, context, draft, type Operation } from "@kartenn/sdk";
import { button, el, failure, stack, t, type Field, type Texts } from "./dom.ts";
import { present } from "./element.ts";

/** Counts the forms drawn: the identifiers of their reasons are unique. */
let reasons = 0;

export interface DraftForm {
  form: HTMLFormElement;
  /** Shows or hides the form for the dates of the shell, refuses it without the permission. */
  refresh(): void;
}

/**
 * A form of fields whose submission adds `operation()` to the draft (`null`: refused on a
 * field, nothing sent). A violation of the server whose pointer ends with the name of a
 * field is said on it.
 */
export function draftForm(
  texts: Texts,
  options: {
    fields: Field<HTMLInputElement | HTMLSelectElement>[];
    extra?: Node[];
    submit: string;
    permission: string;
    operation: () => Operation | null;
    /** After the operation is in the draft: the form cleared, by default. */
    added?: (form: HTMLFormElement) => void;
    actions?: Node[];
  },
): DraftForm {
  const form = el("form", undefined, "k-stack");
  form.dataset["kGap"] = "4";
  form.noValidate = true;
  // The outcome: said by a live region there from the start; a refusal as an alert.
  // Empty, neither takes a place in the stack (`k-machines-outcome`).
  const said = el("p", undefined, "k-machines-outcome");
  said.setAttribute("role", "status");
  const failed = el("div", undefined, "k-machines-outcome");
  const submit = button(texts.text(options.submit), "primary", "submit");
  // Why the button is refused, written beside it: no tooltip without the framework.
  const reason = el("p", undefined, "k-machines-muted");
  reason.id = `k-machines-reason-${++reasons}`;
  reason.hidden = true;
  const actions = el("div", undefined, "k-machines-actions");
  actions.append(submit, ...(options.actions ?? []));
  const fields = el("div", undefined, "k-machines-fields");
  fields.append(...options.fields.map((f) => f.field));
  form.append(stack("4", fields, ...(options.extra ?? [])), said, failed, actions, reason);
  const allowed = () => context.get().user.has(options.permission);

  form.addEventListener("submit", (event) => {
    event.preventDefault();
    if (!allowed() || !present() || submit.getAttribute("aria-busy") === "true") return;
    said.textContent = "";
    failed.replaceChildren();
    for (const field of options.fields) field.refuse(null);
    // Required fields left empty, refused on themselves before anything is sent.
    const empty = options.fields.filter((f) => f.control.required && !f.control.value.trim());
    for (const field of empty) field.refuse(t("form.required"));
    if (empty.length) {
      empty[0]!.control.focus();
      return;
    }
    const operation = options.operation();
    if (!operation) return;
    submit.setAttribute("aria-busy", "true");
    submit.setAttribute("aria-disabled", "true");
    // Said while it is sent: `aria-busy` on a button is seldom read.
    said.textContent = t("form.adding");
    void draft
      .add(operation)
      .then(
        (outcome) => {
          if (outcome.kind === "saved") {
            said.textContent = t("form.added", { n: outcome.draft.operation_count });
            (options.added ?? ((f) => f.reset()))(form);
          } else {
            refused(outcome.problem);
          }
        },
        (error: unknown) => refused(error),
      )
      .finally(() => {
        submit.removeAttribute("aria-busy");
        submit.removeAttribute("aria-disabled");
      });
  });
  // The outcome of the last addition, gone once the form is changed again.
  form.addEventListener("input", () => (said.textContent = ""));

  /** The refusal as an alert (the summary), each violation on its field, the first focused. */
  function refused(problem: unknown): void {
    said.textContent = "";
    failed.replaceChildren(failure(t("form.refused"), problem));
    if (!(problem instanceof ApiProblem)) return;
    let first: Field<HTMLInputElement | HTMLSelectElement> | undefined;
    for (const violation of problem.violations) {
      const name = violation.pointer?.split("/").pop();
      const field = options.fields.find((f) => f.control.name === name);
      field?.refuse(`${violation.detail} (${violation.rule})`);
      first ??= field;
    }
    first?.control.focus();
  }

  return {
    form,
    refresh() {
      // Its section with it: at another date, nothing of the form is offered.
      (form.closest("section") ?? form).hidden = !present();
      if (allowed()) {
        submit.removeAttribute("aria-disabled");
        submit.removeAttribute("aria-describedby");
        reason.hidden = true;
      } else {
        // Visible and refused, its reason as its description (skill kartenn-ui, « Droits »).
        submit.setAttribute("aria-disabled", "true");
        reason.replaceChildren(t("form.needs"), " ", el("code", options.permission));
        reason.hidden = false;
        submit.setAttribute("aria-describedby", reason.id);
      }
    },
  };
}
