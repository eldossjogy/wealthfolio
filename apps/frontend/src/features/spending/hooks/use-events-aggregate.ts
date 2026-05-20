import { useMemo } from "react";

import type { Activity } from "@/lib/types";

import { inclusiveDays } from "../lib/date-utils";
import type { EventSpendingSummary } from "../types/event";
import { computeBaselinePace } from "./use-baseline-pace";

export interface EventsAggregate {
  totalSpent: number;
  totalEventDays: number;
  normalPace: number;
  lift: number;
  topEventName: string | null;
}

export function computeEventsAggregate(
  events: EventSpendingSummary[],
  heatmapActivities: Activity[],
  accountTypeById?: Map<string, string>,
): EventsAggregate {
  let totalSpent = 0;
  let totalEventDays = 0;
  let topEvent: EventSpendingSummary | null = null;
  for (const ev of events) {
    totalSpent += ev.totalSpending;
    const days = Math.max(1, inclusiveDays(new Date(ev.startDate), new Date(ev.endDate)));
    totalEventDays += days;
    if (!topEvent || ev.totalSpending > topEvent.totalSpending) topEvent = ev;
  }

  // Heatmap is a fixed 12-week window (see spending-insights-page.tsx
  // HEATMAP_WEEKS) — use 84 calendar days as the divisor so the baseline
  // reflects pace across the whole window, not just days that saw spending.
  const HEATMAP_PERIOD_DAYS = 12 * 7;
  const normalPace = computeBaselinePace(
    heatmapActivities,
    events,
    HEATMAP_PERIOD_DAYS,
    accountTypeById,
  );
  const expected = normalPace * totalEventDays;
  const lift = totalSpent - expected;

  return {
    totalSpent,
    totalEventDays,
    normalPace,
    lift,
    topEventName: topEvent?.eventName ?? null,
  };
}

export function useEventsAggregate(
  events: EventSpendingSummary[],
  heatmapActivities: Activity[],
  accountTypeById?: Map<string, string>,
): EventsAggregate {
  return useMemo(
    () => computeEventsAggregate(events, heatmapActivities, accountTypeById),
    [events, heatmapActivities, accountTypeById],
  );
}
