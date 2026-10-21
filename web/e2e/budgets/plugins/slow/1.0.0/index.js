// Plugin of the browser bench (spec 007, REQ-007-32): it calls definePlugin after 6 s,
// past the 5 s the shell grants; it is set aside without delaying the first screen.
import { definePlugin } from "@kartenn/sdk";

class SlowView extends HTMLElement {}

setTimeout(() => definePlugin({ name: "slow", elements: { "slow-view": SlowView } }), 6_000);
