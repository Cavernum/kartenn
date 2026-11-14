import { onBeforeUnmount, onMounted, ref, type Ref } from "vue";

/**
 * Whether the component that asks it has been drawn once: false at its mount, true in the
 * task after the first animation frame that follows it, once that frame is painted. The
 * panels beside a record read what its route does not declare (`meta.queries`: the
 * occupancy of a prefix and its next free value, the gaps of an object) once it is true:
 * their requests leave after the record is drawn rather than during its mount, which they
 * would lengthen (task 021-22o). What the cache holds shows at once all the same: a query
 * not yet enabled keeps its data.
 */
export function usePainted(): Readonly<Ref<boolean>> {
  const painted = ref(false);
  let frame: number | undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const after = () => {
    frame = undefined;
    timer = setTimeout(() => (painted.value = true));
  };
  onMounted(() => {
    if (typeof requestAnimationFrame === "function") frame = requestAnimationFrame(after);
    else after();
  });
  onBeforeUnmount(() => {
    if (frame !== undefined) cancelAnimationFrame(frame);
    if (timer !== undefined) clearTimeout(timer);
  });
  return painted;
}
