import { resolveDisplayTimezone } from "@/lib/utils";

const WEEKDAY_INDEX: Record<string, number> = {
  Mon: 0,
  Tue: 1,
  Wed: 2,
  Thu: 3,
  Fri: 4,
  Sat: 5,
  Sun: 6,
};

export interface ZonedDayHour {
  dayKey: string;
  weekday: number;
  hour: number;
}

export function createZonedDayHourFormatter(
  timezone?: string | null,
): (date: Date) => ZonedDayHour | null {
  const formatter = new Intl.DateTimeFormat("en-US", {
    timeZone: resolveDisplayTimezone(timezone),
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    weekday: "short",
    hour: "2-digit",
    hourCycle: "h23",
  });

  return (date: Date) => {
    if (Number.isNaN(date.getTime())) return null;

    const parts = formatter.formatToParts(date);
    const value = (type: Intl.DateTimeFormatPartTypes) =>
      parts.find((part) => part.type === type)?.value;
    const year = value("year");
    const month = value("month");
    const day = value("day");
    const weekdayName = value("weekday");
    const hour = Number(value("hour"));
    const weekday = weekdayName ? WEEKDAY_INDEX[weekdayName] : undefined;

    if (!year || !month || !day || weekday === undefined || !Number.isFinite(hour)) {
      return null;
    }

    return {
      dayKey: `${year}-${month}-${day}`,
      weekday,
      hour,
    };
  };
}
