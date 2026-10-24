<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A menu button and its menu (APG Menu Button, Menu; design « Inventaire », line 11):
// Reka's DropdownMenu. Entries carry their icon and their shortcut at the end (`Kbd`,
// `aria-keyshortcuts`); a disabled entry stays listed, with its reason. The trigger is a
// `Button`, or an `IconButton` when `iconOnly`.
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "reka-ui";
import { injectPortalTarget } from "../../context.ts";
import type { IconName } from "../../icons/index.ts";
import Button from "../Button/Button.vue";
import type { ButtonVariant } from "../Button/button.ts";
import { ariaKeyshortcuts } from "../Button/shortcut.ts";
import Icon from "../Icon/Icon.vue";
import IconButton from "../IconButton/IconButton.vue";
import Kbd from "../Kbd/Kbd.vue";
import type { ControlSize } from "../layout.ts";
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

const isEntry = (item: MenuItem): item is MenuEntry => !item.kind || item.kind === "item";
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
            class="k-menu__item"
            :disabled="item.disabled ?? false"
            :data-danger="item.danger ? 'true' : undefined"
            :aria-keyshortcuts="item.shortcut ? ariaKeyshortcuts(item.shortcut) : undefined"
            @select="emit('select', item.id)"
          >
            <Icon v-if="item.icon" :name="item.icon" class="k-menu__icon" />
            <span class="k-menu__text">
              <span>{{ item.label }}</span>
              <span v-if="item.disabled && item.reason" class="k-menu__reason">{{
                item.reason
              }}</span>
            </span>
            <Kbd v-if="item.shortcut" :keys="item.shortcut" aria-hidden="true" />
          </DropdownMenuItem>
        </template>
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

  .k-menu__item[data-disabled] {
    color: var(--k-text-muted);
  }

  .k-menu__icon {
    color: var(--k-text-muted);
  }

  .k-menu__item[data-danger] .k-menu__icon {
    color: currentcolor;
  }

  .k-menu__text {
    display: grid;
    flex: 1;
    padding-block: var(--k-space-1);
  }

  .k-menu__reason {
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
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
