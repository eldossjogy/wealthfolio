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
    // Walk the date range using ISO-string arithmetic to avoid Date allocs in
    // the inner loop. startDate/endDate are stored as ISO strings.
    const startKey = ev.startDate.slice(0, 10);
    const endKey = ev.endDate.slice(0, 10);
    const cursor = new Date(`${startKey}T12:00:00`);
    const endMs = new Date(`${endKey}T12:00:00`).getTime();
    while (cursor.getTime() <= endMs) {
      exclude.add(cursor.toISOString().slice(0, 10));
      cursor.setDate(cursor.getDate() + 1);
    }
  }
  let total = 0;
  const seen = new Set<string>();
  for (const a of activities) {
    const spendingAmount = getActivitySpendingAmount(a, accountTypeById?.get(a.accountId));
    if (spendingAmount === 0) continue;
    const dayKey = a.activityDate.slice(0, 10);
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
