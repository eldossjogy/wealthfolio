import { cn } from "@/lib/utils";
import type { AccountScope, DriftReport } from "@/lib/types";
import { CurrentVsTargetCard } from "./current-vs-target-card";
import { DriftDriversCard } from "./drift-drivers-card";
import { HoldingsTable } from "./holdings-table";

interface OverviewTabProps {
  report: DriftReport;
  driftBandBps: number;
  accountScope: AccountScope;
  onRebalanceClick?: () => void;
}

export function OverviewTab({
  report,
  driftBandBps,
  accountScope,
  onRebalanceClick,
}: OverviewTabProps) {
  const isFine = report.outOfBandCount === 0;
  const bandPct = (driftBandBps / 100).toFixed(1);
  const oobNames = report.rows
    .filter((r) => r.status === "overweight" || r.status === "underweight")
    .map((r) => r.categoryName)
    .join(", ");

  return (
    <div className="space-y-5">
      {/* Status banner */}
      <div
        className={cn(
          "flex items-center justify-between gap-4 rounded-lg border px-5 py-4",
          isFine
            ? "border-green-200 bg-green-50 dark:border-green-800 dark:bg-green-950/20"
            : "border-amber-200 bg-amber-50 dark:border-amber-800 dark:bg-amber-950/20",
        )}
      >
        <div className="flex items-center gap-3">
          <div
            className={cn(
              "h-2.5 w-2.5 shrink-0 rounded-full",
              isFine ? "bg-green-600" : "bg-amber-600",
            )}
          />
          <div>
            <div className="text-foreground text-[13px] font-semibold">
              {isFine
                ? "All sleeves within tolerance"
                : `${report.outOfBandCount} ${report.outOfBandCount === 1 ? "sleeve" : "sleeves"} out of band`}
            </div>
            <div className="text-muted-foreground text-[12px]">
              {isFine
                ? `Drift band ±${bandPct}%`
                : `Drift band ±${bandPct}% · largest breach ${(Math.abs(report.maxDriftBps) / 100).toFixed(2)}%${oobNames ? ` — ${oobNames}` : ""}`}
            </div>
          </div>
        </div>
        {onRebalanceClick && (
          <button
            onClick={onRebalanceClick}
            className="bg-foreground text-background h-8 whitespace-nowrap rounded-md px-3 text-[12px] font-medium hover:opacity-90"
          >
            Plan rebalance →
          </button>
        )}
      </div>

      <div className="grid grid-cols-1 gap-5 md:min-h-[480px] md:grid-cols-[3fr_2fr]">
        <CurrentVsTargetCard report={report} />
        <DriftDriversCard report={report} />
      </div>

      <HoldingsTable report={report} accountScope={accountScope} />
    </div>
  );
}
