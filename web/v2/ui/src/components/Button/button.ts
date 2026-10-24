// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { IconName } from "../../icons/index.ts";
import type { ControlSize } from "../layout.ts";

/** The four looks of a button: one `primary` per zone at most (brief, « Clichés »). */
export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";

/** Props shared by `Button` and `IconButton`. */
export interface ButtonBaseProps {
  variant?: ButtonVariant;
  size?: ControlSize;
  type?: "button" | "submit" | "reset";
  disabled?: boolean;
  /** Busy: the button stays focusable, announces `aria-busy` and ignores activation. */
  loading?: boolean;
  /** A toggle button (`aria-pressed`), when set. */
  pressed?: boolean | undefined;
  /**
   * The permission the user lacks (024/M14): the action stays visible and focusable,
   * `aria-disabled`, and its tooltip says « Il faut <permission> ».
   */
  forbidden?: string | undefined;
  /** Refused for another reason (a policy): the same, with this sentence as tooltip. */
  reason?: string | undefined;
}

/** Whether an action is refused with its reason (`forbidden` or `reason`). */
export const isBlocked = (props: ButtonBaseProps): boolean => !!(props.forbidden || props.reason);

export interface ButtonProps extends ButtonBaseProps {
  /** A leading icon, decorative: the label names the button. */
  icon?: IconName | undefined;
  /** The shortcut shown at the end (`Kbd`), as `KeyboardEvent.key` names the keys. */
  shortcut?: string[] | undefined;
}
