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

import { BudgetVsActualBars } from "../budget-vs-actual-bars";
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

 const { months, isLoading: isHistoryLoading } = useMonthlyHistory(range);

 return (
 <div className="space-y-6">
 <Section title="Category trends" subtitle="Sparkline per top-level category">
 <CategorySparklineGrid
 months={months}
 taxonomyCategories={taxonomy.data?.categories ?? []}
 currency={currency}
 isLoading={isHistoryLoading}
 />
 </Section>

 <Section
 title="Budget vs actual"
 subtitle="Sorted by % consumed · click to drill into transactions"
 >
 <BudgetVsActualBars
 breakdown={currentReport?.spendingBreakdown ?? []}
 allocations={budget?.allocations ?? []}
 taxonomyCategories={taxonomy.data?.categories ?? []}
 currency={currency}
 isLoading={isCurrentLoading}
 />
 </Section>

 <Section title="Breakdown" subtitle="Budgeted vs spent · expand a row to drill in">
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
 <div className="border-border bg-card shadow-xs rounded-xl border p-4 md:p-5 ">
 {children}
 </div>
 </section>
 );
}
