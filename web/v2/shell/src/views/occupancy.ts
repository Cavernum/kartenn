import type { components } from "@kartenn/sdk";

// The share of a prefix in use (REQ-021-9): its used values out of its total, whole numbers
// of any size (decimal strings, IPv6 included), as the record (`OccupancyPanel.vue`) and the
// column of the topography (021/A11, `TreeNode.occupancy`) say it.

type Occupancy = Pick<components["schemas"]["Occupancy"], "used" | "total">;

/** A decimal string as a whole number; not one, zero. */
export function whole(value: string): bigint {
  try {
    return BigInt(value);
  } catch {
    return 0n;
  }
}

/**
 * The share used, in whole percent rounded down (« 72 % »); under one percent but not none,
 * « < 1 % »; nothing for a total of zero or no occupancy.
 */
export function shareOf(occupancy: Occupancy | undefined): string | undefined {
  if (!occupancy) return undefined;
  const total = whole(occupancy.total);
  const used = whole(occupancy.used);
  if (total <= 0n) return undefined;
  const percent = (used * 100n) / total;
  return percent === 0n && used > 0n ? "< 1 %" : `${percent} %`;
}
