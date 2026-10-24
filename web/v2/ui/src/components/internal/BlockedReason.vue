<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// Why an action is refused (024/M14): « Il faut <permission> », the permission in fixed
// pitch, or the reason given by the host (a policy that holds the changeset).
import { t } from "../../i18n.ts";

const props = defineProps<{ forbidden?: string | undefined; reason?: string | undefined }>();
</script>

<template>
  <template v-if="props.forbidden"
    >{{ t("needs") }} <code>{{ props.forbidden }}</code></template
  >
  <template v-else>{{ props.reason }}</template>
</template>
