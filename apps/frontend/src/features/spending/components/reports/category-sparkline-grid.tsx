import { useMemo } from "react";
import { Link } from "react-router-dom";
import { Area, AreaChart, ResponsiveContainer } from "recharts";

import { Skeleton, formatCompactAmount } from "@wealthfolio/ui";
import type { TaxonomyCategory } from "@/lib/types";
import { cn } from "@/lib/utils";

import type { MonthBucket } from "../../hooks/use-monthly-history";

interface CategorySparklineGridProps {
  months: MonthBucket[];
  taxonomyCategories: TaxonomyCategory[];
  currency: string;
  isLoading: boolean;
  /** Limit visible cards. */
  topN?: number;
}

interface CategorySparklineRow {
  id: string;
  name: string;
  color: string | null;
  series: { label: string; value: number }[];
  total: number;
  /** % delta — last month vs first month with non-zero data. */
  deltaPct: number | null;
}

/**
 * Per-category sparkline grid — one card per top-level category.
 *
 * Shows the spending trajectory across the months in scope. Designed for the
 * Categories tab where the user wants to spot which categories are accelerating
 * or decelerating without drilling into a chart per row.
 */
export function CategorySparklineGrid({
  months,
  taxonomyCategories,
  currency,
  isLoading,
  topN = 8,
}: CategorySparklineGridProps) {
  const rows = useMemo(
    () => buildRows(months, taxonomyCategories, topN),
    [months, taxonomyCategories, topN],
  );

  if (isLoading && rows.length === 0) {
    return (
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
        {Array.from({ length: 8 }).map((_, i) => (
          <Skeleton key={i} className="h-24 rounded-lg" />
        ))}
      </div>
    );
  }

  if (rows.length === 0) {
    return (
      <div className="text-muted-foreground py-8 text-center text-sm">
        No category history yet for this window.
      </div>
    );
  }

  return (
    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
      {rows.map((r) => (
        <SparklineCard key={r.id} row={r} currency={currency} />
      ))}
    </div>
  );
}

function SparklineCard({ row, currency }: { row: CategorySparklineRow; currency: string }) {
  const color = row.color ?? "var(--muted-foreground)";
  const gradId = `spark-${row.id.replace(/[^a-z0-9]/gi, "_")}`;

  return (
    <Link
      to={`/spending/transactions?category=${encodeURIComponent(row.id)}`}
      className="border-border/60 bg-card/40 hover:bg-card/60 group flex flex-col gap-1 rounded-lg border px-3 py-2.5 transition-colors"
    >
      <div className="flex items-center justify-between gap-2">
        <span className="text-foreground truncate text-xs font-medium">{row.name}</span>
        {row.deltaPct != null && Math.abs(row.deltaPct) >= 1 && (
          <span
            className={cn(
              "shrink-0 text-[10px] font-semibold tabular-nums",
              row.deltaPct >= 0 ? "text-destructive" : "text-success",
            )}
          >
            {row.deltaPct >= 0 ? "↑" : "↓"} {Math.abs(row.deltaPct).toFixed(0)}%
          </span>
        )}
      </div>
      <div className="text-foreground text-sm font-semibold tabular-nums">
        {formatCompactAmount(row.total, currency)}
      </div>
      <div className="-mx-1 h-10">
        <ResponsiveContainer width="100%" height="100%">
          <AreaChart data={row.series} margin={{ top: 2, right: 0, left: 0, bottom: 0 }}>
            <defs>
              <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor={color} stopOpacity={0.4} />
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
    </Link>
  );
}

function buildRows(
  months: MonthBucket[],
  taxonomyCategories: TaxonomyCategory[],
  topN: number,
): CategorySparklineRow[] {
  const meta = new Map(taxonomyCategories.map((c) => [c.id, c]));

  // Aggregate monthly amounts per top-level category (parent rollup).
  const byCat = new Map<string, { name: string; color: string | null; perMonth: number[] }>();
  months.forEach((m, idx) => {
    for (const r of m.report?.spendingBreakdown ?? []) {
      const c = meta.get(r.categoryId);
      const topId = c?.parentId ?? r.categoryId;
      const top = meta.get(topId) ?? c;
      if (!top) continue;
      const e =
        byCat.get(topId) ??
        ({
          name: top.name,
          color: top.color ?? null,
          perMonth: new Array(months.length).fill(0),
        } as { name: string; color: string | null; perMonth: number[] });
      e.perMonth[idx] += r.amount;
      byCat.set(topId, e);
    }
  });

  const rows: CategorySparklineRow[] = [];
  for (const [id, e] of byCat) {
    const total = e.perMonth.reduce((s, x) => s + x, 0);
    if (total <= 0) continue;
    const series = e.perMonth.map((value, i) => ({ label: months[i].label, value }));
    const firstNonZero = e.perMonth.find((v) => v > 0) ?? 0;
    const last = e.perMonth[e.perMonth.length - 1] ?? 0;
    const deltaPct = firstNonZero > 0 ? ((last - firstNonZero) / firstNonZero) * 100 : null;
    rows.push({ id, name: e.name, color: e.color, series, total, deltaPct });
  }

  return rows.sort((a, b) => b.total - a.total).slice(0, topN);
}
