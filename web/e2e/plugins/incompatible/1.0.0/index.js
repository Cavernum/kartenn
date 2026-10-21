// Test plugin (spec 007, task 15): expects a major version of the SDK the shell lacks.
import { definePlugin } from "@kartenn/sdk";

definePlugin({
  name: "incompatible",
  elements: { "incompatible-view": class extends HTMLElement {} },
});
