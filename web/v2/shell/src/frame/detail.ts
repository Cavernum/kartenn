import { shallowRef, type Ref } from "vue";

// The object a route shows in the Détail tab (`meta.detail`, task 15a): another object
// chosen opens the right panel on it by the route; the same object chosen again asks for
// it here, the route unchanged, and the frame opens the panel on its Détail tab.

const asked = shallowRef(0);

/** Opens the right panel on the Détail tab of the object the route shows. */
export function revealDetail(): void {
  asked.value += 1;
}

/** Each request of `revealDetail`, for the frame. */
export function detailRequests(): Readonly<Ref<number>> {
  return asked;
}
