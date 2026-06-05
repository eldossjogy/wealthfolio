import { useMemo } from "react";
import { Link } from "react-router-dom";

import { DashboardCard } from "@/components/dashboard-card";
import { cn } from "@/lib/utils";
import { formatCompactAmount, Icons, PrivacyAmount, useBalancePrivacy } from "@wealthfolio/ui";

import { CategoryIcon, type CategoryMetaMap } from "./category-chips";
import { topCategoryId } from "../lib/category-rollup";
import type { BudgetCategoryRow } from "../types/budget";
import type { DayBucket } from "../types/report";

type Status = "ok" | "warn" | "over";
type PacePoint = { day: number; value: number };

const MIN_HISTORICAL_PACE_MONTHS = 2;

const STATUS_ACCENTS: Record<
  Status,
  {
    lineColor: string;
    pillBg: string;
    accent: string;
    Icon: typeof Icons.AlertCircle;
    label: string;
  }
> = {
  over: {
    lineColor: "#B85544",
    pillBg: "var(--destructive)",
    accent: "var(--destructive)",
    Icon: Icons.AlertTriangle,
    label: "Over budget",
  },
  warn: {
    lineColor: "#C28B47",
    pillBg: "#C28B47",
    accent: "#C28B47",
    Icon: Icons.AlertCircle,
    label: "Trending high",
  },
  ok: {
    lineColor: "hsl(73 84% 27%)",
    pillBg: "hsl(73 84% 27%)",
    accent: "var(--success)",
    Icon: Icons.CheckCircle ?? Icons.AlertCircle,
    label: "On track",
  },
};

export function BudgetLineChartCard({
  target,
  spent,
  currency,
  historicalDailyAvg,
  allocations,
  spendingBreakdown,
  categoriesMeta,
  monthByDay,
  historicalByDay,
}: {
  target: number;
  spent: number;
  currency: string;
  historicalDailyAvg: number;
  allocations: BudgetCategoryRow[];
  spendingBreakdown: { categoryId: string; amount: number; count: number }[];
  categoriesMeta: CategoryMetaMap;
  monthByDay: DayBucket[];
  historicalByDay: DayBucket[];
}) {
  // All hooks must run unconditionally — the `target <= 0` early return below
  // sits between hooks otherwise, which trips "Rendered more hooks than during
  // the previous render" when a target is added or cleared.
  const monthMeta = useMemo(() => {
    const now = new Date();
    const dayOfMonth = now.getDate();
    const daysInMonth = new Date(now.getFullYear(), now.getMonth() + 1, 0).getDate();
    return {
      now,
      dayOfMonth,
      daysInMonth,
      daysRemaining: Math.max(0, daysInMonth - dayOfMonth),
      monthLabel: now.toLocaleString("en-US", { month: "long", year: "numeric" }).toUpperCase(),
      shortLabel: now.toLocaleString("en-US", { month: "short", year: "numeric" }).toUpperCase(),
    };
  }, []);
  const { dayOfMonth, daysInMonth, daysRemaining, monthLabel } = monthMeta;

  const cumulative = useMemo(() => {
    const byDay = new Map<number, number>();
    for (const b of monthByDay) {
      const d = parseInt(b.date.split("-")[2], 10);
      if (Number.isFinite(d)) byDay.set(d, (byDay.get(d) ?? 0) + b.outflow);
    }
    let running = 0;
    const out: { day: number; value: number }[] = [];
    for (let d = 1; d <= dayOfMonth; d++) {
      running += byDay.get(d) ?? 0;
      out.push({ day: d, value: running });
    }
    return out;
  }, [monthByDay, dayOfMonth]);

  const rings = useMemo(() => {
    const spentByTop = new Map<string, number>();
    for (const row of spendingBreakdown) {
      const topId = topCategoryId(row.categoryId, categoriesMeta);
      spentByTop.set(topId, (spentByTop.get(topId) ?? 0) + row.amount);
    }
    return allocations
      .map((al) => {
        const t = al.target || 0;
        if (t <= 0) return null;
        const meta = categoriesMeta.get(al.categoryId);
        const s = spentByTop.get(al.categoryId) ?? 0;
        return {
          id: al.categoryId,
          categoryId: al.categoryId,
          name: meta?.name ?? al.categoryId,
          color: meta?.color ?? null,
          icon: meta?.icon ?? null,
          target: t,
          spent: Math.max(0, s),
          pct: Math.max(0, s) / t,
        };
      })
      .filter((r): r is NonNullable<typeof r> => r !== null)
      .sort((x, y) => y.pct - x.pct);
  }, [allocations, spendingBreakdown, categoriesMeta]);

  // Chart geometry derived from target — captured here so actualPath useMemo
  // can depend on stable primitives instead of recomputing each render.
  const chartW = 320;
  const chartH = 110;
  const padL = 0;
  const padR = 0;
  const padT = 24;
  const padB = 14;
  const innerW = chartW - padL - padR;
  const innerH = chartH - padT - padB;
  const yMax = Math.max(target, spent) * 1.05;

  const actualPath = useMemo(() => {
    if (!cumulative.length) return "";
    const xForDay = (day: number) => padL + ((day - 1) / Math.max(1, daysInMonth - 1)) * innerW;
    const yForVal = (v: number) => padT + (1 - v / yMax) * innerH;
    return (
      "M " +
      cumulative
        .map((p) => `${xForDay(p.day).toFixed(2)} ${yForVal(p.value).toFixed(2)}`)
        .join(" L ")
    );
  }, [cumulative, daysInMonth, innerW, innerH, padL, padT, yMax]);

  const historicalPace = useMemo(
    () => buildHistoricalPaceCurve(historicalByDay, daysInMonth),
    [historicalByDay, daysInMonth],
  );

  const targetPacePath = useMemo(() => {
    if (!historicalPace || target <= 0) return "";
    const xForDay = (day: number) => padL + ((day - 1) / Math.max(1, daysInMonth - 1)) * innerW;
    const yForVal = (v: number) => padT + (1 - v / yMax) * innerH;
    return toSvgPath(
      historicalPace.points.map((p) => ({
        day: p.day,
        value: p.value * target,
      })),
      xForDay,
      yForVal,
    );
  }, [historicalPace, target, daysInMonth, innerW, innerH, padL, padT, yMax]);

  const haveHistory = historicalDailyAvg > 0;
  const forecast =
    target > 0
      ? haveHistory
        ? spent + historicalDailyAvg * daysRemaining
        : dayOfMonth > 0
          ? (spent / dayOfMonth) * daysInMonth
          : 0
      : 0;

  if (target <= 0) {
    return (
      <DashboardCard
        title="Monthly budget"
        subtitle={monthMeta.shortLabel}
        action={<BudgetManageLink />}
        className="text-center"
      >
        <p className="text-muted-foreground text-sm">No monthly target set yet.</p>
        <Link
          to="/settings/spending/setup"
          className="text-foreground mt-2 inline-flex text-xs underline-offset-4 hover:underline"
        >
          Set a budget →
        </Link>
      </DashboardCard>
    );
  }

  const remaining = Math.max(0, target - spent);
  const overBy = spent - target;
  const isOver = overBy > 0;
  const forecastReliable = haveHistory || dayOfMonth >= 7;
  const forecastDelta = forecast - target;
  const willOverspend = forecastReliable && forecastDelta > 0;

  const historicalPaceAtToday = historicalPace?.pctByDay[dayOfMonth];
  const paceAtToday =
    target *
    (historicalPaceAtToday !== undefined ? historicalPaceAtToday : dayOfMonth / daysInMonth);
  const gapVsPace = spent - paceAtToday;
  const aheadOfPace = gapVsPace < 0;

  const status: Status = isOver ? "over" : !aheadOfPace ? "warn" : "ok";
  const a = STATUS_ACCENTS[status];
  const { Icon } = a;

  const xForDay = (day: number) => padL + ((day - 1) / Math.max(1, daysInMonth - 1)) * innerW;
  const yForVal = (v: number) => padT + (1 - v / yMax) * innerH;

  const paceX1 = xForDay(1);
  const paceY1 = yForVal(0);
  const paceX2 = xForDay(daysInMonth);
  const paceY2 = yForVal(target);

  const endX = cumulative.length ? xForDay(cumulative[cumulative.length - 1].day) : padL;
  const endY = cumulative.length ? yForVal(cumulative[cumulative.length - 1].value) : padT + innerH;

  const gapAbs = Math.abs(gapVsPace);
  const gapLabel = isOver
    ? `${formatCompactAmount(overBy, currency)} over budget`
    : aheadOfPace
      ? `${formatCompactAmount(gapAbs, currency)} under budget`
      : `${formatCompactAmount(gapAbs, currency)} over pace`;

  const pillLeftPctRaw = (endX / chartW) * 100;
  const pillLeftPct = Math.min(78, Math.max(8, pillLeftPctRaw - 4));
  // Near the right edge, anchor the badge to the endpoint and grow leftward so
  // it never overflows the card.
  const pillFlip = pillLeftPctRaw > 55;
  const pillTopPx = Math.max(0, endY - 28);

  return (
    <DashboardCard title="Monthly budget" subtitle={monthLabel} action={<BudgetManageLink />}>
      <div className="flex items-center gap-2">
        <Icon className="h-4 w-4 shrink-0" style={{ color: a.accent }} />
        <span className="text-foreground text-sm font-semibold">{a.label}</span>
        <span className="text-muted-foreground/70 ml-auto text-xs tabular-nums">
          Day {dayOfMonth} / {daysInMonth}
        </span>
      </div>

      <div className="mt-3">
        {willOverspend && forecastDelta > target * 0.05 ? (
          <>
            <div className="text-foreground text-2xl font-bold tabular-nums tracking-tight">
              <PrivacyAmount value={forecast} currency={currency} />{" "}
              <span className="text-muted-foreground/70 text-base font-medium">forecast</span>
            </div>
            <div className="text-destructive mt-0.5 inline-flex items-center gap-1 text-xs font-semibold tabular-nums">
              <Icons.ArrowUp className="h-3 w-3" />
              <PrivacyAmount value={forecastDelta} currency={currency} /> over budget
            </div>
            <div className="text-muted-foreground/80 mt-0.5 text-xs tabular-nums">
              <PrivacyAmount value={remaining} currency={currency} /> left today · of{" "}
              <PrivacyAmount value={target} currency={currency} /> budgeted this month
            </div>
          </>
        ) : (
          <>
            <div className="text-foreground text-2xl font-bold tabular-nums tracking-tight">
              <PrivacyAmount value={isOver ? overBy : remaining} currency={currency} />{" "}
              <span className="text-muted-foreground/70 text-base font-medium">
                {isOver ? "over" : "left"}
              </span>
            </div>
            <div className="text-muted-foreground/80 text-xs tabular-nums">
              of <PrivacyAmount value={target} currency={currency} /> budgeted this month
            </div>
          </>
        )}
      </div>

      <div className="relative mt-4 w-full">
        <svg
          viewBox={`0 0 ${chartW} ${chartH}`}
          preserveAspectRatio="none"
          className="block h-[110px] w-full"
        >
          {targetPacePath ? (
            <path
              d={targetPacePath}
              fill="none"
              stroke="var(--muted-foreground)"
              strokeOpacity={0.35}
              strokeDasharray="3 4"
              strokeWidth={1.25}
              strokeLinecap="round"
              strokeLinejoin="round"
              vectorEffect="non-scaling-stroke"
            />
          ) : (
            <line
              x1={paceX1}
              y1={paceY1}
              x2={paceX2}
              y2={paceY2}
              stroke="var(--muted-foreground)"
              strokeOpacity={0.35}
              strokeDasharray="3 4"
              strokeWidth={1.25}
              vectorEffect="non-scaling-stroke"
            />
          )}
          {actualPath && (
            <path
              d={actualPath}
              fill="none"
              stroke={a.lineColor}
              strokeWidth={2.5}
              strokeLinecap="round"
              strokeLinejoin="round"
              vectorEffect="non-scaling-stroke"
            />
          )}
        </svg>
        {cumulative.length > 0 && (
          // Rendered as HTML rather than an SVG <circle> so it stays round: the
          // SVG uses preserveAspectRatio="none", which would stretch a circle
          // into an ellipse.
          <div
            className="absolute h-[9px] w-[9px] rounded-full bg-white"
            style={{
              left: `${pillLeftPctRaw}%`,
              top: `${endY}px`,
              transform: "translate(-50%, -50%)",
              border: `2.5px solid ${a.lineColor}`,
            }}
          />
        )}
        {cumulative.length > 0 && (
          <div
            className="absolute whitespace-nowrap rounded-full px-2 py-0.5 text-[11px] font-semibold tabular-nums shadow-sm"
            style={{
              left: `${pillFlip ? pillLeftPctRaw : pillLeftPct}%`,
              top: `${pillTopPx}px`,
              transform: pillFlip ? "translateX(calc(-100% - 6px))" : undefined,
              backgroundColor: a.pillBg,
              color: "white",
            }}
          >
            {gapLabel}
          </div>
        )}
      </div>
      <div className="text-muted-foreground/70 mt-1 flex justify-between text-[10px] tabular-nums">
        <span>Day 1</span>
        <span>Day {daysInMonth}</span>
      </div>

      <div className="border-border mt-4 grid grid-cols-2 gap-3 border-t pt-3 text-xs">
        <div>
          <div className="text-muted-foreground/70 text-[11px] uppercase tracking-wide">
            Spent so far
          </div>
          <div className="text-foreground text-sm font-semibold tabular-nums">
            <PrivacyAmount value={spent} currency={currency} />
          </div>
        </div>
        <div className="text-right">
          <div className="text-muted-foreground/70 text-[11px] uppercase tracking-wide">
            Forecast
          </div>
          <div
            className={cn(
              "text-sm font-semibold tabular-nums",
              forecastReliable
                ? willOverspend
                  ? "text-destructive"
                  : "text-foreground"
                : "text-muted-foreground/60",
            )}
          >
            {forecastReliable ? <PrivacyAmount value={forecast} currency={currency} /> : "—"}
          </div>
          <div className="text-muted-foreground/60 text-[10px]">
            {forecastReliable
              ? haveHistory
                ? "vs last 3 months"
                : "at current pace"
              : "more data needed"}
          </div>
        </div>
      </div>

      <div className="border-border mt-5 border-t pt-4">
        <div className="mb-3 flex items-center justify-between">
          <span className="text-muted-foreground/80 text-[11px] font-semibold uppercase tracking-wide">
            By category
          </span>
          <Link
            to="/spending/budget"
            className="text-muted-foreground hover:text-foreground text-xs underline-offset-4 hover:underline"
          >
            Manage →
          </Link>
        </div>
        {rings.length === 0 ? (
          <div className="text-muted-foreground py-2 text-center text-xs">
            No category budgets set yet.{" "}
            <Link
              to="/settings/spending/setup"
              className="hover:text-foreground underline-offset-4 hover:underline"
            >
              Set one
            </Link>
          </div>
        ) : (
          <div
            data-no-swipe-drag
            className="-mx-1 flex gap-3 overflow-x-auto px-1 pb-1"
            style={{
              maskImage: "linear-gradient(to right, black calc(100% - 32px), transparent 100%)",
              WebkitMaskImage:
                "linear-gradient(to right, black calc(100% - 32px), transparent 100%)",
            }}
          >
            {rings.map((r) => (
              <BudgetRing key={r.id} ring={r} currency={currency} />
            ))}
          </div>
        )}
      </div>
    </DashboardCard>
  );
}

function buildHistoricalPaceCurve(
  byDay: DayBucket[],
  currentDaysInMonth: number,
): { points: PacePoint[]; pctByDay: number[] } | null {
  const months = new Map<
    string,
    { daysInMonth: number; outflowByDay: Map<number, number>; total: number }
  >();

  for (const bucket of byDay) {
    const parsed = parseDayBucketDate(bucket.date);
    if (!parsed) continue;
    const outflow = Number.isFinite(bucket.outflow) ? bucket.outflow : 0;

    const key = `${parsed.year}-${String(parsed.month).padStart(2, "0")}`;
    const month = months.get(key) ?? {
      daysInMonth: new Date(parsed.year, parsed.month, 0).getDate(),
      outflowByDay: new Map<number, number>(),
      total: 0,
    };
    month.outflowByDay.set(parsed.day, (month.outflowByDay.get(parsed.day) ?? 0) + outflow);
    month.total += outflow;
    months.set(key, month);
  }

  const eligibleMonths = Array.from(months.values())
    .filter((month) => month.total > 0)
    .map((month) => {
      const cumulativeByDay = Array.from({ length: month.daysInMonth + 1 }, () => 0);
      let running = 0;
      for (let day = 1; day <= month.daysInMonth; day++) {
        running += month.outflowByDay.get(day) ?? 0;
        cumulativeByDay[day] = Math.max(cumulativeByDay[day - 1], clamp(running, 0, month.total));
      }
      return { ...month, cumulativeByDay };
    });

  if (eligibleMonths.length < MIN_HISTORICAL_PACE_MONTHS) return null;

  const pctByDay = Array.from({ length: currentDaysInMonth + 1 }, () => 0);
  const points: PacePoint[] = [];
  for (let day = 1; day <= currentDaysInMonth; day++) {
    const values = eligibleMonths.map((month) => {
      const historyDay = Math.min(
        month.daysInMonth,
        Math.max(1, Math.ceil((day / currentDaysInMonth) * month.daysInMonth)),
      );
      return clamp(month.cumulativeByDay[historyDay] / month.total, 0, 1);
    });
    const value = median(values);
    pctByDay[day] = value;
    points.push({ day, value });
  }

  return { points, pctByDay };
}

function parseDayBucketDate(date: string): { year: number; month: number; day: number } | null {
  const [yearRaw, monthRaw, dayRaw] = date.split("-");
  const year = Number(yearRaw);
  const month = Number(monthRaw);
  const day = Number(dayRaw);
  if (!Number.isInteger(year) || !Number.isInteger(month) || !Number.isInteger(day)) return null;
  if (month < 1 || month > 12 || day < 1 || day > 31) return null;
  return { year, month, day };
}

function toSvgPath(
  points: PacePoint[],
  xForDay: (day: number) => number,
  yForVal: (value: number) => number,
): string {
  if (!points.length) return "";
  return (
    "M " +
    points.map((p) => `${xForDay(p.day).toFixed(2)} ${yForVal(p.value).toFixed(2)}`).join(" L ")
  );
}

function median(values: number[]): number {
  if (!values.length) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

const BudgetManageLink = () => (
  <Link
    to="/spending/budget"
    className="text-muted-foreground hover:text-foreground text-xs underline-offset-4 hover:underline"
  >
    Manage →
  </Link>
);

function BudgetRing({
  ring,
  currency,
}: {
  ring: {
    categoryId: string;
    name: string;
    color: string | null;
    icon: string | null;
    target: number;
    spent: number;
    pct: number;
  };
  currency: string;
}) {
  const { isBalanceHidden } = useBalancePrivacy();
  const isOver = ring.spent > ring.target;
  const remaining = ring.target - ring.spent;
  const ringColor = isOver ? "var(--destructive)" : ring.pct > 0.85 ? "#C28B47" : "var(--success)";
  const displayAmount = Math.abs(isOver ? ring.spent - ring.target : remaining);

  const size = 56;
  const stroke = 4;
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  const fillPct = Math.min(1, ring.pct);
  const dash = `${c * fillPct} ${c}`;

  return (
    <Link
      to={`/activities?tab=spending&category=${encodeURIComponent(ring.categoryId)}`}
      className="hover:bg-muted/40 flex shrink-0 flex-col items-center gap-1 rounded-md px-1 py-1 transition-colors"
      title={`${ring.name}: ${ring.spent.toFixed(2)} / ${ring.target.toFixed(2)}`}
    >
      <div className="relative" style={{ width: size, height: size }}>
        <svg width={size} height={size} className="-rotate-90">
          <circle
            cx={size / 2}
            cy={size / 2}
            r={r}
            fill="none"
            stroke={ringColor}
            strokeOpacity={0.22}
            strokeWidth={stroke}
          />
          <circle
            cx={size / 2}
            cy={size / 2}
            r={r}
            fill="none"
            stroke={ringColor}
            strokeWidth={stroke}
            strokeLinecap="round"
            strokeDasharray={dash}
          />
        </svg>
        <div
          className="absolute inset-0 flex items-center justify-center"
          style={{ color: ring.color ?? ringColor }}
        >
          <CategoryIcon icon={ring.icon} fallback={ring.name} className="h-5 w-5" />
        </div>
      </div>
      <div className="text-foreground text-xs font-semibold tabular-nums">
        {isBalanceHidden ? "••••" : formatCompactAmount(displayAmount, currency)}
      </div>
      <div
        className={cn(
          "text-[10px] uppercase tracking-wide",
          isOver ? "text-destructive" : "text-muted-foreground/70",
        )}
      >
        {isOver ? "over" : "left"}
      </div>
    </Link>
  );
}
