import type { DriftReport } from "@/lib/types";
import { useTaxonomy } from "@/hooks/use-taxonomies";
import { CurrentVsTargetCard } from "./current-vs-target-card";
import { DriftDriversCard } from "./drift-drivers-card";
import { HoldingsTable } from "./holdings-table";
import { resolveDriftReportCategories } from "./drift-report-resolver";
import { categoryNoun, taxonomyLabel, targetLabel } from "./drift-copy";

interface OverviewTabProps {
  report: DriftReport;
  taxonomyId: string;
  targetName?: string;
  onRebalanceClick?: () => void;
}

export function OverviewTab({
  report,
  taxonomyId,
  targetName,
  onRebalanceClick,
}: OverviewTabProps) {
  const { data: taxonomy } = useTaxonomy(taxonomyId);
  const resolvedReport = resolveDriftReportCategories(report, taxonomy?.categories);
  const taxonomyName = taxonomy?.taxonomy.name;
  const categoryLabel = categoryNoun(taxonomyId, taxonomyName, report.outOfBandCount);
  const pluralCategoryLabel = categoryNoun(taxonomyId, taxonomyName, 2);
  const displayTaxonomy = taxonomyLabel(taxonomyId, taxonomyName);
  const gapStatus =
    report.outOfBandCount === 0
      ? `All ${pluralCategoryLabel} inside target`
      : `${report.outOfBandCount} ${categoryLabel} outside target`;

  return (
    <div className="space-y-5">
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-[minmax(0,1.7fr)_minmax(340px,0.75fr)]">
        <CurrentVsTargetCard
          report={resolvedReport}
          taxonomyLabel={displayTaxonomy}
          targetLabel={targetLabel(targetName)}
        />
        <DriftDriversCard
          report={resolvedReport}
          statusDescription={gapStatus}
          onRebalanceClick={onRebalanceClick}
        />
      </div>

      <HoldingsTable report={resolvedReport} />
    </div>
  );
}
