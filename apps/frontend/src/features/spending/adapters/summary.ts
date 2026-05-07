import { invoke, logger } from "#platform";

import type { SpendingSummary } from "../types";
import type { EventWithTypeName } from "../types/event";

/**
 * Returns one SpendingSummary per period (TOTAL, YTD, LAST_YEAR, TWO_YEARS_AGO).
 * Frontend picks the relevant one with `.find(s => s.period === selectedPeriod)`.
 */
export const getSpendingSummary = async (
  includeEventIds?: string[],
  includeAllEvents?: boolean,
): Promise<SpendingSummary[]> => {
  try {
    return await invoke<SpendingSummary[]>("get_spending_summary", {
      includeEventIds,
      includeAllEvents,
    });
  } catch (error) {
    logger.error("Error fetching spending summary.");
    throw error;
  }
};

export const getEventsWithNames = async (): Promise<EventWithTypeName[]> => {
  try {
    return await invoke<EventWithTypeName[]>("get_events_with_names");
  } catch (error) {
    logger.error("Error fetching events with names.");
    throw error;
  }
};
