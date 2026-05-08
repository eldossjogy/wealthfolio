import { useMemo } from "react";

import { useTaxonomy } from "@/hooks/use-taxonomies";

import { useBudget } from "../../../hooks/use-budget";
import { useMonthlyHistory } from "../../../hooks/use-monthly-history";
import { useSpendingReport } from "../../../hooks/use-spending-report";
import {
  comparisonRange,
  rangeToReportRequest,
  type ComparisonMode,
  type ReportsRange,
} from "../../../lib/reports-period";

import { CategoryHierarchyTable } from "../category-hierarchy-table";
import { CategorySparklineGrid } from "../category-sparkline-grid";

const SPENDING_TAXONOMY = "spending_categories";

interface CategoriesTabProps {
  range: ReportsRange;
  comparison: ComparisonMode;
  currency: string;
}

export function CategoriesTab({ range, comparison, currency }: CategoriesTabProps) {
  const taxonomy = useTaxonomy(SPENDING_TAXONOMY);
  const { data: budget } = useBudget();

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

  // Adaptive granularity: short windows render daily sparklines from the
  // current report's per-day-per-category data; longer windows aggregate
  // monthly via parallel monthly reports.
  const useDaily = range.days <= 35;
  const { months, isLoading: isHistoryLoading } = useMonthlyHistory(range);

  return (
    <div className="space-y-6">
      <Section title="Category trends" subtitle="Sparkline per top-level category">
        <CategorySparklineGrid
          taxonomyCategories={taxonomy.data?.categories ?? []}
          currency={currency}
          isLoading={useDaily ? isCurrentLoading : isHistoryLoading}
          priorBreakdown={priorReport?.spendingBreakdown ?? []}
          granularity={useDaily ? "day" : "month"}
          months={useDaily ? undefined : months}
          byDayByCategory={useDaily ? (currentReport?.byDayByCategory ?? []) : undefined}
        />
      </Section>

      <Section title="Breakdown" subtitle="Spent, budget progress, change vs prior period">
        <CategoryHierarchyTable
          breakdown={currentReport?.spendingBreakdown ?? []}
          priorBreakdown={priorReport?.spendingBreakdown ?? []}
          allocations={budget?.allocations ?? []}
          taxonomyCategories={taxonomy.data?.categories ?? []}
          currency={currency}
          isLoading={isCurrentLoading || (comparison !== "none" && isPriorLoading)}
        />
      </Section>
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
      <div className="border-border/60 bg-card/40 rounded-xl border p-4 backdrop-blur-xl md:p-5">
        {children}
      </div>
    </section>
  );
}
