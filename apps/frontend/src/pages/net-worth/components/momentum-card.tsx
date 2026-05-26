import { CompactAmount } from "./compact-amount";
import { SectionCard } from "./section-card";
import { THEME_COLOR, type Momentum } from "./utils";

function monthLabel(month: string): string {
  // month is "YYYY-MM"
  const date = new Date(`${month}-01T00:00:00`);
  return date.toLocaleDateString("en-US", { month: "short", year: "2-digit" });
}

interface MomentumCardProps {
  momentum: Momentum;
  currency: string;
  periodLabel: string;
}

export function MomentumCard({ momentum, currency, periodLabel }: MomentumCardProps) {
  const { currentChange, beatBy, bars } = momentum;
  const maxBar = Math.max(1, ...bars.map((b) => Math.abs(b.value)));
  const changeSign = Math.abs(currentChange) < 0.005 ? "" : currentChange > 0 ? "+" : "-";
  const changeColor = currentChange >= 0 ? "text-success" : "text-destructive";

  return (
    <SectionCard title="Momentum" meta={beatBy == null ? "all time" : `vs prior ${periodLabel}`}>
      <div className={`text-2xl font-bold tabular-nums ${changeColor}`}>
        {changeSign}
        <CompactAmount value={Math.abs(currentChange)} currency={currency} />
      </div>
      {beatBy != null && (
        <p className="text-muted-foreground mt-0.5 text-xs">
          {beatBy >= 0 ? "Beat prior period by " : "Behind prior period by "}
          <span
            className={
              beatBy >= 0 ? "text-success font-semibold" : "text-destructive font-semibold"
            }
          >
            <CompactAmount value={Math.abs(beatBy)} currency={currency} />
          </span>
        </p>
      )}

      {bars.length > 1 && (
        <>
          <div className="mt-4 flex h-14 items-end gap-1">
            {bars.map((bar) => {
              const height = Math.max(4, (Math.abs(bar.value) / maxBar) * 100);
              const negative = bar.value < 0;
              return (
                <div
                  key={bar.month}
                  className="flex-1 rounded-sm"
                  style={{
                    height: `${height}%`,
                    backgroundColor: bar.current
                      ? THEME_COLOR
                      : "color-mix(in srgb, var(--muted-foreground) 35%, transparent)",
                    opacity: negative ? 0.45 : 1,
                  }}
                  title={`${monthLabel(bar.month)}: ${bar.value >= 0 ? "+" : ""}${bar.value.toFixed(0)}`}
                />
              );
            })}
          </div>
          <div className="text-muted-foreground/60 mt-1.5 flex justify-between text-[10px]">
            <span>{monthLabel(bars[0].month)}</span>
            <span>Now</span>
          </div>
        </>
      )}
    </SectionCard>
  );
}
