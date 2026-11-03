// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { createApp, h } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { installViewMark, VIEW_MARK } from "../src/prefetch/viewMark.ts";

// The mark `k:view` of the bench (spec 021, REQ-021-22, REQ-021-30; task 22): one per
// navigation after the first, once the view is drawn, none for a navigation refused.

const frame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
const marks = () =>
  performance.getEntriesByName(VIEW_MARK, "mark").map((mark) => (mark as PerformanceMark).detail);

afterEach(() => performance.clearMarks(VIEW_MARK));

describe("k:view", () => {
  it("marks each navigation but the first, after the view is drawn, with its path", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/", component: { render: () => h("p", "home") } },
        { path: "/b", component: { render: () => h("p", { id: "b" }, "b") } },
        { path: "/refused", component: { render: () => h("p") }, beforeEnter: () => false },
      ],
    });
    installViewMark(router);
    const root = document.createElement("div");
    document.body.append(root);
    const app = createApp({ render: () => h(RouterView) }).use(router);
    await router.push("/");
    app.mount(root);
    await frame();
    expect(marks()).toEqual([]);

    await router.push("/b?x=1");
    // Not before the next frame: the view is drawn first.
    expect(marks()).toEqual([]);
    await frame();
    await frame();
    expect(marks()).toEqual(["/b?x=1"]);
    expect(root.querySelector("#b")).not.toBeNull();

    await router.push("/refused");
    await frame();
    await frame();
    expect(marks()).toEqual(["/b?x=1"]);
    app.unmount();
    root.remove();
  });
});
