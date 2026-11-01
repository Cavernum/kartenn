<script setup lang="ts">
// « À propos » (REQ-021-13, REQ-007-10; menu of the user): the versions of the shell, of
// what it shares with the plugins (SDK, @kartenn/ui, framework) and of the server, the time
// zone the dates are shown in, then the plugins of the capabilities — loaded, loading, or
// set aside with their reason and its code — and the `g` letters refused to them. Open
// without a session: what it shows needs no right.
import { SDK_VERSION } from "@kartenn/sdk/host";
import {
  Button,
  CodeBadge,
  EmptyState,
  InlineAlert,
  ShortcutHint,
  Skeleton,
  Stack,
  StatusBadge,
  UI_VERSION,
} from "@kartenn/ui";
import { ref, version as VUE_VERSION } from "vue";
import { useI18n } from "vue-i18n";
import shell from "../../package.json";
import { retryPlugins } from "../plugins/loader.ts";
import {
  reasonCode,
  useCapabilitiesRead,
  usePlugins,
  usePluginsUnavailable,
  useRefusedLetters,
  useServerVersion,
} from "../plugins/registry.ts";
import { useSession } from "../rights/index.ts";

const { t } = useI18n();
const session = useSession();
const read = useCapabilitiesRead();
const plugins = usePlugins();
const unavailable = usePluginsUnavailable();
const refused = useRefusedLetters();
const server = useServerVersion();
const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone;
/** The heading of the plugins: « Réessayer » goes away with its state, the focus goes here. */
const pluginsHeading = ref<HTMLElement | null>(null);
function retry(): void {
  pluginsHeading.value?.focus();
  void retryPlugins();
}
</script>

<template>
  <Stack gap="6" class="k-about">
    <h1 class="k-about__title">{{ t("about.title") }}</h1>
    <section aria-labelledby="k-about-versions">
      <Stack gap="3">
        <h2 id="k-about-versions" class="k-about__heading">{{ t("about.versions") }}</h2>
        <dl class="k-about__facts">
          <dt>{{ t("about.shell") }}</dt>
          <dd class="k-mono">{{ shell.version }}</dd>
          <dt>{{ t("about.sdk") }}</dt>
          <dd class="k-mono">{{ SDK_VERSION }}</dd>
          <dt>{{ t("about.ui") }}</dt>
          <dd class="k-mono">{{ UI_VERSION }}</dd>
          <dt>{{ t("about.framework") }}</dt>
          <dd class="k-mono">{{ VUE_VERSION }}</dd>
          <template v-if="server">
            <dt>{{ t("about.server") }}</dt>
            <dd class="k-mono">{{ server }}</dd>
          </template>
          <dt>{{ t("about.timeZone") }}</dt>
          <dd class="k-mono">{{ timeZone }}</dd>
        </dl>
      </Stack>
    </section>
    <section aria-labelledby="k-about-plugins">
      <Stack gap="3">
        <h2 id="k-about-plugins" ref="pluginsHeading" class="k-about__heading" tabindex="-1">
          {{ t("about.plugins") }}
        </h2>
        <!-- The capabilities could not be read: the plugins are unknown, not absent. -->
        <Stack v-if="unavailable" gap="2" align="start">
          <InlineAlert tone="warn" :title="t('about.pluginsUnavailable')" />
          <Button icon="refresh-cw" @click="retry">{{ t("about.retry") }}</Button>
        </Stack>
        <!-- Not read yet: the capabilities ask for a session; signed in, they come. -->
        <InlineAlert v-else-if="!read && !session.me" :title="t('about.signedOut')" />
        <Stack v-else-if="!read" gap="2" aria-busy="true">
          <span class="k-visually-hidden">{{ t("about.loading") }}</span>
          <Skeleton variant="text" :rows="3" />
        </Stack>
        <ul v-else-if="plugins.length > 0" class="k-about__plugins">
          <li v-for="{ plugin, status } in plugins" :key="plugin.name" class="k-about__plugin">
            <span class="k-about__name">
              <!-- A space between them, read and copied (the gap draws it). -->
              <span class="k-mono">{{ plugin.name }}</span
              >{{ " " }}<span class="k-mono k-about__version">{{ plugin.version }}</span>
            </span>
            <span class="k-about__state">
              <StatusBadge
                v-if="status.state === 'loaded'"
                state="ok"
                :label="t('plugins.loaded')"
              />
              <StatusBadge
                v-else-if="status.state === 'loading'"
                state="neutral"
                :label="t('plugins.loading')"
              />
              <template v-else>
                <StatusBadge state="ko" :label="t('about.rejected')" />
                <!-- The reason a sentence that wraps, never in the badge, which does not. -->
                <span>{{ t(`plugins.reason.${status.reason}`) }}</span>
                <CodeBadge :code="reasonCode(status.reason)" />
              </template>
            </span>
            <!-- A letter taken: the plugin loads, without that shortcut (design « Raccourcis »). -->
            <p
              v-for="letter in refused.get(plugin.name) ?? []"
              :key="letter.view"
              class="k-about__letter"
            >
              <StatusBadge state="warn" :label="t('about.letterRefused')" />
              <ShortcutHint :keys="[['g'], [letter.letter]]" />
              <span>{{ t("about.letterTaken", { view: letter.view }) }}</span>
            </p>
          </li>
        </ul>
        <EmptyState v-else :title="t('about.noPlugins')">{{ t("about.noPluginsText") }}</EmptyState>
      </Stack>
    </section>
  </Stack>
</template>

<style>
@layer components {
  .k-about {
    max-inline-size: var(--k-size-dialog-lg);
  }

  .k-about__title {
    font-size: var(--k-text-lg);
    line-height: var(--k-text-lg-line);
    font-weight: var(--k-weight-semibold);
  }

  .k-about__heading {
    font-size: var(--k-text-md);
    line-height: var(--k-text-md-line);
    font-weight: var(--k-weight-semibold);
  }

  /* Label then value, in two aligned columns. */
  .k-about__facts {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: var(--k-space-1) var(--k-space-4);
  }

  .k-about__facts > dt {
    color: var(--k-text-muted);
  }

  .k-about__plugins {
    display: grid;
    gap: var(--k-space-2);
  }

  /* The plugin at the start, its state after it (below it when the line is too short), a
     refused letter under both. */
  .k-about__plugin {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: var(--k-space-1) var(--k-space-3);
    align-items: center;
    padding-block: var(--k-space-2);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  .k-about__name,
  .k-about__state,
  .k-about__letter {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    align-items: center;
    min-inline-size: 0;
  }

  .k-about__version {
    color: var(--k-text-muted);
  }

  .k-about__letter {
    flex-basis: 100%;
  }
}
</style>
