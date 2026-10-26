// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * The geometry of the error of a cell (cinquième relecture du 2026-10-26): its message in a
 * floating layer under the cell (above it near the bottom of the viewport), at least as
 * wide as the cell, aligned on its inline start (on its end near the edge). One outline
 * goes round the cell (its top and both sides, and its bottom outside the junction) and
 * the message (all round outside the junction), joined by concave fillets where the
 * message is wider than the cell: a tab that grows out of the cell. The fill covers the
 * message and the fillets, never the cell, whose content stays in view.
 */

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface CellErrorInput {
  /** The cell, in viewport pixels. */
  cell: Rect;
  /** The natural size of the message (its width is raised to the cell's). */
  message: { width: number; height: number };
  viewport: { width: number; height: number };
  /** The radius of the outer corners and of the fillets. */
  radius: number;
  /** The width of the outline. */
  stroke: number;
  /** The least distance kept to the edges of the viewport. */
  margin: number;
  rtl: boolean;
}

export interface CellErrorPlacement {
  /** The layer (cell and message), physical, in viewport pixels. */
  x: number;
  y: number;
  width: number;
  height: number;
  /** Where the message stands: under the cell, or above it. */
  side: "block-end" | "block-start";
  /** The message aligned on the inline start of the cell, or on its end. */
  align: "start" | "end";
  /** The offset of the message from the top of the layer. */
  messageTop: number;
  /** The path of the fill (message and fillets), in the coordinates of the layer. */
  fill: string;
  /** The path of the outline (cell and message). */
  outline: string;
}

type Segment =
  | { kind: "M" | "L"; x: number; y: number }
  | { kind: "A"; r: number; sweep: 0 | 1; x: number; y: number };

const round = (n: number) => Math.round(n * 100) / 100;

/** The path of segments, its vertical axis turned over when the message stands above. */
function serialise(segments: Segment[], flip: number | null): string {
  const y = (v: number) => round(flip === null ? v : flip - v);
  return (
    segments
      .map((s) =>
        s.kind === "A"
          ? `A${round(s.r)} ${round(s.r)} 0 0 ${flip === null ? s.sweep : 1 - s.sweep} ${round(s.x)} ${y(s.y)}`
          : `${s.kind}${round(s.x)} ${y(s.y)}`,
      )
      .join(" ") + " Z"
  );
}

export function placeCellError(input: CellErrorInput): CellErrorPlacement {
  const { cell, viewport, margin, rtl } = input;
  const width = Math.max(input.message.width, cell.width);
  const messageHeight = input.message.height;

  // Across: the inline start of the message on the cell's, else its end on the cell's end,
  // else as far in as the viewport allows, always covering the cell's width.
  const startAligned = rtl ? cell.x + cell.width - width : cell.x;
  const endAligned = rtl ? cell.x : cell.x + cell.width - width;
  const fits = (x: number) => x >= margin && x + width <= viewport.width - margin;
  let x: number;
  let align: "start" | "end" = "start";
  if (fits(startAligned)) x = startAligned;
  else if (fits(endAligned)) {
    x = endAligned;
    align = "end";
  } else {
    x = Math.max(margin, Math.min(startAligned, viewport.width - margin - width));
    x = Math.max(cell.x + cell.width - width, Math.min(x, cell.x));
  }

  // Down: under the cell, unless it would leave the viewport and there is room above.
  const below =
    cell.y + cell.height + messageHeight <= viewport.height - margin ||
    cell.y - messageHeight < margin;
  const height = cell.height + messageHeight;

  // The shape, drawn with the message under the cell, then turned over if it stands above.
  // The outline is inset by half its width: it stays inside the cell and the layer.
  const h = input.stroke / 2;
  const x0 = cell.x - x + h;
  const x1 = cell.x + cell.width - x - h;
  const m0 = h;
  const m1 = width - h;
  const top = h;
  const join = cell.height;
  const bottom = height - h;
  const fillet = (overhang: number) =>
    overhang < 0.5 ? 0 : Math.min(input.radius, overhang / 2, cell.height / 2);
  const fl = fillet(x0 - m0);
  const fr = fillet(m1 - x1);
  const rb = Math.min(input.radius, messageHeight / 2, width / 2);

  // From the junction at the end of the cell round the message to its start (clockwise).
  const around: Segment[] = [];
  if (fr > 0)
    around.push(
      { kind: "A", r: fr, sweep: 0, x: x1 + fr, y: join },
      { kind: "L", x: m1 - fr, y: join },
      { kind: "A", r: fr, sweep: 1, x: m1, y: join + fr },
    );
  around.push(
    { kind: "L", x: m1, y: bottom - rb },
    { kind: "A", r: rb, sweep: 1, x: m1 - rb, y: bottom },
    { kind: "L", x: m0 + rb, y: bottom },
    { kind: "A", r: rb, sweep: 1, x: m0, y: bottom - rb },
  );
  if (fl > 0)
    around.push(
      { kind: "L", x: m0, y: join + fl },
      { kind: "A", r: fl, sweep: 1, x: m0 + fl, y: join },
      { kind: "L", x: x0 - fl, y: join },
      { kind: "A", r: fl, sweep: 0, x: x0, y: join - fl },
    );

  const outline: Segment[] = [
    { kind: "M", x: x0, y: join - fl },
    { kind: "L", x: x0, y: top },
    { kind: "L", x: x1, y: top },
    { kind: "L", x: x1, y: join - fr },
    ...around,
  ];
  // The fill keeps out of the cell: along the junction, up the cell's edge to a fillet.
  const fill: Segment[] = [
    { kind: "M", x: x0, y: join },
    { kind: "L", x: x1, y: join },
    ...(fr > 0 ? [{ kind: "L" as const, x: x1, y: join - fr }] : []),
    ...around,
    ...(fl > 0 ? [{ kind: "L" as const, x: x0, y: join }] : []),
  ];

  const flip = below ? null : height;
  return {
    x,
    y: below ? cell.y : cell.y - messageHeight,
    width,
    height,
    side: below ? "block-end" : "block-start",
    align,
    messageTop: below ? cell.height : 0,
    fill: serialise(fill, flip),
    outline: serialise(outline, flip),
  };
}
