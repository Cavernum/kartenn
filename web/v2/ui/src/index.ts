// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * `@kartenn/ui`: the design system of kartenn (spec 020) — tokens, cascade layers, fonts,
 * icons and components. The shell and first-party plugins share one instance through
 * the import map (ADR 0038); the sheet is `@kartenn/ui/style.css`.
 */

import "./style.css";

/** Version of the library. */
export const UI_VERSION = "0.1.0";

export { tokens, STATES, ENV_HUES } from "./tokens/index.ts";
export type { Token, State } from "./tokens/index.ts";

export { currentLocale, setLocale, t } from "./i18n.ts";
export type { Locale, MessageKey } from "./i18n.ts";
export { copyText } from "./clipboard.ts";
export { provideLink, providePortalTarget } from "./context.ts";
export { formatInstant } from "./time.ts";
export { applyMotion, currentMotion } from "./motion.ts";
export type { MotionSetting } from "./motion.ts";
export type { CopyOutcome } from "./clipboard.ts";
export type { Align, ControlSize, Space, WidthClass } from "./components/layout.ts";
export { CONTAINER_WIDTHS, WIDTH_CLASSES, widthClass, widthQuery } from "./components/layout.ts";

export { default as Stack } from "./components/Stack/Stack.vue";
export { default as Cluster } from "./components/Cluster/Cluster.vue";
export { default as Grid } from "./components/Grid/Grid.vue";
export { default as Split } from "./components/Split/Split.vue";
export { default as Button } from "./components/Button/Button.vue";
export type { ButtonProps, ButtonVariant } from "./components/Button/button.ts";
export { default as IconButton } from "./components/IconButton/IconButton.vue";
export { default as TextField } from "./components/TextField/TextField.vue";
export { default as FormField } from "./components/FormField/FormField.vue";
export { default as Checkbox } from "./components/Checkbox/Checkbox.vue";
export { default as Switch } from "./components/Switch/Switch.vue";
export { default as Select } from "./components/Select/Select.vue";
export type { ListOption } from "./components/Select/select.ts";
export { default as Combobox } from "./components/Combobox/Combobox.vue";
export { filterOptions } from "./components/Combobox/combobox.ts";
export type { Suggest } from "./components/Combobox/combobox.ts";
export { default as Kbd } from "./components/Kbd/Kbd.vue";
export { keyLabel } from "./components/Kbd/kbd.ts";
export { default as ShortcutHint } from "./components/ShortcutHint/ShortcutHint.vue";
export { shortcutAria, shortcutSteps } from "./components/ShortcutHint/shortcutHint.ts";
export type { Shortcut } from "./components/ShortcutHint/shortcutHint.ts";
export { default as StatusBadge } from "./components/StatusBadge/StatusBadge.vue";
export { default as KeyLabel } from "./components/KeyLabel/KeyLabel.vue";
export { default as EnvChip } from "./components/EnvChip/EnvChip.vue";
export { default as CodeBadge } from "./components/CodeBadge/CodeBadge.vue";
export { default as Breadcrumb } from "./components/Breadcrumb/Breadcrumb.vue";
export type { Crumb } from "./components/Breadcrumb/breadcrumb.ts";
export { default as SidebarNav } from "./components/SidebarNav/SidebarNav.vue";
export type { SidebarEntry, SidebarGroup } from "./components/SidebarNav/sidebar.ts";
export { default as DatePill } from "./components/DatePill/DatePill.vue";
export { default as DraftCounter } from "./components/DraftCounter/DraftCounter.vue";
export type { DraftSummary } from "./components/DraftCounter/draft.ts";
export { default as TimeBand } from "./components/TimeBand/TimeBand.vue";
export { default as Tabs } from "./components/Tabs/Tabs.vue";
export type { TabItem } from "./components/Tabs/tabs.ts";
export { default as Menu } from "./components/Menu/Menu.vue";
export type {
  MenuCheckbox,
  MenuDivider,
  MenuEntry,
  MenuItem,
  MenuRadio,
} from "./components/Menu/menu.ts";
export { default as Dialog } from "./components/Dialog/Dialog.vue";
export { default as AlertDialog } from "./components/AlertDialog/AlertDialog.vue";
export { default as Drawer } from "./components/Drawer/Drawer.vue";
export { default as SidePanel } from "./components/SidePanel/SidePanel.vue";
export { default as BottomPanel } from "./components/BottomPanel/BottomPanel.vue";
export { default as CommitForm } from "./components/CommitForm/CommitForm.vue";
export { default as SplitButton } from "./components/SplitButton/SplitButton.vue";
export type { SplitAction } from "./components/SplitButton/split.ts";
export { default as Toolbar } from "./components/Toolbar/Toolbar.vue";
export { foldActions } from "./components/Toolbar/toolbar.ts";
export type { ToolbarAction } from "./components/Toolbar/toolbar.ts";
export { default as Tooltip } from "./components/Tooltip/Tooltip.vue";
export { useTooltip } from "./components/Tooltip/tooltip.ts";
export type { TooltipControl } from "./components/Tooltip/tooltip.ts";
export { default as CommandPalette } from "./components/CommandPalette/CommandPalette.vue";
export type { PaletteGroup, PaletteItem } from "./components/CommandPalette/palette.ts";
export { default as DataGrid } from "./components/DataGrid/DataGrid.vue";
export { columnsTemplate, nextSort } from "./components/DataGrid/grid.ts";
export type {
  CellRef,
  CellState,
  GridColumn,
  GridSort,
  RowState,
} from "./components/DataGrid/grid.ts";
export { default as ExpandableGrid } from "./components/ExpandableGrid/ExpandableGrid.vue";
export { default as MatrixGrid } from "./components/MatrixGrid/MatrixGrid.vue";
export type { MatrixAxis, MatrixCell, MatrixCellState } from "./components/MatrixGrid/matrix.ts";
export { moveInGrid } from "./components/internal/gridKeys.ts";
export type { GridPosition } from "./components/internal/gridKeys.ts";
export { default as TreeGrid } from "./components/TreeGrid/TreeGrid.vue";
export { flatten, misplaced } from "./components/TreeGrid/tree.ts";
export type {
  MoveCheck,
  ParentMode,
  TreeNode,
  TreeNodeState,
  TreeRow,
} from "./components/TreeGrid/tree.ts";
export { default as FilterChip } from "./components/FilterChip/FilterChip.vue";
export { default as FilterPicker } from "./components/FilterPicker/FilterPicker.vue";
export {
  arity,
  checkFilter,
  checkValue,
  filterSentence,
  filtersFromParams,
  filtersToParams,
  operatorLabel,
} from "./components/FilterPicker/filters.ts";
export type {
  Filter,
  FilterModel,
  FilterOperator,
  FilterProperty,
  FilterType,
} from "./components/FilterPicker/filters.ts";
export { default as DateTimeField } from "./components/DateTimeField/DateTimeField.vue";
export { default as PeriodPicker } from "./components/PeriodPicker/PeriodPicker.vue";
export { checkEntry, parseEntry } from "./components/DateTimeField/dates.ts";
export type { DateCheck, DateGranularity, Period } from "./components/DateTimeField/dates.ts";
export { default as BarChart } from "./components/BarChart/BarChart.vue";
export { default as BarList } from "./components/BarList/BarList.vue";
export { layoutBars, niceCeil } from "./components/BarChart/chart.ts";
export type { BarLayout, ChartPoint } from "./components/BarChart/chart.ts";
export { default as DiffList } from "./components/DiffList/DiffList.vue";
export { default as DiffValue } from "./components/DiffValue/DiffValue.vue";
export { DIFF_SIGNS } from "./components/DiffList/diff.ts";
export type { DiffChange, DiffGroup, DiffKind } from "./components/DiffList/diff.ts";
export { default as Skeleton } from "./components/Skeleton/Skeleton.vue";
export { default as EmptyState } from "./components/EmptyState/EmptyState.vue";
export { default as ErrorState } from "./components/ErrorState/ErrorState.vue";
export { default as InlineAlert } from "./components/InlineAlert/InlineAlert.vue";
export type { AlertTone } from "./components/InlineAlert/alert.ts";
export { default as Icon } from "./components/Icon/Icon.vue";
export type { IconProps } from "./components/Icon/icon.ts";
export { icons, lucideIcons, metierIcons } from "./icons/index.ts";
export type { IconName, IconNode, IconSize } from "./icons/index.ts";
