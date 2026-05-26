import { formatPercent } from "@/lib/utils";
import { CompactAmount } from "./compact-amount";
import { SectionCard } from "./section-card";
import { Sparkline } from "./sparkline";
import {
  CARD_LABEL,
  CATEGORY_COLORS,
  seriesFor,
  type ParsedHistoryPoint,
  type ParsedNetWorth,
} from "./utils";

// name | % | value | Δ | trend — % and trend collapse on small screens.
const ROW_GRID =
  "grid grid-cols-[minmax(0,1fr)_auto_auto] md:grid-cols-[minmax(0,1fr)_3rem_7rem_8rem_4.5rem] items-center gap-x-3 md:gap-x-4";

interface Change {
  amount: number;
  percent: number;
}

/**
 * Change over the range from a value series. Liabilities are stored as positive
 * magnitudes, so a reduction is expressed as a positive (good) change.
 */
function deriveChange(series: number[], isLiability: boolean): Change {
  if (series.length < 2) return { amount: 0, percent: 0 };
  const first = series[0];
  const last = series[series.length - 1];
  const amount = isLiability ? first - last : last - first;
  const base = Math.abs(first);
  return { amount, percent: base > 0 ? amount / base : 0 };
}

// Percent is a ratio (0.145 = 14.5%); drop decimals for very large swings.
function formatChangePercent(percent: number): string {
  const abs = Math.abs(percent);
  return formatPercent(abs, { digits: abs >= 10 ? 0 : 1, signDisplay: "never" });
}

function ChangeCell({ change, currency }: { change: Change; currency: string }) {
  const isZero = Math.abs(change.amount) < 0.005;
  const color = isZero
    ? "text-muted-foreground/60"
    : change.amount > 0
      ? "text-success"
      : "text-destructive";
  const sign = isZero ? "" : change.amount > 0 ? "+" : "-";
  return (
    <div className={`flex items-baseline justify-end gap-1.5 ${color}`}>
      <span className="text-sm tabular-nums">
        {sign}
        <CompactAmount value={Math.abs(change.amount)} currency={currency} />
      </span>
      <span className="text-xs tabular-nums opacity-80">
        {sign}
        {formatChangePercent(change.percent)}
      </span>
    </div>
  );
}

function trendColor(amount: number): string {
  if (Math.abs(amount) < 0.005) return "var(--muted-foreground)";
  return amount > 0 ? "var(--success)" : "var(--destructive)";
}

interface RowProps {
  name: string;
  dotClass: string;
  value: number;
  percentOfSection: number;
  series: number[];
  change: Change;
  currency: string;
  negative?: boolean;
}

function BreakdownRow({
  name,
  dotClass,
  value,
  percentOfSection,
  series,
  change,
  currency,
  negative,
}: RowProps) {
  return (
    <div className={`${ROW_GRID} py-2`}>
      <div className="flex min-w-0 items-center gap-2.5">
        <div className={`h-2 w-2 shrink-0 rounded-full ${dotClass}`} />
        <span className="text-foreground/90 truncate text-sm">{name}</span>
      </div>
      <span className="text-muted-foreground/70 hidden text-right text-xs tabular-nums md:block">
        {percentOfSection.toFixed(1)}%
      </span>
      <span className="text-foreground/90 justify-self-end text-sm tabular-nums">
        {negative && value !== 0 ? "-" : ""}
        <CompactAmount value={value} currency={currency} />
      </span>
      <ChangeCell change={change} currency={currency} />
      <div className="hidden justify-self-end md:block">
        <Sparkline
          data={series}
          stroke={trendColor(change.amount)}
          fill={trendColor(change.amount)}
        />
      </div>
    </div>
  );
}

interface BreakdownTableProps {
  data: ParsedNetWorth;
  history: ParsedHistoryPoint[];
  currency: string;
  periodLabel: string;
}

export function BreakdownTable({ data, history, currency, periodLabel }: BreakdownTableProps) {
  const hasLiabilities = data.liabilities.total > 0 || data.liabilities.breakdown.length > 0;
  const netWorthSeries = history.map((point) => point.netWorth);
  const netWorthChange = deriveChange(netWorthSeries, false);

  return (
    <SectionCard title="Breakdown" meta={`Trend = change over ${periodLabel}`}>
      {/* Assets section header + total */}
      <div className="border-border/60 flex items-center justify-between border-b pb-2">
        <span className="text-sm font-semibold">Assets</span>
        <span className="text-success text-sm font-semibold tabular-nums">
          <CompactAmount value={data.assets.total} currency={currency} />
        </span>
      </div>

      {/* Column labels */}
      <div className={`${ROW_GRID} pt-2`}>
        <span className={CARD_LABEL}>Category</span>
        <span className={`${CARD_LABEL} hidden text-right md:block`}>%</span>
        <span className={`${CARD_LABEL} justify-self-end`}>Value</span>
        <span className={`${CARD_LABEL} justify-self-end`}>Δ {periodLabel}</span>
        <span className={`${CARD_LABEL} hidden justify-self-end md:block`}>Trend</span>
      </div>

      <div className="divide-border/40 divide-y">
        {data.assets.breakdown.map((item) => (
          <BreakdownRow
            key={item.category}
            name={item.name}
            dotClass={CATEGORY_COLORS[item.category] ?? "bg-muted-foreground"}
            value={item.value}
            percentOfSection={data.assets.total > 0 ? (item.value / data.assets.total) * 100 : 0}
            series={seriesFor(history, item.category)}
            change={deriveChange(seriesFor(history, item.category), false)}
            currency={currency}
          />
        ))}
      </div>

      {/* Liabilities section */}
      {hasLiabilities && (
        <>
          <div className="border-border/60 mt-2 flex items-center justify-between border-y py-2">
            <span className="text-sm font-semibold">Liabilities</span>
            <span className="text-destructive text-sm font-semibold tabular-nums">
              -<CompactAmount value={data.liabilities.total} currency={currency} />
            </span>
          </div>
          <div className="divide-border/40 divide-y">
            {data.liabilities.breakdown.map((item, index) => {
              const key = item.assetId ?? `${item.category}-${index}`;
              const series = item.assetId ? seriesFor(history, item.assetId) : [];
              return (
                <BreakdownRow
                  key={key}
                  name={item.name}
                  dotClass={CATEGORY_COLORS.liabilities}
                  value={item.value}
                  negative
                  percentOfSection={
                    data.liabilities.total > 0 ? (item.value / data.liabilities.total) * 100 : 0
                  }
                  series={series}
                  change={deriveChange(series, true)}
                  currency={currency}
                />
              );
            })}
          </div>
        </>
      )}

      {/* Net Worth total */}
      <div className={`${ROW_GRID} bg-muted/30 -mx-4 mt-2 rounded-lg px-4 py-3 md:-mx-5 md:px-5`}>
        <span className="text-sm font-bold">Net Worth</span>
        <span className="hidden md:block" />
        <span className="justify-self-end text-sm font-bold tabular-nums">
          <CompactAmount value={data.netWorth} currency={currency} />
        </span>
        <ChangeCell change={netWorthChange} currency={currency} />
        <div className="hidden justify-self-end md:block">
          <Sparkline
            data={netWorthSeries}
            stroke={trendColor(netWorthChange.amount)}
            fill={trendColor(netWorthChange.amount)}
          />
        </div>
      </div>
    </SectionCard>
  );
}
