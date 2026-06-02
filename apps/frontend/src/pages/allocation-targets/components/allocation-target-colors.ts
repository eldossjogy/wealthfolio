// Allocation palette — forest / sage / sand / clay / plum / stone (matches the design + chart tokens).
const CALM_PALETTE = [
  "#355c4c", // forest
  "#7e9f8c", // sage
  "#cbba8c", // sand
  "#c08a5f", // clay
  "#9a7e92", // plum
  "#b1aa9a", // stone
  "#5a7d6b", // forest tint
  "#a8b89e", // sage tint
  "#8a6b49", // muted brown
];

const NAMED_COLORS: Record<string, string> = {
  equity: "#355c4c", // forest
  fixed: "#7e9f8c", // sage
  cash: "#cbba8c", // sand
  commodities: "#c08a5f", // clay
  real: "#c08a5f", // clay (real assets / real estate)
  property: "#c08a5f", // clay
  crypto: "#9a7e92", // plum
  digital: "#9a7e92", // plum
  alternatives: "#b1aa9a", // stone
};

export interface AllocationTargetColorRow {
  categoryId: string;
  categoryName: string;
}

export type AllocationTargetColorMap = ReadonlyMap<string, string>;

function categoryKey(id: string, name: string): string {
  return `${id} ${name}`.toLowerCase().replace(/[\s-]+/g, "_");
}

export function allocationTargetColor(id: string, name: string, index = 0): string {
  const key = categoryKey(id, name);
  const named = Object.entries(NAMED_COLORS).find(([needle]) => key.includes(needle));
  if (named) return named[1];
  return CALM_PALETTE[index % CALM_PALETTE.length];
}

export function buildAllocationTargetColorMap(
  rows: readonly AllocationTargetColorRow[],
): AllocationTargetColorMap {
  const colors = new Map<string, string>();

  rows.forEach((row, index) => {
    if (!colors.has(row.categoryId)) {
      colors.set(row.categoryId, allocationTargetColor(row.categoryId, row.categoryName, index));
    }
  });

  return colors;
}

export function allocationTargetColorForRow(
  row: AllocationTargetColorRow,
  colors: AllocationTargetColorMap | undefined,
  fallbackIndex = 0,
): string {
  return (
    colors?.get(row.categoryId) ??
    allocationTargetColor(row.categoryId, row.categoryName, fallbackIndex)
  );
}
