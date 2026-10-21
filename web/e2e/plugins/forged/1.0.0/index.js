// Test plugin (spec 007, task 15): valid; a journey forges its digest in the capabilities.
import { definePlugin } from "@kartenn/sdk";

definePlugin({ name: "forged", elements: { "forged-view": class extends HTMLElement {} } });
