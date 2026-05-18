/** Spending period the overview UI offers. */
export type SpendingPeriod = "TOTAL" | "YTD" | "LAST_YEAR";

export interface DateRange {
  startDate?: string; // YYYY-MM-DD
  endDate?: string; // YYYY-MM-DD
}

const ymd = (d: Date): string =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(
    2,
    "0",
  )}`;

/** Convert a SpendingPeriod into an inclusive date range (in the user's local TZ). */
export function periodToDateRange(period: SpendingPeriod): DateRange {
  const now = new Date();
  const year = now.getFullYear();
  switch (period) {
    case "YTD":
      return { startDate: `${year}-01-01`, endDate: ymd(now) };
    case "LAST_YEAR":
      return { startDate: `${year - 1}-01-01`, endDate: `${year - 1}-12-31` };
    case "TOTAL":
    default:
      return {};
  }
}

/** Build a spending-transactions URL with category/subcategory + date filters as query params. */
export function buildCashflowUrl(opts: {
  categoryId?: string | null;
  subcategoryId?: string | null;
  startDate?: string;
  endDate?: string;
}): string {
  const params = new URLSearchParams();
  params.set("tab", "spending");
  if (opts.categoryId) params.set("category", opts.categoryId);
  if (opts.subcategoryId) params.set("subcategory", opts.subcategoryId);
  if (opts.startDate) params.set("from", opts.startDate);
  if (opts.endDate) params.set("to", opts.endDate);
  return `/activities?${params.toString()}`;
}
