import { AnimatedToggleGroup } from "@wealthfolio/ui";

import { REPORTS_PERIODS, type ComparisonMode, type ReportsPeriod } from "../../lib/reports-period";

const PERIOD_LABELS: Record<ReportsPeriod, string> = {
  "1M": "1M",
  "3M": "3M",
  "6M": "6M",
  YTD: "YTD",
  "1Y": "1Y",
};

const COMPARISON_LABELS: Record<ComparisonMode, string> = {
  prior: "Prior",
  yoy: "YoY",
  none: "None",
};

interface ReportsHeaderProps {
  period: ReportsPeriod;
  onPeriodChange: (period: ReportsPeriod) => void;
  comparison: ComparisonMode;
  onComparisonChange: (mode: ComparisonMode) => void;
}

/**
 * Top-of-page header for the Reports view. Owns no state — period and
 * comparison are lifted to the page so every tab observes the same window.
 */
export function ReportsHeader({
  period,
  onPeriodChange,
  comparison,
  onComparisonChange,
}: ReportsHeaderProps) {
  return (
    <div className="flex flex-col gap-3 pb-2 sm:flex-row sm:items-center sm:justify-between">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Reports</h1>
        <p className="text-muted-foreground text-sm">
          Patterns, trends, and deeper analysis of your spending.
        </p>
      </div>
      <div className="flex flex-wrap items-center gap-3">
        <div className="flex flex-col gap-1">
          <span className="text-muted-foreground/70 text-[10px] font-semibold uppercase tracking-wide">
            Period
          </span>
          <AnimatedToggleGroup
            variant="secondary"
            size="xs"
            items={REPORTS_PERIODS.map((p) => ({ value: p, label: PERIOD_LABELS[p] }))}
            value={period}
            onValueChange={onPeriodChange}
          />
        </div>
        <div className="flex flex-col gap-1">
          <span className="text-muted-foreground/70 text-[10px] font-semibold uppercase tracking-wide">
            Compare to
          </span>
          <AnimatedToggleGroup
            variant="secondary"
            size="xs"
            items={(["prior", "yoy", "none"] as ComparisonMode[]).map((m) => ({
              value: m,
              label: COMPARISON_LABELS[m],
            }))}
            value={comparison}
            onValueChange={onComparisonChange}
          />
        </div>
      </div>
    </div>
  );
}
