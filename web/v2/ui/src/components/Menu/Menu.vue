<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A menu button and its menu (APG Menu Button, Menu; design « Inventaire », line 11):
// Reka's DropdownMenu, one component (the menu carries its button). Enter, Space and Down
// open it on its first entry, Up on its last; the arrows go round, Home and End, typing
// ahead; Escape closes it and gives the focus back to the button; Tab and Shift+Tab close
// it and go on from the button (APG Menu: Reka, not modal, would leave the focus at the
// end of the page). Entries carry their icon and their shortcut at the end
// (`ShortcutHint`: a combination, also in `aria-keyshortcuts`; a sequence, « g puis p »,
// in the entry's description). A disabled entry stays listed and reachable by the arrows
// (`aria-disabled`; choosing it does nothing): its reason, or « Il faut <permission> »,
// is its description and a tooltip beside it while it has the focus (024/M14). The
// trigger is a `Button`, or an `IconButton` when `iconOnly`. A checkbox entry
// (`menuitemcheckbox`, `aria-checked`) keeps the menu open when chosen (a column chooser).
// Radio entries (`menuitemradio` in a `group`, `aria-checked`) say which one of a set is on
// (the active draft); choosing one closes the menu.
import {
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuItemIndicator,
  DropdownMenuLabel,
  DropdownMenuPortal,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuRoot,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "reka-ui";
import {
  computed,
  ref,
  shallowRef,
  useId,
  type ComponentPublicInstance,
  type Directive,
} from "vue";
import { injectPortalTarget } from "../../context.ts";
import type { IconName } from "../../icons/index.ts";
import Button from "../Button/Button.vue";
import type { ButtonVariant } from "../Button/button.ts";
import Icon from "../Icon/Icon.vue";
import IconButton from "../IconButton/IconButton.vue";
import BlockedReason from "../internal/BlockedReason.vue";
import { besideInTabOrder } from "../internal/tabOrder.ts";
import type { ControlSize } from "../layout.ts";
import ShortcutHint from "../ShortcutHint/ShortcutHint.vue";
import { shortcutAria, shortcutSteps } from "../ShortcutHint/shortcutHint.ts";
import Tooltip from "../Tooltip/Tooltip.vue";
import { useTooltip } from "../Tooltip/tooltip.ts";
import { menuBlocks, type MenuEntry, type MenuItem } from "./menu.ts";

const props = withDefaults(
  defineProps<{
    label: string;
    items: MenuItem[];
    icon?: IconName | undefined;
    iconOnly?: boolean;
    variant?: ButtonVariant;
    size?: ControlSize;
    align?: "start" | "end";
    defaultOpen?: boolean;
  }>(),
  { icon: undefined, variant: "secondary", size: "md", align: "start" },
);
const emit = defineEmits<{ select: [id: string]; open: [open: boolean] }>();
const portal = injectPortalTarget();
const base = useId();
const open = ref(props.defaultOpen);
const trigger = ref<ComponentPublicInstance | null>(null);

function onOpen(now: boolean): void {
  open.value = now;
  emit("open", now);
}

const isEntry = (item: MenuItem): item is MenuEntry => !item.kind || item.kind === "item";
/** The items, each run of radio entries in a group of its own. */
const blocks = computed(() => menuBlocks(props.items));
const isBlocked = (entry: MenuEntry) => !!(entry.disabled || entry.forbidden);
const hasReason = (entry: MenuEntry) => isBlocked(entry) && !!(entry.forbidden || entry.reason);
const reasonId = (entry: MenuEntry) => `${base}-${entry.id}-why`;
const isSequence = (entry: MenuEntry) =>
  !!entry.shortcut && shortcutSteps(entry.shortcut).length > 1;
const keysId = (entry: MenuEntry) => `${base}-${entry.id}-keys`;
/** The description of an entry: its reason, then a sequence of keys said in words. */
function describedBy(entry: MenuEntry): string | undefined {
  const ids = [hasReason(entry) && reasonId(entry), isSequence(entry) && keysId(entry)];
  return ids.filter(Boolean).join(" ") || undefined;
}

/** Up on the closed button opens the menu on its last entry (APG Menu Button). */
let fromEnd = false;
function onTriggerKey(event: KeyboardEvent): void {
  if (event.key !== "ArrowUp" || open.value) return;
  event.preventDefault();
  fromEnd = true;
  onOpen(true);
}
function onOpenFocus(event: Event): void {
  if (!fromEnd) return;
  fromEnd = false;
  event.preventDefault();
  const entries = (event.target as HTMLElement).querySelectorAll<HTMLElement>("[role^='menuitem']");
  entries[entries.length - 1]?.focus();
}

/** Tab, Shift+Tab: the menu closes, the focus goes on from the button (APG Menu). */
let tabbed: 1 | -1 | 0 = 0;
let surface: HTMLElement | null = null;
function onMenuKey(event: KeyboardEvent): void {
  if (event.key !== "Tab" || event.ctrlKey || event.altKey || event.metaKey) return;
  event.preventDefault();
  tabbed = event.shiftKey ? -1 : 1;
  surface = event.currentTarget as HTMLElement;
  onOpen(false);
}
function onCloseFocus(event: Event): void {
  const step = tabbed;
  tabbed = 0;
  const button = trigger.value?.$el as HTMLElement | undefined;
  if (!step || !button) return;
  event.preventDefault();
  (besideInTabOrder(button, step, surface) ?? button).focus();
}

/** One tooltip for the menu: the reason of the disabled entry that has the focus. */
const tip = useTooltip();
const shown = shallowRef<MenuEntry | null>(null);
function onFocus(entry: MenuEntry, event: FocusEvent): void {
  if (!hasReason(entry)) return;
  shown.value = entry;
  tip.show(event.currentTarget as HTMLElement);
}

/**
 * `aria-disabled` on a disabled entry: Reka writes the attribute from its own `disabled`
 * prop, which would also take the entry out of the arrows' reach; the directive sets it
 * on the element after Reka's render.
 */
function ariaDisabled(el: HTMLElement, { value }: { value: boolean }): void {
  if (value) el.setAttribute("aria-disabled", "true");
  else el.removeAttribute("aria-disabled");
}
const vAriaDisabled: Directive<HTMLElement, boolean> = {
  mounted: ariaDisabled,
  updated: ariaDisabled,
};

function onSelect(entry: MenuEntry, event: Event): void {
  // A disabled entry keeps the menu open and runs nothing; chosen by a tap, where no focus
  // came, it shows its reason (« Tactile »: nothing reserved to hovering or focus).
  if (isBlocked(entry)) {
    event.preventDefault();
    // Reka's select event is not dispatched: the entry is found by its reason's id.
    const item = hasReason(entry)
      ? document.getElementById(reasonId(entry))?.closest<HTMLElement>("[role='menuitem']")
      : null;
    if (item) {
      shown.value = entry;
      tip.pin(item);
    }
  } else emit("select", entry.id);
}
</script>

<template>
  <DropdownMenuRoot :open="open" :modal="false" @update:open="onOpen">
    <DropdownMenuTrigger ref="trigger" as-child>
      <IconButton
        v-if="props.iconOnly"
        :icon="props.icon ?? 'ellipsis'"
        :label="props.label"
        :variant="props.variant"
        :size="props.size"
        :data-chevron="props.icon === 'chevron-down' ? 'true' : undefined"
        @keydown="onTriggerKey"
      />
      <Button
        v-else
        :icon="props.icon"
        :variant="props.variant"
        :size="props.size"
        @keydown="onTriggerKey"
      >
        {{ props.label }}<Icon name="chevron-down" class="k-menu__chevron" />
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuPortal :to="portal">
      <DropdownMenuContent
        class="k-popover k-menu"
        :align="props.align"
        :side-offset="4"
        loop
        @open-auto-focus="onOpenFocus"
        @close-auto-focus="onCloseFocus"
        @keydown="onMenuKey"
      >
        <template v-for="block in blocks" :key="block.key">
          <DropdownMenuRadioGroup
            v-if="block.kind === 'radios'"
            class="k-menu__group"
            :model-value="block.value"
          >
            <DropdownMenuRadioItem
              v-for="radio in block.radios"
              :key="radio.id"
              class="k-menu__item"
              :value="radio.id"
              @select="emit('select', radio.id)"
            >
              <span class="k-menu__check">
                <DropdownMenuItemIndicator><Icon name="check" /></DropdownMenuItemIndicator>
              </span>
              <span class="k-menu__text">{{ radio.label }}</span>
            </DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>
          <!-- Any other item, under its own name. -->
          <template v-else>
            <template v-for="item in [block.item]" :key="item.kind ?? 'item'">
              <DropdownMenuSeparator v-if="item.kind === 'separator'" class="k-menu__separator" />
              <DropdownMenuLabel v-else-if="item.kind === 'label'" class="k-menu__label">
                {{ item.label }}
              </DropdownMenuLabel>
              <DropdownMenuCheckboxItem
                v-else-if="item.kind === 'checkbox'"
                class="k-menu__item"
                :model-value="item.checked"
                @select="
                  (event: Event) => {
                    event.preventDefault();
                    emit('select', item.id);
                  }
                "
              >
                <span class="k-menu__check">
                  <DropdownMenuItemIndicator><Icon name="check" /></DropdownMenuItemIndicator>
                </span>
                <span class="k-menu__text">{{ item.label }}</span>
              </DropdownMenuCheckboxItem>
              <DropdownMenuItem
                v-else-if="isEntry(item)"
                v-aria-disabled="isBlocked(item)"
                class="k-menu__item"
                :data-danger="item.danger ? 'true' : undefined"
                :data-blocked="isBlocked(item) ? 'true' : undefined"
                :aria-describedby="describedBy(item)"
                :aria-keyshortcuts="item.shortcut ? shortcutAria(item.shortcut) : undefined"
                @select="(event: Event) => onSelect(item, event)"
                @focusin="(event: FocusEvent) => onFocus(item, event)"
                @focusout="tip.hide()"
              >
                <Icon v-if="item.icon" :name="item.icon" class="k-menu__icon" />
                <span class="k-menu__text">{{ item.label }}</span>
                <!-- The description only: hidden from the name, read through aria-describedby. -->
                <span
                  v-if="hasReason(item)"
                  :id="reasonId(item)"
                  class="k-visually-hidden"
                  aria-hidden="true"
                  ><BlockedReason :forbidden="item.forbidden" :reason="item.reason"
                /></span>
                <Icon v-if="item.forbidden" name="lock" class="k-menu__icon" />
                <!-- Hidden from the name; a sequence is the entry's description. -->
                <ShortcutHint
                  v-if="item.shortcut"
                  :id="isSequence(item) ? keysId(item) : undefined"
                  :keys="item.shortcut"
                  aria-hidden="true"
                />
              </DropdownMenuItem>
            </template>
          </template>
        </template>
        <Tooltip :control="tip" side="inline-end" decorative>
          <BlockedReason v-if="shown" :forbidden="shown.forbidden" :reason="shown.reason" />
        </Tooltip>
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>

<style>
@layer components {
  .k-menu__chevron {
    margin-inline-end: calc(var(--k-space-1) * -1);
    color: var(--k-text-muted);
  }

  /* The surface and its motion are the popover's (`k-popover`, Popover/surface.css). */
  .k-menu {
    display: grid;
    min-inline-size: var(--k-size-menu-min);
    max-inline-size: var(--k-size-panel-md);
    padding: var(--k-space-1);
  }

  /* The chevron of the trigger (a glyph of its own, or the icon of a chevron-only button)
   * turns while its menu is open; the glyph alone. */
  .k-menu__chevron,
  .k-icon-button[data-chevron] > .k-icon {
    transition-property: rotate;
    transition-duration: var(--k-duration-short);
    transition-timing-function: var(--k-ease-standard);
  }

  .k-button[aria-expanded="true"] .k-menu__chevron,
  .k-icon-button[data-chevron][aria-expanded="true"] > .k-icon {
    rotate: var(--k-turn-half);
  }

  .k-menu__item {
    display: flex;
    gap: var(--k-space-2);
    align-items: center;
    min-block-size: var(--k-control-height-md);
    padding-inline: var(--k-space-2);
    color: var(--k-text);
    cursor: default;
    border-radius: var(--k-radius-md);
    outline: none;
  }

  .k-menu__item[data-highlighted] {
    background-color: var(--k-accent-soft);
    box-shadow: var(--k-shadow-inset-accent);
  }

  .k-menu__item[data-danger] {
    color: var(--k-state-ko-fg);
  }

  /* Disabled, still reachable: muted, its reason in its tooltip. */
  .k-menu__item[data-blocked] {
    color: var(--k-text-muted);
    cursor: not-allowed;
  }

  .k-menu__icon {
    color: var(--k-text-muted);
  }

  .k-menu__item[data-danger] .k-menu__icon {
    color: currentcolor;
  }

  .k-menu__group {
    display: grid;
  }

  /* The tick of a checkbox or radio entry, in a box of an icon's width: labels align. */
  .k-menu__check {
    display: inline-flex;
    flex: none;
    inline-size: var(--k-icon-size);
    color: var(--k-accent);
  }

  .k-menu__text {
    flex: 1;
    padding-block: var(--k-space-1);
  }

  .k-menu__label {
    padding: var(--k-space-1) var(--k-space-2);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-weight: var(--k-weight-medium);
    color: var(--k-text-muted);
  }

  .k-menu__separator {
    block-size: var(--k-border-width);
    margin-block: var(--k-space-1);
    background-color: var(--k-border);
  }
}
</style>
