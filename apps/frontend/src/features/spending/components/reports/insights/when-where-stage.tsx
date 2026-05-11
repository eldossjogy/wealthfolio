import { useMemo, type FC } from "react";

import {
  Icons,
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
  formatCompactAmount,
} from "@wealthfolio/ui";
import type { Activity, TaxonomyCategory } from "@/lib/types";
import { cn, formatAmount } from "@/lib/utils";

import { OUTFLOW_TYPES, type CashActivityType } from "../../../lib/constants";
import { FOREST_THEME } from "../../../lib/theme";
import type { EventSpendingSummary } from "../../../types/event";
import { formatMonthDay } from "./format";

const CARD_CLASS = "border-border/60 bg-card/40 rounded-2xl border p-5 backdrop-blur-xl";
const LABEL_CLASS =
  "text-muted-foreground/70 text-[10px] font-semibold uppercase tracking-[0.12em]";

export interface WhenWhereStageProps {
  /** Last 12 weeks of cash activities (for the heatmap). */
  heatmapActivities: Activity[];
  events: EventSpendingSummary[];
  taxonomyCategories: TaxonomyCategory[];
  currency: string;
  /** Period start/end for the events strip. */
  rangeStart: Date;
  rangeEnd: Date;
}

export function WhenWhereStage({
  heatmapActivities,
  events,
  taxonomyCategories,
  currency,
  rangeStart,
  rangeEnd,
}: WhenWhereStageProps) {
  return (
    <div className="flex flex-col gap-6">
      <WhenYouSpendCard activities={heatmapActivities} currency={currency} />
      {events.length > 0 ? (
        <>
          <EventsHeadlineCard
            events={events}
            currency={currency}
            rangeStart={rangeStart}
            rangeEnd={rangeEnd}
            heatmapActivities={heatmapActivities}
          />
          <div className="grid gap-4 lg:grid-cols-3">
            {events.slice(0, 3).map((event) => (
              <EventDetailCard
                key={event.eventId}
                event={event}
                taxonomyCategories={taxonomyCategories}
                currency={currency}
                heatmapActivities={heatmapActivities}
              />
            ))}
          </div>
        </>
      ) : (
        <EmptyEventsCard />
      )}
    </div>
  );
}

// ═════════════════════════════════════════════════════════════════════════
// "When you spend" — weekday × hour heatmap with per-row median
// ═════════════════════════════════════════════════════════════════════════

interface WhenYouSpendCardProps {
  activities: Activity[];
  currency: string;
}

const WhenYouSpendCard: FC<WhenYouSpendCardProps> = ({ activities, currency }) => {
  const grid = useMemo(() => buildWeekdayHourGrid(activities), [activities]);
  const accent = FOREST_THEME.deep;

  if (activities.length === 0) {
    return (
      <div className={CARD_CLASS}>
        <header className="mb-3">
          <h3 className="text-foreground text-base font-semibold tracking-tight">When you spend</h3>
          <p className="text-muted-foreground text-xs">
            Last 12 weeks · spending intensity by weekday and hour.
          </p>
        </header>
        <div className="text-muted-foreground py-8 text-center text-sm">
          No cash activity in the last 12 weeks.
        </div>
      </div>
    );
  }

  // Hour tick labels — every 3 hours.
  const hourLabels = ["12a", "3a", "6a", "9a", "12p", "3p", "6p", "9p"];

  return (
    <div className={CARD_CLASS}>
      <header className="mb-4 flex flex-wrap items-baseline justify-between gap-2">
        <div>
          <h3 className="text-foreground text-base font-semibold tracking-tight">When you spend</h3>
          <p className="text-muted-foreground text-xs">
            Last 12 weeks · spending intensity by weekday and hour.
          </p>
        </div>
        <span className={LABEL_CLASS}>MEDIAN PER WEEKDAY</span>
      </header>

      <div className="grid grid-cols-[auto_1fr_auto] items-center gap-x-3 gap-y-1">
        {/* Hour-axis label row */}
        <div />
        <div className="text-muted-foreground/70 grid grid-cols-8 text-[10px]">
          {hourLabels.map((h, i) => (
            <span key={i} className={cn(i === 0 ? "text-left" : "text-left")}>
              {h}
            </span>
          ))}
        </div>
        <div />

        {/* 7 weekday rows */}
        {DAY_NAMES.map((day, di) => {
          const row = grid.cells[di];
          const median = grid.medians[di];
          return (
            <Row
              key={di}
              day={day}
              cells={row}
              max={grid.max}
              accent={accent}
              median={median}
              currency={currency}
            />
          );
        })}
      </div>

      <div className="border-border/40 mt-4 flex items-center justify-between border-t pt-3 text-[11px]">
        <span className="text-muted-foreground/70">
          Each cell is one weekday-hour over 12 weeks. Darker = more spend.
        </span>
        <Legend accent={accent} />
      </div>
    </div>
  );
};

const DAY_NAMES = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

function Row({
  day,
  cells,
  max,
  accent,
  median,
  currency,
}: {
  day: string;
  cells: number[];
  max: number;
  accent: string;
  median: number;
  currency: string;
}) {
  return (
    <>
      <div className="text-muted-foreground/80 pr-1 text-right text-[11px]">{day}</div>
      <div
        className="grid-cols-24 grid gap-[3px]"
        style={{ gridTemplateColumns: "repeat(24, minmax(0,1fr))" }}
      >
        {cells.map((amount, i) => {
          const t = max > 0 ? amount / max : 0;
          const opacity = amount === 0 ? 0.07 : 0.18 + t * 0.75;
          return (
            <div
              key={i}
              className="aspect-square rounded-[3px]"
              style={{ backgroundColor: accent, opacity }}
              title={`${day} ${formatHour(i)} · ${amount > 0 ? formatAmount(amount, currency) : "no spend"}`}
            />
          );
        })}
      </div>
      <div className="text-foreground/90 inline-flex items-center gap-2 pl-1 text-xs tabular-nums">
        <span className="bg-foreground/30 inline-block h-px w-6" />
        <span className="font-medium">{formatCompactAmount(median, currency)}</span>
      </div>
    </>
  );
}

function Legend({ accent }: { accent: string }) {
  return (
    <span className="text-muted-foreground/70 inline-flex items-center gap-1.5">
      <span>less</span>
      {[0.15, 0.35, 0.6, 0.9].map((o, i) => (
        <span
          key={i}
          className="h-2.5 w-2.5 rounded-[2px]"
          style={{ backgroundColor: accent, opacity: o }}
        />
      ))}
      <span>more</span>
    </span>
  );
}

function formatHour(h: number): string {
  if (h === 0) return "12am";
  if (h === 12) return "12pm";
  return h < 12 ? `${h}am` : `${h - 12}pm`;
}

interface WeekdayHourGrid {
  /** [weekdayIndex (Mon=0..Sun=6)][hour 0..23] = total spend in that bucket. */
  cells: number[][];
  max: number;
  /** Median daily total per weekday — i.e. across the 12 weeks, median of that weekday's daily total. */
  medians: number[];
}

function buildWeekdayHourGrid(activities: Activity[]): WeekdayHourGrid {
  const cells: number[][] = Array.from({ length: 7 }, () => new Array(24).fill(0));
  // Per (weekday, dayKey) → daily total. Used to compute the median per weekday.
  const dayTotals = new Map<string, number>();

  for (const a of activities) {
    if (!OUTFLOW_TYPES.includes(a.activityType as CashActivityType)) continue;
    const date = new Date(a.activityDate);
    if (isNaN(date.getTime())) continue;
    const weekday = (date.getDay() + 6) % 7; // Mon=0..Sun=6
    const hour = date.getHours();
    const amt = parseFloat(a.amount ?? "0") || 0;
    cells[weekday][hour] += amt;
    const key = `${weekday}|${date.toISOString().slice(0, 10)}`;
    dayTotals.set(key, (dayTotals.get(key) ?? 0) + amt);
  }

  const max = Math.max(0, ...cells.flat());

  const medians: number[] = [];
  for (let d = 0; d < 7; d++) {
    const values: number[] = [];
    for (const [key, total] of dayTotals) {
      const [weekdayStr] = key.split("|");
      if (parseInt(weekdayStr, 10) === d) values.push(total);
    }
    medians.push(median(values));
  }

  return { cells, max, medians };
}

function median(values: number[]): number {
  if (values.length === 0) return 0;
  const sorted = values.slice().sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0 ? (sorted[mid - 1] + sorted[mid]) / 2 : sorted[mid];
}

// ═════════════════════════════════════════════════════════════════════════
// Events headline + timeline strip
// ═════════════════════════════════════════════════════════════════════════

interface EventsHeadlineCardProps {
  events: EventSpendingSummary[];
  currency: string;
  rangeStart: Date;
  rangeEnd: Date;
  /** Used to estimate "normal pace" outside event windows. */
  heatmapActivities: Activity[];
}

const EventsHeadlineCard: FC<EventsHeadlineCardProps> = ({
  events,
  currency,
  rangeStart,
  rangeEnd,
  heatmapActivities,
}) => {
  const computed = useMemo(
    () => computeEventsAggregate(events, heatmapActivities),
    [events, heatmapActivities],
  );

  const totalSpan = Math.max(1, inclusiveDays(rangeStart, rangeEnd));
  const months = useMemo(() => buildMonthColumns(rangeStart, rangeEnd), [rangeStart, rangeEnd]);
  const lanes = useMemo(() => assignLanes(events), [events]);
  const laneCount = Math.max(1, ...lanes.map((l) => l + 1));

  return (
    <div className={CARD_CLASS}>
      <p className="text-foreground max-w-[95%] font-serif text-base font-normal leading-snug tracking-tight md:text-lg">
        <span className="font-medium">{events.length}</span> tagged event
        {events.length === 1 ? "" : "s"} accounted for{" "}
        <span className="whitespace-nowrap font-medium">
          {formatAmount(computed.totalSpent, currency)}
        </span>{" "}
        across <span className="font-medium">{computed.totalEventDays}</span> days
        {computed.lift !== 0 && (
          <>
            {" — "}
            <span
              className={cn(
                "whitespace-nowrap font-medium",
                computed.lift > 0 ? "text-destructive" : "text-success",
              )}
            >
              {computed.lift > 0 ? "+" : "−"}
              {formatAmount(Math.abs(computed.lift), currency)}{" "}
              {computed.lift > 0 ? "above" : "below"} your normal pace
            </span>
            .
          </>
        )}
        {computed.topEventName && (
          <>
            {" "}
            <span className="font-medium">{computed.topEventName}</span> drove most of it.
          </>
        )}
      </p>

      {/* Compact swim-lane timeline. Month columns at the top; events as
          colored capsules placed by date below, stacked into lanes only when
          they overlap. Hover for full name + amount. */}
      <div className="border-border/40 mt-5 overflow-hidden rounded-lg border">
        {/* Month axis */}
        <div className="bg-muted/20 border-border/40 flex border-b">
          {months.map((m, i) => (
            <div
              key={i}
              className={cn(
                "border-border/40 text-muted-foreground/80 px-2 py-1 text-center text-[10px] uppercase tracking-wide",
                i < months.length - 1 && "border-r",
              )}
              style={{ width: `${m.widthPct}%` }}
            >
              {m.yearLabel && (
                <div className="text-foreground/70 text-[10px] font-semibold">{m.yearLabel}</div>
              )}
              <div>{m.label}</div>
            </div>
          ))}
        </div>

        {/* Lane area */}
        <div className="relative" style={{ height: laneCount * LANE_HEIGHT + LANE_PADDING * 2 }}>
          {/* Month grid lines */}
          <div className="pointer-events-none absolute inset-0 flex">
            {months.map((m, i) => (
              <div
                key={i}
                className={cn(
                  "border-border/30 h-full",
                  i < months.length - 1 && "border-r border-dashed",
                )}
                style={{ width: `${m.widthPct}%` }}
              />
            ))}
          </div>

          <TooltipProvider delayDuration={150}>
            {events.map((ev, i) => {
              const start = new Date(ev.startDate);
              const end = new Date(ev.endDate);
              const leftDays = dayOffset(rangeStart, start);
              const widthDays = Math.max(1, inclusiveDays(start, end));
              const left = Math.max(0, (leftDays / totalSpan) * 100);
              const width = Math.max(0.8, Math.min(100 - left, (widthDays / totalSpan) * 100));
              const color = ev.eventTypeColor ?? FOREST_THEME.deep;
              const lane = lanes[i];
              const dateLabel = formatGanttRange(start, end);
              return (
                <Tooltip key={ev.eventId}>
                  <TooltipTrigger asChild>
                    <button
                      type="button"
                      className="group absolute flex items-center overflow-hidden rounded-sm transition-all hover:z-10 hover:brightness-95 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--ring)]"
                      style={{
                        left: `${left}%`,
                        width: `${width}%`,
                        top: LANE_PADDING + lane * LANE_HEIGHT,
                        height: LANE_HEIGHT - 4,
                        backgroundColor: `${color}24`,
                        borderLeft: `3px solid ${color}`,
                      }}
                      aria-label={`${ev.eventName}, ${dateLabel}, ${formatAmount(ev.totalSpending, currency)}`}
                    >
                      {width >= 8 && (
                        <span
                          className="truncate pl-1.5 pr-1 text-[10px] font-semibold"
                          style={{ color }}
                        >
                          {ev.eventName}
                        </span>
                      )}
                    </button>
                  </TooltipTrigger>
                  <TooltipContent side="top" className="max-w-xs">
                    <div className="space-y-1">
                      <div className="flex items-center gap-2 text-sm font-semibold">
                        <span
                          className="block h-2 w-2 shrink-0 rounded-sm"
                          style={{ backgroundColor: color }}
                        />
                        {ev.eventName}
                      </div>
                      <div className="text-muted-foreground text-xs tabular-nums">
                        {dateLabel} · {ev.transactionCount} tx
                      </div>
                      <div className="text-foreground text-xs font-medium tabular-nums">
                        {formatAmount(ev.totalSpending, currency)}
                      </div>
                    </div>
                  </TooltipContent>
                </Tooltip>
              );
            })}
          </TooltipProvider>
        </div>
      </div>

      <div className="border-border/40 mt-4 flex flex-wrap items-baseline justify-between gap-2 border-t pt-3 text-[11px]">
        <span className="text-muted-foreground inline-flex items-center gap-2">
          <span className="bg-foreground/40 inline-block h-px w-5" />
          Your normal daily pace ({formatAmount(computed.normalPace, currency)})
        </span>
        <span className="text-muted-foreground/80 tabular-nums">
          Combined event days:{" "}
          <span className="text-foreground font-semibold">
            {computed.totalEventDays}/{totalSpan}
          </span>
        </span>
      </div>
    </div>
  );
};

interface EventsAggregate {
  totalSpent: number;
  totalEventDays: number;
  normalPace: number;
  lift: number;
  topEventName: string | null;
}

function computeEventsAggregate(
  events: EventSpendingSummary[],
  heatmapActivities: Activity[],
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

  // Normal pace = average daily outflow over the heatmap activity window
  // (last 12 weeks), excluding event days.
  const eventDayKeys = new Set<string>();
  for (const ev of events) {
    const start = new Date(ev.startDate);
    const end = new Date(ev.endDate);
    for (let d = new Date(start); d <= end; d.setDate(d.getDate() + 1)) {
      eventDayKeys.add(d.toISOString().slice(0, 10));
    }
  }
  let baselineTotal = 0;
  let baselineDays = 0;
  const baselineDaySeen = new Set<string>();
  for (const a of heatmapActivities) {
    if (!OUTFLOW_TYPES.includes(a.activityType as CashActivityType)) continue;
    const dayKey = new Date(a.activityDate).toISOString().slice(0, 10);
    if (eventDayKeys.has(dayKey)) continue;
    baselineTotal += parseFloat(a.amount ?? "0") || 0;
    baselineDaySeen.add(dayKey);
  }
  baselineDays = Math.max(1, baselineDaySeen.size);
  const normalPace = baselineTotal / baselineDays;
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

/** Inclusive day count between A and B — same day = 1, next day = 2, etc. */
function inclusiveDays(a: Date, b: Date): number {
  return Math.max(0, Math.round((b.getTime() - a.getTime()) / (24 * 3600 * 1000))) + 1;
}

/** Day offset of B from A — same day = 0, next day = 1. Used for positioning. */
function dayOffset(a: Date, b: Date): number {
  return Math.max(0, Math.round((b.getTime() - a.getTime()) / (24 * 3600 * 1000)));
}

/** Compact date-range label for a Gantt row. "May 6–8" / "Apr 28 – May 2". */
function formatGanttRange(start: Date, end: Date): string {
  const sameMonth =
    start.getMonth() === end.getMonth() && start.getFullYear() === end.getFullYear();
  if (sameMonth) {
    return start.getDate() === end.getDate()
      ? formatMonthDay(start)
      : `${formatMonthDay(start)}–${end.getDate()}`;
  }
  return `${formatMonthDay(start)} – ${formatMonthDay(end)}`;
}

// ─── Timeline helpers ────────────────────────────────────────────────────

const LANE_HEIGHT = 22;
const LANE_PADDING = 6;

const monthShort = new Intl.DateTimeFormat(undefined, { month: "short" });

interface MonthColumn {
  label: string;
  /** Year shown above the month label, only on the first column or on Jan. */
  yearLabel: string | null;
  /** Width of this column as a percentage of the full timeline width. */
  widthPct: number;
}

/** Compute month columns spanning [rangeStart, rangeEnd], sized by day count. */
function buildMonthColumns(rangeStart: Date, rangeEnd: Date): MonthColumn[] {
  const totalDays = Math.max(
    1,
    Math.round((rangeEnd.getTime() - rangeStart.getTime()) / 86_400_000) + 1,
  );
  const out: MonthColumn[] = [];
  const cursor = new Date(rangeStart.getFullYear(), rangeStart.getMonth(), 1);
  let lastYear: number | null = null;
  while (cursor <= rangeEnd) {
    const monthStart = cursor < rangeStart ? rangeStart : new Date(cursor);
    const monthEnd = new Date(cursor.getFullYear(), cursor.getMonth() + 1, 0, 23, 59, 59, 999);
    const clampedEnd = monthEnd > rangeEnd ? rangeEnd : monthEnd;
    const days = Math.max(
      1,
      Math.round((clampedEnd.getTime() - monthStart.getTime()) / 86_400_000) + 1,
    );
    const year = cursor.getFullYear();
    out.push({
      label: monthShort.format(cursor),
      // Show year on the first column AND on every January after — keeps
      // multi-year ranges navigable without repeating the year on every cell.
      yearLabel: lastYear == null || year !== lastYear ? String(year) : null,
      widthPct: (days / totalDays) * 100,
    });
    lastYear = year;
    cursor.setMonth(cursor.getMonth() + 1);
  }
  return out;
}

/** Assign each event a lane index (0-based) so overlapping events stack. */
function assignLanes(events: EventSpendingSummary[]): number[] {
  // Indices preserved against the input order so the caller can map back.
  const indexed = events.map((e, i) => ({ i, start: e.startDate, end: e.endDate }));
  indexed.sort((a, b) => a.start.localeCompare(b.start));
  const laneEnds: string[] = [];
  const laneByIndex = new Array(events.length).fill(0);
  for (const ev of indexed) {
    let placed = false;
    for (let l = 0; l < laneEnds.length; l++) {
      if (laneEnds[l] < ev.start) {
        laneEnds[l] = ev.end;
        laneByIndex[ev.i] = l;
        placed = true;
        break;
      }
    }
    if (!placed) {
      laneByIndex[ev.i] = laneEnds.length;
      laneEnds.push(ev.end);
    }
  }
  return laneByIndex;
}

// ═════════════════════════════════════════════════════════════════════════
// Per-event detail card
// ═════════════════════════════════════════════════════════════════════════

interface EventDetailCardProps {
  event: EventSpendingSummary;
  taxonomyCategories: TaxonomyCategory[];
  currency: string;
  heatmapActivities: Activity[];
}

const EventDetailCard: FC<EventDetailCardProps> = ({
  event,
  taxonomyCategories,
  currency,
  heatmapActivities,
}) => {
  const startDate = new Date(event.startDate);
  const endDate = new Date(event.endDate);
  const days = Math.max(1, inclusiveDays(startDate, endDate));

  const baseline = useMemo(
    () => computeBaselinePace(heatmapActivities, [event]),
    [heatmapActivities, event],
  );

  const expected = baseline * days;
  const lift = event.totalSpending - expected;
  const multiple = expected > 0 ? event.totalSpending / expected : 0;
  const dailyDuring = days > 0 ? event.totalSpending / days : 0;

  const topCategories = useMemo(
    () => buildEventCategoryRows(event, taxonomyCategories).slice(0, 5),
    [event, taxonomyCategories],
  );
  const maxCategoryAmount = topCategories[0]?.amount ?? 1;
  const maxRate = Math.max(baseline, dailyDuring, 1);

  const tag = event.eventTypeName ?? "Event";
  const tagColor = event.eventTypeColor ?? FOREST_THEME.deep;
  const HeaderIcon = pickEventIcon(tag);

  const rangeLabel = formatRange(startDate, endDate);

  const caption = useMemo(
    () => buildEventCaption({ days, lift, currency, top: topCategories }),
    [days, lift, currency, topCategories],
  );

  return (
    <div className={CARD_CLASS}>
      <div className="flex items-center gap-2.5">
        <span
          className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg"
          style={{ backgroundColor: `${tagColor}1F`, color: tagColor }}
        >
          <HeaderIcon className="h-4 w-4" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="text-foreground truncate text-sm font-semibold">{event.eventName}</div>
          <div className="text-muted-foreground/80 text-[11px] uppercase tabular-nums tracking-wide">
            {rangeLabel} · {days} {days === 1 ? "DAY" : "DAYS"} · {event.transactionCount} TX
          </div>
        </div>
      </div>

      <div className="mt-4 grid grid-cols-2 gap-3">
        <div>
          <div className={LABEL_CLASS}>TOTAL</div>
          <div className="text-foreground mt-1 text-lg font-semibold tabular-nums tracking-tight">
            {formatAmount(event.totalSpending, currency)}
          </div>
        </div>
        <div>
          <div className={LABEL_CLASS}>LIFT VS NORMAL</div>
          <div
            className={cn(
              "mt-1 text-lg font-semibold tabular-nums tracking-tight",
              lift > 0 ? "text-destructive" : "text-success",
            )}
          >
            {lift >= 0 ? "+" : "−"}
            {formatAmount(Math.abs(lift), currency)}
            {multiple > 0 && (
              <span className="text-muted-foreground/70 ml-1 text-xs font-medium">
                ·{multiple.toFixed(1)}×
              </span>
            )}
          </div>
        </div>
      </div>

      <div className="mt-5">
        <div className={LABEL_CLASS}>DAILY RATE</div>
        <div className="mt-2 space-y-1.5">
          <RateRow
            label="Normal"
            value={baseline}
            currency={currency}
            maxRate={maxRate}
            tone="muted"
          />
          <RateRow
            label="During"
            value={dailyDuring}
            currency={currency}
            maxRate={maxRate}
            tone={dailyDuring > baseline ? "warn" : "good"}
          />
        </div>
      </div>

      {topCategories.length > 0 && (
        <div className="mt-5">
          <div className={LABEL_CLASS}>WHERE IT WENT</div>
          <div className="mt-2 space-y-1.5">
            {topCategories.map((c) => (
              <div key={c.id} className="flex items-center gap-2 text-[11px]">
                <span
                  className="block h-1.5 w-1.5 shrink-0 rounded-full"
                  style={{ backgroundColor: c.color }}
                />
                <span className="text-foreground/90 w-24 shrink-0 truncate text-xs">{c.name}</span>
                <div className="bg-foreground/5 h-1.5 flex-1 overflow-hidden rounded-full">
                  <div
                    className="h-full rounded-full transition-all"
                    style={{
                      width: `${(c.amount / maxCategoryAmount) * 100}%`,
                      backgroundColor: c.color,
                      opacity: 0.8,
                    }}
                  />
                </div>
                <span className="text-foreground/90 w-16 shrink-0 text-right text-xs font-semibold tabular-nums">
                  {formatAmount(c.amount, currency)}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

      <p className="text-muted-foreground/80 mt-4 text-[11px] italic">{caption}</p>
    </div>
  );
};

function RateRow({
  label,
  value,
  currency,
  maxRate,
  tone,
}: {
  label: string;
  value: number;
  currency: string;
  maxRate: number;
  tone: "muted" | "warn" | "good";
}) {
  const fill =
    tone === "warn"
      ? "var(--destructive)"
      : tone === "good"
        ? "var(--success)"
        : "var(--muted-foreground)";
  return (
    <div className="flex items-center gap-2 text-[11px]">
      <span className="text-muted-foreground w-14 shrink-0">{label}</span>
      <div className="bg-foreground/5 h-1.5 flex-1 overflow-hidden rounded-full">
        <div
          className="h-full rounded-full transition-all"
          style={{
            width: `${Math.min(100, (value / maxRate) * 100)}%`,
            backgroundColor: fill,
            opacity: tone === "muted" ? 0.5 : 0.7,
          }}
        />
      </div>
      <span className="text-foreground/90 w-20 shrink-0 text-right text-xs font-semibold tabular-nums">
        {formatAmount(value, currency)}
      </span>
    </div>
  );
}

interface EventCategoryRow {
  id: string;
  name: string;
  color: string;
  amount: number;
}

function buildEventCategoryRows(
  event: EventSpendingSummary,
  taxonomyCategories: TaxonomyCategory[],
): EventCategoryRow[] {
  const meta = new Map(taxonomyCategories.map((c) => [c.id, c]));
  const byTop = new Map<string, EventCategoryRow>();
  for (const cat of Object.values(event.byCategory)) {
    const m = cat.categoryId ? meta.get(cat.categoryId) : undefined;
    const topId = m?.parentId ?? cat.categoryId ?? cat.categoryName;
    const top = (m?.parentId && meta.get(m.parentId)) || m;
    const name = top?.name ?? cat.categoryName ?? "Uncategorized";
    const color = top?.color ?? cat.color ?? "#9CA3AF";
    const e = byTop.get(topId) ?? { id: topId, name, color, amount: 0 };
    e.amount += cat.amount;
    byTop.set(topId, e);
  }
  return Array.from(byTop.values()).sort((a, b) => b.amount - a.amount);
}

function computeBaselinePace(
  activities: Activity[],
  excludeEvents: EventSpendingSummary[],
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
    if (!OUTFLOW_TYPES.includes(a.activityType as CashActivityType)) continue;
    const dayKey = new Date(a.activityDate).toISOString().slice(0, 10);
    if (exclude.has(dayKey)) continue;
    total += parseFloat(a.amount ?? "0") || 0;
    seen.add(dayKey);
  }
  return seen.size === 0 ? 0 : total / seen.size;
}

function buildEventCaption({
  days,
  lift,
  currency,
  top,
}: {
  days: number;
  lift: number;
  currency: string;
  top: EventCategoryRow[];
}): string {
  if (top.length === 0) {
    return lift > 0
      ? `Lift vs your normal week: +${formatAmount(lift, currency)} over ${days} days.`
      : `In line with your normal week.`;
  }
  if (lift > 0 && days <= 4) {
    if (top.length === 1) {
      return `One-off — ${top[0].name} drove the spike.`;
    }
    return `One-off — ${top[0].name} and ${top[1].name} drove the spike.`;
  }
  if (Math.abs(lift) < 50) {
    return `Mostly ${top[0].name.toLowerCase()} — modest lift over a normal stretch.`;
  }
  return `Lift vs your normal week: ${lift >= 0 ? "+" : "−"}${formatAmount(Math.abs(lift), currency)} over ${days} days.`;
}

function pickEventIcon(name: string): typeof Icons.Calendar {
  const n = name.toLowerCase();
  if (n.includes("trip") || n.includes("travel") || n.includes("flight"))
    return Icons.Plane ?? Icons.Calendar;
  if (n.includes("birthday") || n.includes("party") || n.includes("celebration"))
    return Icons.Star ?? Icons.Calendar;
  if (n.includes("move") || n.includes("apartment") || n.includes("home"))
    return Icons.Home ?? Icons.Calendar;
  if (n.includes("wedding")) return Icons.Sparkles ?? Icons.Calendar;
  return Icons.Calendar;
}

function formatRange(start: Date, end: Date): string {
  const sameMonth =
    start.getMonth() === end.getMonth() && start.getFullYear() === end.getFullYear();
  return sameMonth
    ? `${formatMonthDay(start)}–${end.getDate()}`.toUpperCase()
    : `${formatMonthDay(start)} – ${formatMonthDay(end)}`.toUpperCase();
}

// ═════════════════════════════════════════════════════════════════════════
// Empty events state
// ═════════════════════════════════════════════════════════════════════════

function EmptyEventsCard() {
  return (
    <div className={CARD_CLASS}>
      <div className={LABEL_CLASS}>NO TAGGED EVENTS</div>
      <p className="text-foreground mt-3 text-base font-semibold leading-snug">
        Tag a trip or one-off to see how it stacks up against your normal week.
      </p>
      <p className="text-muted-foreground/80 mt-2 text-sm">
        Events let you isolate spend that isn't part of your usual rhythm — so the rest of this view
        stays meaningful.
      </p>
    </div>
  );
}
