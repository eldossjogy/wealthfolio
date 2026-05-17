/**
 * Period model for the Reports page.
 *
 * Distinct from the dashboard's `DashboardPeriod` because Reports operate on
 * larger time horizons by default (≥3 months), and they pair the active
 * window with a comparison window (prior period or YoY).
 */

import type { DateRange } from "@/lib/types";

export type ReportsPeriod = "1M" | "3M" | "6M" | "YTD" | "1Y";

export const REPORTS_PERIODS: ReportsPeriod[] = ["1M", "3M", "6M", "YTD", "1Y"];

export const DEFAULT_REPORTS_PERIOD: ReportsPeriod = "6M";

export type ComparisonMode = "prior" | "yoy" | "none";

export const DEFAULT_COMPARISON: ComparisonMode = "prior";

export interface ReportsRange {
  start: Date;
  end: Date;
  /** Number of calendar days the active window covers (inclusive). */
  days: number;
  /** Number of full months the active window covers (used for sparklines). */
  months: number;
}

/** Convert a period selection into the active date range. */
export function periodToReportsRange(period: ReportsPeriod): ReportsRange {
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 23, 59, 59, 999);

  // For 1M we span the full calendar month so "X days left in May" reads
  // correctly and forecasts can project past today. Other periods stay
  // "through today" since they cover multiple months and the "current month"
  // is naturally the trailing edge.
  const { start, end } = (() => {
    switch (period) {
      case "1M": {
        const monthStart = new Date(now.getFullYear(), now.getMonth(), 1, 0, 0, 0, 0);
        const monthEnd = new Date(now.getFullYear(), now.getMonth() + 1, 0, 23, 59, 59, 999);
        return { start: monthStart, end: monthEnd };
      }
      case "3M":
        return {
          start: new Date(now.getFullYear(), now.getMonth() - 2, 1, 0, 0, 0, 0),
          end: today,
        };
      case "6M":
        return {
          start: new Date(now.getFullYear(), now.getMonth() - 5, 1, 0, 0, 0, 0),
          end: today,
        };
      case "YTD":
        return { start: new Date(now.getFullYear(), 0, 1, 0, 0, 0, 0), end: today };
      case "1Y":
        return {
          start: new Date(now.getFullYear(), now.getMonth() - 11, 1, 0, 0, 0, 0),
          end: today,
        };
    }
  })();

  return {
    start,
    end,
    days: daysBetweenInclusive(start, end),
    months: monthsBetweenInclusive(start, end),
  };
}

/** Comparison range — equally-sized prior window or same period last year. */
export function comparisonRange(range: ReportsRange, mode: ComparisonMode): ReportsRange | null {
  if (mode === "none") return null;
  if (mode === "yoy") {
    const start = new Date(range.start);
    start.setFullYear(start.getFullYear() - 1);
    const end = new Date(range.end);
    end.setFullYear(end.getFullYear() - 1);
    return { start, end, days: range.days, months: range.months };
  }
  // mode === "prior" — equally sized window ending the day before `range.start`
  const span = range.end.getTime() - range.start.getTime();
  const priorEnd = new Date(range.start.getTime() - 1);
  const priorStart = new Date(priorEnd.getTime() - span);
  return {
    start: priorStart,
    end: priorEnd,
    days: range.days,
    months: range.months,
  };
}

/** Build the ISO request payload consumed by `useSpendingReport`. */
export function rangeToReportRequest(range: ReportsRange): {
  startDate: string;
  endDate: string;
} {
  return {
    startDate: range.start.toISOString(),
    endDate: range.end.toISOString(),
  };
}

/** Used by `react-router-dom` Link-typed APIs that expect a DateRange. */
export function toDateRange(range: ReportsRange): DateRange {
  return { from: range.start, to: range.end };
}

export function periodLabel(period: ReportsPeriod): string {
  switch (period) {
    case "1M":
      return "This month";
    case "3M":
      return "Past 3 months";
    case "6M":
      return "Past 6 months";
    case "YTD":
      return "Year to date";
    case "1Y":
      return "Past year";
  }
}

export function comparisonLabel(mode: ComparisonMode): string {
  switch (mode) {
    case "prior":
      return "Prior period";
    case "yoy":
      return "Same period last year";
    case "none":
      return "No comparison";
  }
}

// ─── helpers ────────────────────────────────────────────────────────────

function daysBetweenInclusive(a: Date, b: Date): number {
  return Math.round((b.getTime() - a.getTime()) / (1000 * 60 * 60 * 24)) + 1;
}

function monthsBetweenInclusive(a: Date, b: Date): number {
  return (b.getFullYear() - a.getFullYear()) * 12 + (b.getMonth() - a.getMonth()) + 1;
}

/** Enumerate first-of-month dates between two ranges (inclusive). Used for sparkline buckets. */
export function monthsInRange(range: ReportsRange): { start: Date; end: Date; label: string }[] {
  const out: { start: Date; end: Date; label: string }[] = [];
  const cursor = new Date(range.start.getFullYear(), range.start.getMonth(), 1);
  const last = new Date(range.end.getFullYear(), range.end.getMonth(), 1);
  const fmt = new Intl.DateTimeFormat(undefined, { month: "short" });
  while (cursor <= last) {
    const start = new Date(cursor);
    const end = new Date(cursor.getFullYear(), cursor.getMonth() + 1, 0, 23, 59, 59, 999);
    out.push({ start, end, label: fmt.format(start) });
    cursor.setMonth(cursor.getMonth() + 1);
  }
  return out;
}
