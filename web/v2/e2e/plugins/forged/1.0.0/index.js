// Test plugin (spec 021, task 16): valid; a journey forges its digest in the capabilities.
import { definePlugin } from "@kartenn/sdk";

definePlugin({ name: "forged", elements: { "forged-view": class extends HTMLElement {} } });
