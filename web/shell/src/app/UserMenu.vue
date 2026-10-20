<script setup lang="ts">
import {
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
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { LOCALES, setLocale, type Locale } from "../i18n/index.ts";
import { useSession } from "../rights/index.ts";
import { applyTheme, storedTheme, THEMES, type Theme } from "../theme.ts";

const composer = useI18n({ useScope: "global" });
const { t, locale } = composer;
const session = useSession();
const theme = ref<Theme>(storedTheme());
const logoutForm = ref<HTMLFormElement | null>(null);
const shown = computed(() => session.value.me?.name ?? t("menu.anonymous"));

function chooseLocale(value: unknown): void {
  setLocale(composer, value as Locale);
}

function chooseTheme(value: unknown): void {
  theme.value = value as Theme;
  applyTheme(theme.value);
}

// A top-level form POST, which follows the redirection to the issuer (design « Session »).
function logout(): void {
  logoutForm.value?.submit();
}
</script>

<template>
  <!-- Not modal: a modal menu hides the page with aria-hidden while its links stay
       focusable (axe-core aria-hidden-focus); the APG menu pattern needs no modality. -->
  <DropdownMenuRoot :modal="false">
    <!-- The visible text starts the accessible name (WCAG 2.5.3, label in name). -->
    <DropdownMenuTrigger :aria-label="t('menu.user', { name: shown })">
      {{ shown }}
    </DropdownMenuTrigger>
    <DropdownMenuPortal>
      <DropdownMenuContent class="k-menu" align="end" :side-offset="4">
        <DropdownMenuLabel class="k-menu-label">{{ t("menu.language") }}</DropdownMenuLabel>
        <DropdownMenuRadioGroup :model-value="locale" @update:model-value="chooseLocale">
          <DropdownMenuRadioItem
            v-for="code in LOCALES"
            :key="code"
            :value="code"
            :lang="code"
            class="k-menu-item"
          >
            {{ t(`locale.${code}`) }}
            <DropdownMenuItemIndicator>✓</DropdownMenuItemIndicator>
          </DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
        <DropdownMenuSeparator class="k-menu-separator" />
        <DropdownMenuLabel class="k-menu-label">{{ t("menu.theme") }}</DropdownMenuLabel>
        <DropdownMenuRadioGroup :model-value="theme" @update:model-value="chooseTheme">
          <DropdownMenuRadioItem
            v-for="name in THEMES"
            :key="name"
            :value="name"
            class="k-menu-item"
          >
            {{ t(`theme.${name}`) }}
            <DropdownMenuItemIndicator>✓</DropdownMenuItemIndicator>
          </DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
        <template v-if="session.me">
          <DropdownMenuSeparator class="k-menu-separator" />
          <DropdownMenuItem class="k-menu-item" @select="logout">
            {{ t("menu.logout") }}
          </DropdownMenuItem>
        </template>
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
  <form ref="logoutForm" method="post" action="/v1/auth/logout" hidden></form>
</template>
