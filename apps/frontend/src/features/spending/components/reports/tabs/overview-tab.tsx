import { useMemo } from "react";

import { useTaxonomy } from "@/hooks/use-taxonomies";

import { useBudget } from "../../../hooks/use-budget";
import { useCashActivities } from "../../../hooks/use-cash-activities";
import { useMonthlyHistory } from "../../../hooks/use-monthly-history";
import { useSpendingReport } from "../../../hooks/use-spending-report";
import {
  comparisonRange,
  rangeToReportRequest,
  type ComparisonMode,
  type ReportsRange,
} from "../../../lib/reports-period";

import { CashflowDivergingBars } from "../cashflow-diverging-bars";
import { BudgetStatusHero, CashflowHero, PeriodSummaryHero } from "../hero-strip";
import { NotableChangesCard } from "../notable-changes-card";
import { SpendingRhythmHeatmap } from "../spending-rhythm-heatmap";

const SPENDING_TAXONOMY = "spending_categories";

interface OverviewTabProps {
  range: ReportsRange;
  comparison: ComparisonMode;
  currency: string;
}

const FOREST_DEEP = "#2A573F";

/**
 * Overview tab — period KPIs + cashflow + behavioral patterns in one layout.
 *
 * Top: comparison KPIs strip (income / spending / net / savings rate).
 * Middle: full-width cashflow stacked area.
 * Bottom: 2-column grid with spending rhythm heatmap (12wk × 7d) and
 * day-of-week distribution chart.
 *
 * The bottom row consolidates what was the separate "Patterns" tab into
 * scannable side-widgets — pattern info doesn't need a full canvas.
 */
export function OverviewTab({ range, comparison, currency }: OverviewTabProps) {
  const { months, isLoading: isHistoryLoading } = useMonthlyHistory(range);
  const { data: budget, isLoading: isBudgetLoading } = useBudget();
  const taxonomy = useTaxonomy(SPENDING_TAXONOMY);

  const currentRequest = useMemo(() => rangeToReportRequest(range), [range]);
  const { data: currentReport, isLoading: isCurrentLoading } = useSpendingReport(currentRequest);

  const priorRange = useMemo(() => comparisonRange(range, comparison), [range, comparison]);
  const priorRequest = useMemo(
    () => (priorRange ? rangeToReportRequest(priorRange) : null),
    [priorRange],
  );
  const { data: priorReport, isLoading: isPriorLoading } = useSpendingReport(
    priorRequest ?? { startDate: "", endDate: "" },
    !!priorRequest,
  );

  // Heatmap window — last 4 weeks (matches the original V1 dashboard rhythm widget).
  const heatmapRequest = useMemo(() => {
    const end = new Date();
    end.setHours(23, 59, 59, 999);
    const start = new Date(end);
    start.setDate(end.getDate() - 4 * 7);
    start.setHours(0, 0, 0, 0);
    return { startDate: start.toISOString(), endDate: end.toISOString() };
  }, []);
  const { data: heatmapActivities = [] } = useCashActivities(heatmapRequest);

  return (
    <div className="space-y-6">
      {/* Hero strip — at-a-glance period status (3-card layout). */}
      <div className="grid gap-3 lg:grid-cols-3">
        <PeriodSummaryHero
          spent={currentReport?.current.outflow ?? 0}
          days={range.days}
          months={range.months}
          breakdown={currentReport?.spendingBreakdown ?? []}
          taxonomyCategories={taxonomy.data?.categories ?? []}
          currency={currency}
          isLoading={isCurrentLoading}
        />
        <BudgetStatusHero
          spent={currentReport?.current.outflow ?? 0}
          monthsInRange={range.months}
          budget={budget}
          currency={currency}
          isLoading={isCurrentLoading || isBudgetLoading}
        />
        <CashflowHero months={months} currency={currency} isLoading={isHistoryLoading} />
      </div>

      <Section title="Cashflow over time" subtitle="Income above · spending below · net line">
        <CashflowDivergingBars months={months} currency={currency} isLoading={isHistoryLoading} />
      </Section>

      <div className="grid gap-6 lg:grid-cols-2">
        <Section title="Spending rhythm" subtitle="Last 4 weeks · darker = heavier">
          <SpendingRhythmHeatmap
            activities={heatmapActivities}
            weeks={4}
            accent={FOREST_DEEP}
            currency={currency}
          />
        </Section>

        <Section
          title="Notable changes"
          subtitle={
            comparison === "none" ? "Enable a comparison to surface" : "vs comparison period"
          }
        >
          <NotableChangesCard
            current={currentReport}
            prior={comparison === "none" ? undefined : priorReport}
            taxonomyCategories={taxonomy.data?.categories ?? []}
            currency={currency}
            isLoading={isCurrentLoading || (comparison !== "none" && isPriorLoading)}
          />
        </Section>
      </div>
    </div>
  );
}

function Section({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle?: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <header className="mb-2 flex items-baseline justify-between">
        <h2 className="text-md font-semibold tracking-tight">{title}</h2>
        {subtitle && <span className="text-muted-foreground/70 text-xs">{subtitle}</span>}
      </header>
      <div className="border-border bg-card shadow-xs rounded-xl border p-4 md:p-5">{children}</div>
    </section>
  );
}
