// Test plugin (spec 021, task 16): expects a major version of @kartenn/ui the shell lacks
// (REQ-021-26): set aside before any of its files is fetched.
import { definePlugin } from "@kartenn/sdk";

definePlugin({
  name: "incompatible",
  elements: { "incompatible-view": class extends HTMLElement {} },
});
