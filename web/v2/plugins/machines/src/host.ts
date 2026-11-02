// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The thin host of a first-party plugin in Vue (ADR 0038, decisions 3 and 4; spec 021, A23,
// task 18ca): each Custom Element of the plugin mounts an application of the shared `vue`
// (`createApp`) in its light DOM — no shadow root, no sheet of its own: the sheet of
// @kartenn/ui that the document links applies, and floating layers go where the shell puts
// its own. The application receives the context of the SDK (followed while it lives), the
// `QueryClient` of the SDK and the texts of the plugin's handle; its root component gets the
// element's `object` and `row`. Links are plain `<a href>`, followed by the shell's
// `ExtensionElement` (design 021 « Liens d'un plugin »).
// Written for every first-party plugin (nothing here names the machines): to move as is into
// @kartenn/ui (A23), with `data.ts`.

import {
  context,
  definePlugin,
  queryClient,
  type Context,
  type ObjectRef,
  type PluginDefinition,
  type PluginHandle,
  type QueryClient,
} from "@kartenn/sdk";
import { providePortalTarget } from "@kartenn/ui";
import {
  createApp,
  defineComponent,
  h,
  inject,
  shallowRef,
  type App,
  type Component,
  type InjectionKey,
  type ShallowRef,
} from "vue";

/** The layer of the shell's frame where its menus, lists and tooltips go (`ShellFrame`). */
const SHELL_LAYERS = "#k-layers";

/** What the element gives its application, by `useHosted`. */
export interface Hosted {
  /** The Custom Element that holds the application (its events go out from it). */
  readonly element: HTMLElement;
  /** The context of the SDK, set again at each of its changes (language, dates, user). */
  readonly context: Readonly<ShallowRef<Readonly<Context>>>;
  readonly queryClient: QueryClient;
  /** A text of the plugin under its namespace, in the language of the context. */
  t(key: string, params?: Record<string, unknown>): string;
}

const HOSTED: InjectionKey<Hosted> = Symbol("kartenn.hosted");

/** The properties the root component of an element receives (W5: `object`, a column's `row`). */
export interface HostedProps {
  object: ObjectRef | null;
  row: Record<string, unknown> | null;
}

/** The host of one plugin: its elements, then its definition. */
export interface PluginHost {
  /** The Custom Element that mounts `component` (props `HostedProps`) in its light DOM. */
  element(component: Component): CustomElementConstructor;
  /** `definePlugin` of the SDK, the handle kept for the texts of the applications. */
  define(definition: PluginDefinition): PluginHandle;
}

/** Counts the applications mounted: each has its own prefix of identifiers. */
let mounted = 0;

export function pluginHost(): PluginHost {
  let handle: PluginHandle | null = null;
  return {
    define(definition) {
      handle = definePlugin(definition);
      return handle;
    },
    element(component) {
      return class HostedElement extends HTMLElement {
        readonly #object = shallowRef<ObjectRef | null>(null);
        readonly #row = shallowRef<Record<string, unknown> | null>(null);
        #app: App | null = null;
        readonly #context = shallowRef(context.get());
        #stop: (() => void) | null = null;
        #settling = false;

        constructor() {
          super();
          // A property set on the element before its definition (an upgrade) hides the
          // accessor: taken back through it.
          for (const key of ["object", "row"] as const) {
            if (Object.hasOwn(this, key)) {
              const value = (this as Record<string, unknown>)[key];
              delete (this as Record<string, unknown>)[key];
              (this as Record<string, unknown>)[key] = value;
            }
          }
        }

        get object(): ObjectRef | null {
          return this.#object.value;
        }

        set object(value: ObjectRef | null | undefined) {
          this.#object.value = value ?? null;
        }

        get row(): Record<string, unknown> | null {
          return this.#row.value;
        }

        set row(value: Record<string, unknown> | null | undefined) {
          this.#row.value = value ?? null;
        }

        connectedCallback(): void {
          // Moved in the tree: the application kept follows the context again.
          if (this.#app && !this.#stop) this.#follow();
          this.#settle();
        }

        disconnectedCallback(): void {
          // At once, before its unmounting: out of the page, its reads move no more.
          this.#stop?.();
          this.#stop = null;
          this.#settle();
        }

        /**
         * Mounted or unmounted after a microtask, never within the call: the element is
         * connected while the shell's renderer inserts it (a cell of a grid), and an
         * application mounted or unmounted there would flush the shell's pending callbacks
         * in the middle of its patch; an element moved in the tree (disconnected then
         * connected at once) keeps its application.
         */
        #settle(): void {
          if (this.#settling) return;
          this.#settling = true;
          queueMicrotask(() => {
            this.#settling = false;
            if (this.isConnected && !this.#app) this.#mount();
            else if (!this.isConnected && this.#app) this.#unmount();
          });
        }

        /** The context of the SDK, followed into `#context` until the disconnection. */
        #follow(): void {
          this.#context.value = context.get();
          this.#stop = context.subscribe((next) => (this.#context.value = next));
        }

        #mount(): void {
          this.#follow();
          const current = this.#context;
          const hosted: Hosted = {
            element: this,
            context: current,
            queryClient,
            t(key, params) {
              // Read again when the language changes.
              void current.value.locale;
              return handle ? handle.t(key, params) : key;
            },
          };
          const props = (): HostedProps => ({ object: this.#object.value, row: this.#row.value });
          const root = defineComponent({
            name: "KartennHosted",
            setup() {
              providePortalTarget(document.querySelector(SHELL_LAYERS) ? SHELL_LAYERS : "body");
              return () => h(component, props());
            },
          });
          const app = createApp(root);
          // The identifiers of `useId` are counted per application: a prefix of its own
          // keeps them apart from the shell's and from the other elements'.
          app.config.idPrefix = `${this.localName}-${++mounted}`;
          app.config.errorHandler = (error, _instance, info) =>
            console.error(`kartenn: ${this.localName}: ${info}:`, error);
          app.provide(HOSTED, hosted);
          app.mount(this);
          this.#app = app;
        }

        #unmount(): void {
          this.#app?.unmount();
          this.#app = null;
          this.#stop?.();
          this.#stop = null;
        }
      };
    },
  };
}

/** What the element hosting the application gives it; only within such an application. */
export function useHosted(): Hosted {
  const hosted = inject(HOSTED, null);
  if (!hosted) throw new Error("useHosted: outside the application of a plugin element");
  return hosted;
}

/** The function of the texts of the plugin, reactive to the language. */
export function useTexts(): Hosted["t"] {
  return useHosted().t;
}

/** Whether the shell looks at the present: writes are offered only then (W9), reactive. */
export function usePresent(): () => boolean {
  const { context: current } = useHosted();
  return () => current.value.time.present;
}

/** Asks the host to close the element's place (an action in its dialog: `kartenn-close`). */
export function useClose(): () => void {
  const { element } = useHosted();
  return () => element.dispatchEvent(new Event("kartenn-close", { bubbles: true, composed: true }));
}
