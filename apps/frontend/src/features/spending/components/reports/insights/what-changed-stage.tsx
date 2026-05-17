import { useMemo, useState, type FC, type ReactNode } from "react";
import { Area, AreaChart, ResponsiveContainer } from "recharts";

import { Skeleton, formatCompactAmount } from "@wealthfolio/ui";
import type { TaxonomyCategory } from "@/lib/types";
import { cn, formatAmount } from "@/lib/utils";

import { CategoryIcon } from "../../category-chips";
import type { MonthBucket } from "../../../hooks/use-monthly-history";
import type { ReportsRange } from "../../../lib/reports-period";
import type { CategoryBreakdownRow, DayCategoryBucket, MonthlyReport } from "../../../types/report";
import { formatMonthName, formatMonthYear, formatPercentValue } from "./format";

const CARD_CLASS = "border-border/60 bg-card/40 rounded-2xl border p-5 backdrop-blur-xl";
const LABEL_CLASS =
  "text-muted-foreground/70 text-[10px] font-semibold uppercase tracking-[0.12em]";

export interface WhatChangedStageProps {
  range: ReportsRange;
  currentReport: MonthlyReport | undefined;
  priorReport: MonthlyReport | undefined;
  months: MonthBucket[];
  taxonomyCategories: TaxonomyCategory[];
  currency: string;
  isLoading: boolean;
  onCategoryClick?: (categoryId: string) => void;
}

export function WhatChangedStage({
  range,
  currentReport,
  priorReport,
  months,
  taxonomyCategories,
  currency,
  isLoading,
  onCategoryClick,
}: WhatChangedStageProps) {
  return (
    <div className="flex flex-col gap-6">
      <HeadlineCard
        range={range}
        currentReport={currentReport}
        priorReport={priorReport}
        taxonomyCategories={taxonomyCategories}
        currency={currency}
        isLoading={isLoading}
      />
      <CategoryTrendsCard
        months={months}
        currentReport={currentReport}
        priorReport={priorReport}
        taxonomyCategories={taxonomyCategories}
        currency={currency}
        useDaily={range.days <= 35}
        isLoading={isLoading}
        onCategoryClick={onCategoryClick}
      />
      <ComparisonTable
        range={range}
        currentReport={currentReport}
        priorReport={priorReport}
        taxonomyCategories={taxonomyCategories}
        currency={currency}
        isLoading={isLoading}
        onCategoryClick={onCategoryClick}
      />
    </div>
  );
}

// ═════════════════════════════════════════════════════════════════════════
// Headline card — narrative summary + 4-stat row
// ═════════════════════════════════════════════════════════════════════════

interface HeadlineCardProps {
  range: ReportsRange;
  currentReport: MonthlyReport | undefined;
  priorReport: MonthlyReport | undefined;
  taxonomyCategories: TaxonomyCategory[];
  currency: string;
  isLoading: boolean;
}

const HeadlineCard: FC<HeadlineCardProps> = ({
  range,
  currentReport,
  priorReport,
  taxonomyCategories,
  currency,
  isLoading,
}) => {
  const current = currentReport?.current.outflow ?? 0;
  const prior = priorReport?.current.outflow ?? 0;
  const change = current - prior;
  const pct = prior > 0 ? (change / prior) * 100 : null;

  const movers = useMemo(
    () =>
      computeTopMovers(
        currentReport?.spendingBreakdown ?? [],
        priorReport?.spendingBreakdown ?? [],
        taxonomyCategories,
      ),
    [currentReport, priorReport, taxonomyCategories],
  );

  const labelPair = useMemo(() => buildPeriodLabels(range), [range]);

  if (isLoading) {
    return (
      <div className={CARD_CLASS}>
        <Skeleton className="h-3 w-32" />
        <Skeleton className="mt-3 h-16 w-full" />
        <Skeleton className="mt-6 h-10 w-1/2" />
      </div>
    );
  }

  return (
    <div className={CARD_CLASS}>
      <div className={LABEL_CLASS}>HEADLINE · {labelPair.combined}</div>
      <p className="text-foreground mt-3 max-w-[95%] text-base font-normal leading-snug tracking-tight md:text-lg">
        {buildHeadline({ change, pct, movers, currency, priorLabel: labelPair.prior })}
      </p>

      <div className="border-border/40 mt-5 grid grid-cols-2 gap-4 border-t pt-4 sm:grid-cols-4">
        <Stat label="THIS PERIOD" value={formatAmount(current, currency)} />
        <Stat label="PRIOR PERIOD" value={formatAmount(prior, currency)} muted />
        <Stat
          label="CHANGE"
          value={`${change >= 0 ? "+" : "−"}${formatAmount(Math.abs(change), currency)}`}
        />
        <Stat
          label="Δ %"
          value={pct == null ? "—" : formatPercentValue(pct, { digits: 0, signDisplay: "always" })}
          tone={pct == null ? "neutral" : pct >= 0 ? "warn" : "good"}
        />
      </div>
    </div>
  );
};

function Stat({
  label,
  value,
  muted,
  tone = "neutral",
}: {
  label: string;
  value: string;
  muted?: boolean;
  tone?: "neutral" | "warn" | "good" | "bad";
}) {
  const toneClass =
    tone === "warn"
      ? "text-amber-700 dark:text-amber-400"
      : tone === "good"
        ? "text-success"
        : tone === "bad"
          ? "text-destructive"
          : muted
            ? "text-muted-foreground/80"
            : "text-foreground";
  return (
    <div>
      <div className={LABEL_CLASS}>{label}</div>
      <div className={cn("mt-1.5 text-lg font-semibold tabular-nums tracking-tight", toneClass)}>
        {value}
      </div>
    </div>
  );
}

interface MoverRow {
  id: string;
  name: string;
  color: string | null;
  icon: string | null;
  current: number;
  prior: number;
  delta: number;
  pct: number | null;
}

function computeTopMovers(
  current: CategoryBreakdownRow[],
  prior: CategoryBreakdownRow[],
  taxonomyCategories: TaxonomyCategory[],
): MoverRow[] {
  const meta = new Map(taxonomyCategories.map((c) => [c.id, c]));
  const roll = (rows: CategoryBreakdownRow[]) => {
    const m = new Map<string, number>();
    for (const r of rows) {
      const c = meta.get(r.categoryId);
      const top = c?.parentId ?? r.categoryId;
      m.set(top, (m.get(top) ?? 0) + r.amount);
    }
    return m;
  };
  const cur = roll(current);
  const pri = roll(prior);
  const all = new Set<string>([...cur.keys(), ...pri.keys()]);
  const rows: MoverRow[] = [];
  for (const id of all) {
    const c = cur.get(id) ?? 0;
    const p = pri.get(id) ?? 0;
    const delta = c - p;
    const pct = p > 0 ? (delta / p) * 100 : null;
    const m = meta.get(id);
    rows.push({
      id,
      name: m?.name ?? id,
      color: m?.color ?? null,
      icon: m?.icon ?? null,
      current: c,
      prior: p,
      delta,
      pct,
    });
  }
  return rows.sort((a, b) => Math.abs(b.delta) - Math.abs(a.delta));
}

function buildHeadline({
  change,
  pct,
  movers,
  currency,
  priorLabel,
}: {
  change: number;
  pct: number | null;
  movers: MoverRow[];
  currency: string;
  priorLabel: string;
}): ReactNode {
  if (movers.length === 0 || change === 0) {
    return <>No meaningful change vs prior period.</>;
  }
  const direction = change >= 0 ? "more" : "less";
  const directionTone = change >= 0 ? "text-destructive" : "text-success";
  const top = movers.find((m) => m.pct != null && Math.abs(m.pct) >= 10) ?? movers[0];
  const second = movers
    .filter((m) => m.id !== top.id && m.pct != null && Math.abs(m.pct) >= 10)
    .find((m) => Math.sign(m.delta) !== Math.sign(top.delta));

  const topPct = top.pct ?? 0;
  const topTone = topPct >= 0 ? "text-destructive" : "text-success";
  const topVerb = topPct >= 0 ? "up" : "down";

  const totalPctSentence =
    pct == null ? null : (
      <span className={cn("font-medium", pct >= 0 ? "text-destructive" : "text-success")}>
        Total spend {pct >= 0 ? "up" : "down"} {formatPercentValue(Math.abs(pct), { digits: 0 })}.
      </span>
    );

  const lead = (
    <>
      You spent{" "}
      <span className={cn("whitespace-nowrap font-serif font-medium", directionTone)}>
        {formatAmount(Math.abs(change), currency)} {direction}
      </span>{" "}
      than {priorLabel}.
    </>
  );

  const drivers = second ? (
    <>
      Most of the {change >= 0 ? "rise" : "drop"} came from{" "}
      <span className={cn("whitespace-nowrap font-serif font-medium", topTone)}>
        {top.name} {topVerb} {formatPercentValue(Math.abs(topPct), { digits: 0 })}
      </span>
      .{" "}
      <span
        className={cn(
          "whitespace-nowrap font-serif font-medium",
          (second.pct ?? 0) >= 0 ? "text-destructive" : "text-success",
        )}
      >
        {second.name} {(second.pct ?? 0) >= 0 ? "up" : "down"}{" "}
        {formatPercentValue(Math.abs(second.pct ?? 0), { digits: 0 })}
      </span>
      .
    </>
  ) : (
    <>
      Most of the {change >= 0 ? "rise" : "drop"} came from{" "}
      <span className={cn("whitespace-nowrap font-serif font-medium", topTone)}>
        {top.name} {topVerb} {formatPercentValue(Math.abs(topPct), { digits: 0 })}
      </span>
      .
    </>
  );

  return (
    <>
      <span>{lead}</span>
      <br />
      <span>{drivers}</span>
      {totalPctSentence && <> {totalPctSentence}</>}
    </>
  );
}

function buildPeriodLabels(range: ReportsRange): {
  current: string;
  prior: string;
  combined: string;
} {
  const priorEnd = new Date(range.start.getTime() - 1);

  // Single-month windows label by month name; multi-month windows by range bounds.
  if (range.months <= 1) {
    const current = formatMonthName(range.end);
    const prior = formatMonthName(priorEnd);
    return { current, prior, combined: `${current.toUpperCase()} VS ${prior.toUpperCase()}` };
  }
  const priorStart = new Date(priorEnd.getTime() - (range.end.getTime() - range.start.getTime()));
  const current = `${formatMonthYear(range.start)} – ${formatMonthYear(range.end)}`;
  const prior = `${formatMonthYear(priorStart)} – ${formatMonthYear(priorEnd)}`;
  return { current, prior, combined: `${range.months}M VS PRIOR ${range.months}M` };
}

// ═════════════════════════════════════════════════════════════════════════
// Category trends card — sparkline grid (2-col), summary line, "show more"
// ═════════════════════════════════════════════════════════════════════════

interface CategoryTrendsCardProps {
  months: MonthBucket[];
  currentReport: MonthlyReport | undefined;
  priorReport: MonthlyReport | undefined;
  taxonomyCategories: TaxonomyCategory[];
  currency: string;
  useDaily: boolean;
  isLoading: boolean;
  onCategoryClick?: (categoryId: string) => void;
}

const CategoryTrendsCard: FC<CategoryTrendsCardProps> = ({
  months,
  currentReport,
  priorReport,
  taxonomyCategories,
  currency,
  useDaily,
  isLoading,
  onCategoryClick,
}) => {
  const rows = useMemo(
    () =>
      buildSparklineRows({
        months,
        byDayByCategory: currentReport?.byDayByCategory,
        priorBreakdown: priorReport?.spendingBreakdown ?? [],
        taxonomyCategories,
        useDaily,
      }),
    [months, currentReport, priorReport, taxonomyCategories, useDaily],
  );

  const [showAll, setShowAll] = useState(false);
  const visible = showAll ? rows : rows.slice(0, 6);
  const remaining = rows.length - visible.length;

  const movedMessage = useMemo(() => {
    const moved = rows.filter((r) => r.deltaPct != null && Math.abs(r.deltaPct) >= 15).slice(0, 2);
    if (moved.length === 0) return null;
    return moved;
  }, [rows]);

  if (isLoading && rows.length === 0) {
    return (
      <div className={CARD_CLASS}>
        <Skeleton className="h-4 w-32" />
        <div className="mt-4 grid gap-3 sm:grid-cols-2">
          {Array.from({ length: 6 }).map((_, i) => (
            <Skeleton key={i} className="h-20 rounded-xl" />
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className={CARD_CLASS}>
      <header className="mb-4">
        <h3 className="text-foreground text-base font-semibold tracking-tight">Category trends</h3>
        <p className="text-muted-foreground text-xs">Sorted by change vs prior period.</p>
        {movedMessage && (
          <p className="text-foreground/90 mt-2 text-sm">
            <span className="font-semibold">
              {movedMessage.length === 1
                ? "One category moved"
                : `${movedMessage.length} categories moved`}{" "}
              this period:
            </span>{" "}
            {movedMessage.map((m, i) => (
              <span key={m.id}>
                {i > 0 && <span className="text-muted-foreground/60"> · </span>}
                {m.name}{" "}
                <span
                  className={cn(
                    "font-semibold",
                    (m.deltaPct ?? 0) >= 0 ? "text-destructive" : "text-success",
                  )}
                >
                  {(m.deltaPct ?? 0) >= 0 ? "↑" : "↓"}{" "}
                  {formatPercentValue(Math.abs(m.deltaPct ?? 0), { digits: 0 })}
                </span>
              </span>
            ))}
          </p>
        )}
      </header>

      {rows.length === 0 ? (
        <div className="text-muted-foreground py-6 text-center text-sm">
          No category history yet for this window.
        </div>
      ) : (
        <>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            {visible.map((row) => (
              <SparklineRow
                key={row.id}
                row={row}
                currency={currency}
                onCategoryClick={onCategoryClick}
              />
            ))}
          </div>
          {remaining > 0 && (
            <div className="mt-4 text-center">
              <button
                type="button"
                onClick={() => setShowAll(true)}
                className="text-foreground hover:text-foreground/80 inline-flex items-center gap-1 text-xs font-medium underline-offset-4 hover:underline"
              >
                Show {remaining} more →
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
};

interface SparkRow {
  id: string;
  name: string;
  color: string | null;
  icon: string | null;
  series: { label: string; value: number }[];
  total: number;
  deltaPct: number | null;
}

function buildSparklineRows({
  months,
  byDayByCategory,
  priorBreakdown,
  taxonomyCategories,
  useDaily,
}: {
  months: MonthBucket[];
  byDayByCategory: DayCategoryBucket[] | undefined;
  priorBreakdown: CategoryBreakdownRow[];
  taxonomyCategories: TaxonomyCategory[];
  useDaily: boolean;
}): SparkRow[] {
  const meta = new Map(taxonomyCategories.map((c) => [c.id, c]));

  const priorByTop = new Map<string, number>();
  for (const r of priorBreakdown) {
    const c = meta.get(r.categoryId);
    const top = c?.parentId ?? r.categoryId;
    priorByTop.set(top, (priorByTop.get(top) ?? 0) + r.amount);
  }

  type Bucket = { name: string; color: string | null; icon: string | null; perBucket: number[] };
  const byCat = new Map<string, Bucket>();

  if (useDaily && byDayByCategory) {
    const days = Array.from(new Set(byDayByCategory.map((b) => b.date))).sort();
    const dayIndex = new Map(days.map((d, i) => [d, i] as const));
    for (const b of byDayByCategory) {
      if (b.taxonomyId !== "spending_categories") continue;
      const c = meta.get(b.categoryId);
      const top = c?.parentId ?? b.categoryId;
      const tcat = meta.get(top) ?? c;
      if (!tcat) continue;
      const idx = dayIndex.get(b.date);
      if (idx == null) continue;
      const e = byCat.get(top) ?? {
        name: tcat.name,
        color: tcat.color ?? null,
        icon: tcat.icon ?? null,
        perBucket: new Array(days.length).fill(0),
      };
      e.perBucket[idx] += b.amount;
      byCat.set(top, e);
    }
  } else {
    months.forEach((m, idx) => {
      for (const r of m.report?.spendingBreakdown ?? []) {
        const c = meta.get(r.categoryId);
        const top = c?.parentId ?? r.categoryId;
        const tcat = meta.get(top) ?? c;
        if (!tcat) continue;
        const e = byCat.get(top) ?? {
          name: tcat.name,
          color: tcat.color ?? null,
          icon: tcat.icon ?? null,
          perBucket: new Array(months.length).fill(0),
        };
        e.perBucket[idx] += r.amount;
        byCat.set(top, e);
      }
    });
  }

  const rows: SparkRow[] = [];
  for (const [id, e] of byCat) {
    const total = e.perBucket.reduce((s, x) => s + x, 0);
    if (total <= 0) continue;
    const series = e.perBucket.map((value, i) => ({ label: String(i), value }));
    const prior = priorByTop.get(id) ?? 0;
    const deltaPct = prior > 0 ? ((total - prior) / prior) * 100 : null;
    rows.push({ id, name: e.name, color: e.color, icon: e.icon, series, total, deltaPct });
  }

  // Sort by absolute delta vs prior — biggest movers first.
  return rows.sort((a, b) => {
    const da = a.deltaPct == null ? -Infinity : Math.abs(a.deltaPct);
    const db = b.deltaPct == null ? -Infinity : Math.abs(b.deltaPct);
    return db - da;
  });
}

function SparklineRow({
  row,
  currency,
  onCategoryClick,
}: {
  row: SparkRow;
  currency: string;
  onCategoryClick?: (categoryId: string) => void;
}) {
  const color = row.color ?? "var(--muted-foreground)";
  const tintBg = row.color ? `${row.color}14` : "var(--muted)";
  const gradId = `wc-spark-${row.id.replace(/[^a-z0-9]/gi, "_")}`;
  const noChange = row.deltaPct == null || Math.abs(row.deltaPct) < 1;
  const clickable = !!onCategoryClick;
  return (
    <div
      role={clickable ? "button" : undefined}
      tabIndex={clickable ? 0 : undefined}
      onClick={clickable ? () => onCategoryClick?.(row.id) : undefined}
      onKeyDown={
        clickable
          ? (e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onCategoryClick?.(row.id);
              }
            }
          : undefined
      }
      className={cn(
        "border-border/60 bg-card/50 flex flex-col gap-1.5 rounded-xl border px-4 pb-4 pt-3",
        clickable && "hover:border-border/90 hover:bg-card/70 cursor-pointer transition-colors",
      )}
      style={{ backgroundImage: `linear-gradient(to bottom, ${tintBg}, transparent 70%)` }}
    >
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2">
          <span className="block h-1.5 w-1.5 rounded-full" style={{ backgroundColor: color }} />
          <CategoryIcon
            icon={row.icon}
            fallback={row.name}
            className="text-foreground/70 h-3.5 w-3.5"
          />
          <span className="text-foreground truncate text-sm font-medium">{row.name}</span>
        </div>
        {!noChange && (
          <span
            className={cn(
              "rounded-full px-2 py-0.5 text-[11px] font-medium tabular-nums",
              (row.deltaPct ?? 0) >= 0
                ? "bg-destructive/10 text-destructive"
                : "bg-success/15 text-success",
            )}
          >
            {(row.deltaPct ?? 0) >= 0 ? "↑" : "↓"}{" "}
            {formatPercentValue(Math.abs(row.deltaPct ?? 0), { digits: 0 })}
          </span>
        )}
        {noChange && (
          <span className="text-muted-foreground/70 rounded-full px-2 py-0.5 text-[11px]">
            no change
          </span>
        )}
      </div>
      <div className="-mx-1 mt-1 h-9">
        <ResponsiveContainer width="100%" height="100%">
          <AreaChart data={row.series} margin={{ top: 2, right: 0, left: 0, bottom: 0 }}>
            <defs>
              <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor={color} stopOpacity={0.45} />
                <stop offset="100%" stopColor={color} stopOpacity={0} />
              </linearGradient>
            </defs>
            <Area
              type="monotone"
              dataKey="value"
              stroke={color}
              strokeWidth={1.5}
              fill={`url(#${gradId})`}
              isAnimationActive={false}
            />
          </AreaChart>
        </ResponsiveContainer>
      </div>
      <div className="text-muted-foreground/80 text-xs tabular-nums">
        {formatCompactAmount(row.total, currency)}
      </div>
    </div>
  );
}

// ═════════════════════════════════════════════════════════════════════════
// Comparison table — Category | This | Prior | Δ$ | direction bar | Δ%
// ═════════════════════════════════════════════════════════════════════════

interface ComparisonTableProps {
  range: ReportsRange;
  currentReport: MonthlyReport | undefined;
  priorReport: MonthlyReport | undefined;
  taxonomyCategories: TaxonomyCategory[];
  currency: string;
  isLoading: boolean;
  onCategoryClick?: (categoryId: string) => void;
}

function ComparisonTable({
  range,
  currentReport,
  priorReport,
  taxonomyCategories,
  currency,
  isLoading,
  onCategoryClick,
}: ComparisonTableProps) {
  const movers = useMemo(
    () =>
      computeTopMovers(
        currentReport?.spendingBreakdown ?? [],
        priorReport?.spendingBreakdown ?? [],
        taxonomyCategories,
      ),
    [currentReport, priorReport, taxonomyCategories],
  );

  const labels = useMemo(() => buildPeriodLabels(range), [range]);
  const maxDelta = movers.reduce((m, r) => Math.max(m, Math.abs(r.delta)), 1);

  if (isLoading && movers.length === 0) {
    return (
      <div className={CARD_CLASS}>
        <Skeleton className="h-3 w-32" />
        <div className="mt-3 space-y-2">
          {Array.from({ length: 5 }).map((_, i) => (
            <Skeleton key={i} className="h-8 w-full" />
          ))}
        </div>
      </div>
    );
  }

  if (movers.length === 0) {
    return (
      <div className={CARD_CLASS}>
        <div className="text-muted-foreground py-6 text-center text-sm">
          No data to compare across the two periods.
        </div>
      </div>
    );
  }

  return (
    <div className="border-border/60 bg-card/40 overflow-hidden rounded-2xl border backdrop-blur-xl">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-border/40 text-muted-foreground/70 border-b text-[10px] font-semibold uppercase tracking-[0.12em]">
            <th className="px-4 py-3 text-left">Category</th>
            <th className="px-3 py-3 text-right">{labels.current}</th>
            <th className="px-3 py-3 text-right">{labels.prior}</th>
            <th className="px-3 py-3 text-right">Δ $</th>
            <th className="px-3 py-3 text-left">Direction</th>
            <th className="px-4 py-3 text-right">Δ %</th>
          </tr>
        </thead>
        <tbody>
          {movers.map((row) => (
            <ComparisonRow
              key={row.id}
              row={row}
              currency={currency}
              maxDelta={maxDelta}
              onCategoryClick={onCategoryClick}
            />
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ComparisonRow({
  row,
  currency,
  maxDelta,
  onCategoryClick,
}: {
  row: MoverRow;
  currency: string;
  maxDelta: number;
  onCategoryClick?: (categoryId: string) => void;
}) {
  const color = row.color ?? "var(--muted-foreground)";
  const isUp = row.delta > 0;
  const widthPct = (Math.abs(row.delta) / maxDelta) * 50; // each side max 50% of bar
  const clickable = !!onCategoryClick;
  return (
    <tr
      className={cn(
        "border-border/30 hover:bg-muted/20 border-b last:border-b-0",
        clickable && "cursor-pointer",
      )}
      onClick={clickable ? () => onCategoryClick?.(row.id) : undefined}
    >
      <td className="px-4 py-2.5">
        <div className="flex items-center gap-2">
          <span
            className="block h-1.5 w-1.5 shrink-0 rounded-full"
            style={{ backgroundColor: color }}
          />
          <span className="text-foreground text-sm">{row.name}</span>
        </div>
      </td>
      <td className="text-foreground/90 px-3 py-2.5 text-right text-xs tabular-nums">
        {formatAmount(row.current, currency)}
      </td>
      <td className="text-muted-foreground/80 px-3 py-2.5 text-right text-xs tabular-nums">
        {formatAmount(row.prior, currency)}
      </td>
      <td
        className={cn(
          "px-3 py-2.5 text-right text-xs font-medium tabular-nums",
          row.delta === 0 ? "text-muted-foreground/70" : isUp ? "text-destructive" : "text-success",
        )}
      >
        {row.delta === 0
          ? "—"
          : `${isUp ? "+" : "−"}${formatAmount(Math.abs(row.delta), currency)}`}
      </td>
      <td className="px-3 py-2.5">
        <div className="relative h-2 w-full">
          <div className="bg-foreground/10 absolute inset-y-1/2 left-1/2 h-px w-px" />
          <div
            className={cn(
              "absolute inset-y-0 rounded-sm",
              isUp ? "bg-destructive/70 left-1/2" : "bg-success/70 right-1/2",
            )}
            style={{ width: `${widthPct}%` }}
          />
        </div>
      </td>
      <td
        className={cn(
          "px-4 py-2.5 text-right text-xs font-medium tabular-nums",
          row.pct == null
            ? "text-muted-foreground/70"
            : row.pct >= 0
              ? "text-destructive"
              : "text-success",
        )}
      >
        {row.pct == null ? "—" : formatPercentValue(row.pct, { digits: 0, signDisplay: "always" })}
      </td>
    </tr>
  );
}
