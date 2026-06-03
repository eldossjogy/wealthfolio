import type { DateRange, TimePeriod } from "@/lib/types";

const ALL_TIME_YEAR = 1970;
const ALL_TIME_MONTH_INDEX = 0;
const ALL_TIME_DAY = 1;

function isAllTimeStartDate(date: Date): boolean {
  // UI controls build the 1970 sentinel with both local and ISO constructors.
  return (
    (date.getFullYear() === ALL_TIME_YEAR &&
      date.getMonth() === ALL_TIME_MONTH_INDEX &&
      date.getDate() === ALL_TIME_DAY) ||
    (date.getUTCFullYear() === ALL_TIME_YEAR &&
      date.getUTCMonth() === ALL_TIME_MONTH_INDEX &&
      date.getUTCDate() === ALL_TIME_DAY)
  );
}

export function isAllTimeDateRange(range: DateRange | undefined): boolean {
  return !!range?.from && isAllTimeStartDate(range.from);
}

export function getPerformanceDateRangeForRequest(
  range: DateRange | undefined,
  intervalCode?: TimePeriod,
): DateRange | undefined {
  return intervalCode === "ALL" || isAllTimeDateRange(range) ? undefined : range;
}
