import type { DriftReport, TaxonomyCategory } from "@/lib/types";

function resolveCategory<
  T extends {
    categoryId: string;
    categoryName: string;
    color?: string;
    categoryColor?: string | null;
  },
>(row: T, category: TaxonomyCategory | undefined): { row: T; changed: boolean } {
  if (!category) return { row, changed: false };

  const colorKey = "categoryColor" in row ? "categoryColor" : "color";
  const currentColor = row[colorKey];
  if (row.categoryName === category.name && currentColor === category.color) {
    return { row, changed: false };
  }

  return {
    row: {
      ...row,
      categoryName: category.name,
      [colorKey]: category.color,
    } as T,
    changed: true,
  };
}

export function resolveDriftReportCategories(
  report: DriftReport,
  categories: TaxonomyCategory[] | undefined,
): DriftReport {
  if (!categories?.length) return report;

  const byId = new Map(categories.map((category) => [category.id, category]));
  let changed = false;

  const rows = report.rows.map((row) => {
    const resolved = resolveCategory(row, byId.get(row.categoryId));
    changed ||= resolved.changed;
    return resolved.row;
  });

  const holdings = report.holdings
    ? {
        ...report.holdings,
        rows: report.holdings.rows.map((row) => {
          const resolved = resolveCategory(row, byId.get(row.categoryId));
          changed ||= resolved.changed;
          return resolved.row;
        }),
      }
    : report.holdings;

  return changed ? { ...report, rows, holdings } : report;
}
