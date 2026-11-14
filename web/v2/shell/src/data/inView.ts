import { onBeforeUnmount, onMounted, readonly, ref, type Ref } from "vue";

// Whether an element of a component is in view (task 021-11p10-a, from the VLAN column of
// task 26): a grid mounts every row up to 200, and a cell that reads for itself reads only
// once it is seen (`enabled` of its read), its read under way given up when it leaves the
// view (`gate.ts`, `giveUpUnseen`). Without `IntersectionObserver`, always in view.

/**
 * Whether `target` intersects the view, `false` until the first observation; `left` is
 * called each time it leaves it.
 */
export function useInView(
  target: Readonly<Ref<Element | null | undefined>>,
  left?: () => void,
): Readonly<Ref<boolean>> {
  const inView = ref(false);
  let observer: IntersectionObserver | undefined;
  onMounted(() => {
    if (typeof IntersectionObserver === "undefined" || !target.value) {
      inView.value = true;
      return;
    }
    observer = new IntersectionObserver((entries) => {
      // The last entry is the latest change of the one element observed.
      const seen = entries.at(-1)?.isIntersecting ?? inView.value;
      const leaving = inView.value && !seen;
      inView.value = seen;
      if (leaving) left?.();
    });
    observer.observe(target.value);
  });
  onBeforeUnmount(() => observer?.disconnect());
  return readonly(inView);
}
