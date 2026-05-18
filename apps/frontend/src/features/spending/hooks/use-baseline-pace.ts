import { useMemo } from "react";

import type { Activity } from "@/lib/types";

import { getActivitySpendingAmount } from "../lib/constants";
import type { EventSpendingSummary } from "../types/event";

/**
 * Average daily outflow across `activities`, ignoring days falling inside any
 * window in `excludeEvents`. Returns 0 when no eligible days exist.
 *
 * Used as the "normal pace" benchmark for events analysis: the user's typical
 * daily spend minus the days they were doing something atypical (a trip, a
 * wedding, etc.).
 */
export function computeBaselinePace(
  activities: Activity[],
  excludeEvents: EventSpendingSummary[],
  accountTypeById?: Map<string, string>,
): number {
  const exclude = new Set<string>();
  for (const ev of excludeEvents) {
    const start = new Date(ev.startDate);
    const end = new Date(ev.endDate);
    for (let d = new Date(start); d <= end; d.setDate(d.getDate() + 1)) {
      exclude.add(d.toISOString().slice(0, 10));
    }
  }
  let total = 0;
  const seen = new Set<string>();
  for (const a of activities) {
    const spendingAmount = getActivitySpendingAmount(a, accountTypeById?.get(a.accountId));
    if (spendingAmount === 0) continue;
    const dayKey = new Date(a.activityDate).toISOString().slice(0, 10);
    if (exclude.has(dayKey)) continue;
    total += spendingAmount;
    seen.add(dayKey);
  }
  return seen.size === 0 ? 0 : Math.max(0, total) / seen.size;
}

export function useBaselinePace(
  activities: Activity[],
  excludeEvents: EventSpendingSummary[],
  accountTypeById?: Map<string, string>,
): number {
  return useMemo(
    () => computeBaselinePace(activities, excludeEvents, accountTypeById),
    [activities, excludeEvents, accountTypeById],
  );
}
