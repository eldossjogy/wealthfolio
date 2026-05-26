import { CompactAmount } from "./compact-amount";
import { SectionCard } from "./section-card";
import { CARD_LABEL, type Velocity } from "./utils";

function signClass(value: number): string {
  if (Math.abs(value) < 0.005) return "text-muted-foreground/60";
  return value > 0 ? "text-success" : "text-destructive";
}

function MetricBar({
  label,
  value,
  max,
  currency,
}: {
  label: string;
  value: number;
  max: number;
  currency: string;
}) {
  const width = max > 0 ? Math.min(100, (Math.abs(value) / max) * 100) : 0;
  const positive = value >= 0;
  const sign = Math.abs(value) < 0.005 ? "" : positive ? "+" : "-";
  return (
    <div>
      <p className={CARD_LABEL}>{label}</p>
      <p className={`text-sm font-semibold tabular-nums ${signClass(value)}`}>
        {sign}
        <CompactAmount value={Math.abs(value)} currency={currency} />
      </p>
      <div className="bg-muted/40 relative mt-1.5 h-1.5 w-full overflow-hidden rounded-full">
        <div
          className="absolute inset-y-0 left-0 rounded-full"
          style={{
            width: `${width}%`,
            backgroundColor: positive ? "var(--success)" : "var(--destructive)",
            opacity: 0.9,
          }}
        />
      </div>
    </div>
  );
}

interface VelocityCardProps {
  velocity: Velocity;
  /** Average monthly net worth change over the trailing year, for the multiple. */
  trailingYearMonthly?: number;
  currency: string;
  periodLabel: string;
}

export function VelocityCard({
  velocity,
  trailingYearMonthly,
  currency,
  periodLabel,
}: VelocityCardProps) {
  const { perMonth, marketGains, contributions, equityBuilt } = velocity;
  const max = Math.max(Math.abs(marketGains), Math.abs(contributions), Math.abs(equityBuilt));
  const multiple =
    trailingYearMonthly && Math.abs(trailingYearMonthly) > 0.005
      ? perMonth / trailingYearMonthly
      : null;
  const perMonthSign = Math.abs(perMonth) < 0.005 ? "" : perMonth > 0 ? "+" : "-";

  return (
    <SectionCard title="Velocity" meta={periodLabel}>
      <div className="flex items-baseline gap-1.5">
        <span className={`text-2xl font-bold tabular-nums ${signClass(perMonth)}`}>
          {perMonthSign}
          <CompactAmount value={Math.abs(perMonth)} currency={currency} />
        </span>
        <span className="text-muted-foreground text-sm">/ month</span>
      </div>
      {multiple != null && (
        <p className="text-muted-foreground mt-0.5 text-xs">
          {multiple.toFixed(1)}× trailing-year average
        </p>
      )}

      <div className="mt-4 grid grid-cols-2 gap-x-4 gap-y-3">
        <MetricBar label="Market gains" value={marketGains} max={max} currency={currency} />
        <MetricBar label="Contributions" value={contributions} max={max} currency={currency} />
        <MetricBar label="Equity built" value={equityBuilt} max={max} currency={currency} />
      </div>
    </SectionCard>
  );
}
