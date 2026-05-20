/**
 * Fetch a per-month spending report for each month in the supplied range,
 * via parallel React Query queries (one per month).
 *
 * Each query is keyed independently so months stay cached as the user shifts
 * the active period — switching from 6M → 1Y reuses the 6 months we already
 * have and only fetches the 6 missing ones.
 *
 * This is the data backbone for:
 *   • Cashflow stacked-area chart (Trends tab)
 *   • Per-category sparkline grid (Categories tab)
 */

import { useMemo } from "react";
import { useQueries } from "@tanstack/react-query";

import { QueryKeys } from "@/lib/query-keys";

import { getSpendingReport } from "../adapters/reports";
import { monthsInRange, type ReportsRange } from "../lib/reports-period";
import type { MonthlyReport } from "../types/report";

export interface MonthBucket {
  /** ISO YYYY-MM-01, anchors the month for charting. */
  iso: string;
  /** Short month label, e.g. "May". */
  label: string;
  /** Resolved monthly report (undefined while in flight). */
  report: MonthlyReport | undefined;
  isLoading: boolean;
}

export interface MonthlyHistory {
  months: MonthBucket[];
  isLoading: boolean;
}

/** YYYY-MM from local date components — NOT toISOString().slice(0,7).
 *  toISOString shifts to UTC, so for users east of UTC a local Jan-1 lands on
 *  Dec-31 in ISO, polluting the query cache key and the bucket anchor. */
function localMonthKey(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}
/** YYYY-MM-DD from local date components. */
function localDayKey(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function useMonthlyHistory(range: ReportsRange, enabled = true): MonthlyHistory {
  const months = useMemo(() => monthsInRange(range), [range]);

  const queries = useQueries({
    queries: months.map((m) => ({
      queryKey: [QueryKeys.SPENDING_REPORT, "monthly", localMonthKey(m.start)],
      queryFn: () =>
        getSpendingReport({
          startDate: m.start.toISOString(),
          endDate: m.end.toISOString(),
        }),
      enabled,
      staleTime: 5 * 60_000, // 5 minutes
    })),
  });

  const buckets: MonthBucket[] = months.map((m, i) => ({
    iso: localDayKey(m.start),
    label: m.label,
    report: queries[i]?.data,
    isLoading: queries[i]?.isLoading ?? false,
  }));

  return {
    months: buckets,
    isLoading: queries.some((q) => q.isLoading),
  };
}
