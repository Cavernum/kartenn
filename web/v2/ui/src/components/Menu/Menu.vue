<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A menu button and its menu (APG Menu Button, Menu; design « Inventaire », line 11):
// Reka's DropdownMenu. Entries carry their icon and their shortcut at the end (`Kbd`,
// `aria-keyshortcuts`). A disabled entry stays listed and reachable by the arrows
// (`aria-disabled`; choosing it does nothing): its reason, or « Il faut <permission> »,
// is its description and a tooltip beside it while it has the focus (024/M14). The
// trigger is a `Button`, or an `IconButton` when `iconOnly`.
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "reka-ui";
import { shallowRef, useId, type Directive } from "vue";
import { injectPortalTarget } from "../../context.ts";
import type { IconName } from "../../icons/index.ts";
import Button from "../Button/Button.vue";
import type { ButtonVariant } from "../Button/button.ts";
import { ariaKeyshortcuts } from "../Button/shortcut.ts";
import Icon from "../Icon/Icon.vue";
import IconButton from "../IconButton/IconButton.vue";
import BlockedReason from "../internal/BlockedReason.vue";
import Kbd from "../Kbd/Kbd.vue";
import type { ControlSize } from "../layout.ts";
import Tooltip from "../Tooltip/Tooltip.vue";
import { useTooltip } from "../Tooltip/tooltip.ts";
import type { MenuEntry, MenuItem } from "./menu.ts";

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
const emit = defineEmits<{ select: [id: string] }>();
const portal = injectPortalTarget();
const base = useId();

const isEntry = (item: MenuItem): item is MenuEntry => !item.kind || item.kind === "item";
const isBlocked = (entry: MenuEntry) => !!(entry.disabled || entry.forbidden);
const hasReason = (entry: MenuEntry) => isBlocked(entry) && !!(entry.forbidden || entry.reason);
const reasonId = (entry: MenuEntry) => `${base}-${entry.id}-why`;

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
  // A disabled entry keeps the menu open and runs nothing.
  if (isBlocked(entry)) event.preventDefault();
  else emit("select", entry.id);
}
</script>

<template>
  <DropdownMenuRoot :default-open="props.defaultOpen" :modal="false">
    <DropdownMenuTrigger as-child>
      <IconButton
        v-if="props.iconOnly"
        :icon="props.icon ?? 'ellipsis'"
        :label="props.label"
        :variant="props.variant"
        :size="props.size"
        :data-chevron="props.icon === 'chevron-down' ? 'true' : undefined"
      />
      <Button v-else :icon="props.icon" :variant="props.variant" :size="props.size">
        {{ props.label }}<Icon name="chevron-down" class="k-menu__chevron" />
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuPortal :to="portal">
      <DropdownMenuContent class="k-menu" :align="props.align" :side-offset="4">
        <template v-for="(item, i) in props.items" :key="isEntry(item) ? item.id : i">
          <DropdownMenuSeparator v-if="item.kind === 'separator'" class="k-menu__separator" />
          <DropdownMenuLabel v-else-if="item.kind === 'label'" class="k-menu__label">
            {{ item.label }}
          </DropdownMenuLabel>
          <DropdownMenuItem
            v-else-if="isEntry(item)"
            v-aria-disabled="isBlocked(item)"
            class="k-menu__item"
            :data-danger="item.danger ? 'true' : undefined"
            :data-blocked="isBlocked(item) ? 'true' : undefined"
            :aria-describedby="hasReason(item) ? reasonId(item) : undefined"
            :aria-keyshortcuts="item.shortcut ? ariaKeyshortcuts(item.shortcut) : undefined"
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
            <Kbd v-if="item.shortcut" :keys="item.shortcut" aria-hidden="true" />
          </DropdownMenuItem>
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

  .k-menu {
    z-index: var(--k-z-overlay);
    display: grid;
    min-inline-size: var(--k-size-menu-min);
    max-inline-size: var(--k-size-panel-md);
    padding: var(--k-space-1);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-lg);
    box-shadow: var(--k-shadow-overlay);
    transition-property: opacity, translate;
    transition-duration: max(var(--k-duration-short), var(--k-duration-fade));
    transition-timing-function: var(--k-ease-standard);
  }

  /* Motion (design « Mouvement »): the menu comes 4 px out of its anchor, then fades out.
   * Interactive from its first frame; Reka focuses its first entry at once. */
  @starting-style {
    .k-menu {
      opacity: 0;
      translate: 0 calc(var(--k-motion-shift-sm) * -1);
    }

    .k-menu[data-side="top"] {
      translate: 0 var(--k-motion-shift-sm);
    }
  }

  .k-menu[data-state="closed"] {
    pointer-events: none;
    animation-name: k-fade-out;
    animation-duration: var(--k-duration-fade);
    animation-timing-function: var(--k-ease-exit);
    animation-fill-mode: forwards;
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
