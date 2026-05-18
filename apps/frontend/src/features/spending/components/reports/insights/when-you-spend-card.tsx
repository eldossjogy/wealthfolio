/**
 * Weekday × hour heatmap. Pulled out of when-where-stage.tsx so the larger
 * file can focus on the events timeline + detail panel; this card is fully
 * self-contained (no shared state with the rest of the stage).
 */
import { useMemo, type FC } from "react";

import { formatCompactAmount } from "@wealthfolio/ui";
import type { Activity } from "@/lib/types";
import { cn, formatAmount } from "@/lib/utils";

import { getActivitySpendingAmount } from "../../../lib/constants";

const CARD_CLASS = "border-border/60 bg-card/40 rounded-2xl border p-5 backdrop-blur-xl";
const LABEL_CLASS = "text-muted-foreground/70 text-[10px] font-normal uppercase tracking-[0.12em]";

const DAY_NAMES = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const HOUR_LABELS = ["12a", "3a", "6a", "9a", "12p", "3p", "6p", "9p"];

export interface WhenYouSpendCardProps {
  activities: Activity[];
  accountTypeById?: Map<string, string>;
  currency: string;
  onCellClick?: (weekday: number, hour: number) => void;
}

export const WhenYouSpendCard: FC<WhenYouSpendCardProps> = ({
  activities,
  accountTypeById,
  currency,
  onCellClick,
}) => {
  const grid = useMemo(
    () => buildWeekdayHourGrid(activities, accountTypeById),
    [accountTypeById, activities],
  );

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
          {HOUR_LABELS.map((h, i) => (
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
              weekdayIndex={di}
              cells={row}
              max={grid.max}
              median={median}
              currency={currency}
              onCellClick={onCellClick}
            />
          );
        })}
      </div>

      <div className="border-border/40 mt-4 flex items-center justify-between border-t pt-3 text-[11px]">
        <span className="text-muted-foreground/70">
          Each cell is one weekday-hour over 12 weeks. <span className="dark:hidden">Darker</span>
          <span className="hidden dark:inline">Brighter</span> = more spend.
        </span>
        <Legend />
      </div>
    </div>
  );
};

function Row({
  day,
  weekdayIndex,
  cells,
  max,
  median,
  currency,
  onCellClick,
}: {
  day: string;
  weekdayIndex: number;
  cells: number[];
  max: number;
  median: number;
  currency: string;
  onCellClick?: (weekday: number, hour: number) => void;
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
          const opacity =
            amount === 0
              ? "var(--heatmap-empty-opacity)"
              : `calc(var(--heatmap-min-opacity) + ${t} * var(--heatmap-range-opacity))`;
          const label = `${day} ${formatHour(i)} · ${amount > 0 ? formatAmount(amount, currency) : "no spend"}`;
          if (!onCellClick) {
            return (
              <div
                key={i}
                className="aspect-square rounded-[3px]"
                style={{ backgroundColor: "var(--heatmap-accent)", opacity }}
                title={label}
              />
            );
          }
          return (
            <button
              key={i}
              type="button"
              onClick={() => onCellClick(weekdayIndex, i)}
              className="aspect-square rounded-[3px] transition-all hover:scale-110 hover:ring-1 hover:ring-[var(--ring)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--ring)]"
              style={{ backgroundColor: "var(--heatmap-accent)", opacity }}
              title={label}
              aria-label={label}
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

function Legend() {
  return (
    <span className="text-muted-foreground/70 inline-flex items-center gap-1.5">
      <span>less</span>
      {[0.18, 0.4, 0.65, 0.95].map((o, i) => (
        <span
          key={i}
          className="h-2.5 w-2.5 rounded-[2px]"
          style={{ backgroundColor: "var(--heatmap-accent)", opacity: o }}
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

function buildWeekdayHourGrid(
  activities: Activity[],
  accountTypeById?: Map<string, string>,
): WeekdayHourGrid {
  const cells: number[][] = Array.from({ length: 7 }, () => new Array(24).fill(0));
  // Per (weekday, dayKey) → daily total. Used to compute the median per weekday.
  const dayTotals = new Map<string, number>();

  for (const a of activities) {
    const amt = getActivitySpendingAmount(a, accountTypeById?.get(a.accountId));
    if (amt === 0) continue;
    const date = new Date(a.activityDate);
    if (isNaN(date.getTime())) continue;
    const weekday = (date.getDay() + 6) % 7; // Mon=0..Sun=6
    const hour = date.getHours();
    cells[weekday][hour] += amt;
    const key = `${weekday}|${date.toISOString().slice(0, 10)}`;
    dayTotals.set(key, (dayTotals.get(key) ?? 0) + amt);
  }

  for (const row of cells) {
    for (let i = 0; i < row.length; i++) {
      if (row[i] < 0) row[i] = 0;
    }
  }
  for (const [key, value] of dayTotals) {
    if (value <= 0) dayTotals.delete(key);
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
