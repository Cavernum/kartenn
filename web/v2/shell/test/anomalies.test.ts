// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { nextTick, ref } from "vue";
import { useFocusWhenEmptied } from "../src/views/anomalies/anomaly.ts";

// The focus after the last anomaly of a list is acknowledged (task 021-11i, 2.4.3): the
// grid replaced by the empty state, the focus would fall to the body; armed by the
// acknowledgement, the next rows read, if empty, put it on the empty state.

const settle = async () => {
  await nextTick();
  await nextTick();
};

afterEach(() => {
  document.body.replaceChildren();
});

function setup() {
  const rows = ref<unknown[]>([{ id: "a" }]);
  const empty = document.createElement("div");
  empty.tabIndex = -1;
  document.body.append(empty);
  const target = ref<HTMLElement | null>(empty);
  const { arm } = useFocusWhenEmptied(() => rows.value, target);
  return { rows, empty, arm };
}

describe("useFocusWhenEmptied", () => {
  it("armed, the list read again empty: the focus on the empty state", async () => {
    const { rows, empty, arm } = setup();
    arm();
    rows.value = [];
    await settle();
    expect(document.activeElement).toBe(empty);
  });

  it("not armed, an empty list leaves the focus where it is", async () => {
    const { rows, empty } = setup();
    rows.value = [];
    await settle();
    expect(document.activeElement).not.toBe(empty);
  });

  it("rows left: disarmed, a later empty list takes no focus", async () => {
    const { rows, empty, arm } = setup();
    arm();
    rows.value = [{ id: "b" }];
    await settle();
    rows.value = [];
    await settle();
    expect(document.activeElement).not.toBe(empty);
  });

  it("the focus already somewhere: kept there", async () => {
    const { rows, empty, arm } = setup();
    const button = document.createElement("button");
    document.body.append(button);
    button.focus();
    arm();
    rows.value = [];
    await settle();
    expect(document.activeElement).toBe(button);
    expect(document.activeElement).not.toBe(empty);
  });
});
