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
}

export interface ButtonProps extends ButtonBaseProps {
  /** A leading icon, decorative: the label names the button. */
  icon?: IconName | undefined;
  /** The shortcut shown at the end (`Kbd`), as `KeyboardEvent.key` names the keys. */
  shortcut?: string[] | undefined;
}
