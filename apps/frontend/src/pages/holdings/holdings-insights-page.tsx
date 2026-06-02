import {
  Button,
  Card,
  CardContent,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
  EmptyPlaceholder,
  Icons,
  Skeleton,
} from "@wealthfolio/ui";
import { useCallback, useMemo, useState } from "react";

import { useHoldings } from "@/hooks/use-holdings";
import { usePortfolioAllocations } from "@/hooks/use-portfolio-allocations";
import { usePortfolios } from "@/hooks/use-portfolios";
import { isAlternativeAssetKind } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import type { AccountScope, AllocationTarget, DriftReport, TaxonomyAllocation } from "@/lib/types";
import { cn } from "@/lib/utils";
import { formatPp, formatTolerance } from "@/pages/allocation-targets/components/drift-copy";
import { isOutOfBand } from "@/pages/allocation-targets/components/drift-row-utils";
import { OverviewTab } from "@/pages/allocation-targets/components/overview-tab";
import {
  accountScopeKey,
  filterTargetsByScope,
} from "@/pages/allocation-targets/components/target-scope";
import { TargetDetailHeader } from "@/pages/allocation-targets/components/target-detail-header";
import { TargetsTab } from "@/pages/allocation-targets/components/targets-tab";
import { UnsavedTargetChangesDialog } from "@/pages/allocation-targets/components/unsaved-target-changes-dialog";
import { useAllocationTargetDrift } from "@/pages/allocation-targets/hooks/use-allocation-target-drift";
import { useAllocationTargets } from "@/pages/allocation-targets/hooks/use-allocation-targets";
import { useNavigate } from "react-router-dom";
import { AllocationDetailSheet } from "./components/allocation-detail-sheet";
import { CashHoldingsWidget } from "./components/cash-holdings-widget";
import { CompactAllocationStrip } from "./components/compact-allocation-strip";
import { PortfolioComposition } from "./components/composition-chart";
import { HoldingCurrencyChart } from "./components/currency-chart";
import { DrillableAccountChart } from "./components/drillable-account-chart";
import { DrillableDonutChart } from "./components/drillable-donut-chart";
import { SectorsChart } from "./components/sectors-chart";
import { SegmentedAllocationBar } from "./components/segmented-allocation-bar";
import {
  customAllocationDrilldownType,
  resolveAllocationForDrilldown,
} from "./allocation-drilldown";

interface HoldingsInsightsPageProps {
  accountId?: string;
  filter?: AccountScope;
}

type AllocationWorkspaceView = "current" | "drift" | "targets" | "rebalance";
type TargetEditorMode = "create" | "edit";

interface TargetAllocationCardProps {
  targets: AllocationTarget[];
  selectedTargetId: string | null;
  target: AllocationTarget | null;
  driftReport: DriftReport | null;
  isLoading: boolean;
  onTargetChange: (id: string) => void;
  onCreateTarget: () => void;
  onEditTarget: () => void;
  onViewDrift: () => void;
  onPlanRebalance: () => void;
}

function TargetAllocationCard({
  targets,
  selectedTargetId,
  target,
  driftReport,
  isLoading,
  onTargetChange,
  onCreateTarget,
  onEditTarget,
  onViewDrift,
  onPlanRebalance,
}: TargetAllocationCardProps) {
  const hasTargets = targets.length > 0;
  const isFine = driftReport ? driftReport.outOfBandCount === 0 : false;
  const largestGapRow = driftReport?.rows
    .filter(isOutOfBand)
    .sort((a, b) => Math.abs(b.driftBps) - Math.abs(a.driftBps))[0];

  return (
    <Card>
      <CardContent className="space-y-4 p-4">
        <div className="flex items-start justify-between gap-3">
          <div>
            <div className="text-muted-foreground text-[11px] font-medium uppercase tracking-wider">
              Target allocation
            </div>
            {target ? (
              <div className="mt-1 flex items-center gap-2">
                <span className="text-foreground text-[13px] font-semibold">{target.name}</span>
              </div>
            ) : (
              <div className="text-foreground mt-1 text-[13px] font-semibold">
                No target selected
              </div>
            )}
          </div>

          {hasTargets && (
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="ghost" size="icon-sm" aria-label="Select target">
                  <Icons.ChevronsUpDown className="h-3.5 w-3.5" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-56">
                {targets.map((p) => (
                  <DropdownMenuItem
                    key={p.id}
                    onSelect={() => onTargetChange(p.id)}
                    className={cn(selectedTargetId === p.id && "font-medium")}
                  >
                    <span className="flex-1">{p.name}</span>
                  </DropdownMenuItem>
                ))}
                <DropdownMenuSeparator />
                <DropdownMenuItem onSelect={onCreateTarget}>
                  <Icons.PlusCircle className="mr-2 h-4 w-4" />
                  New target
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          )}
        </div>

        {isLoading ? (
          <div className="space-y-2">
            <Skeleton className="h-4 w-3/4" />
            <Skeleton className="h-8 w-full" />
          </div>
        ) : driftReport ? (
          <div className="space-y-3">
            <div
              className={cn(
                "rounded-md border px-3 py-2",
                isFine
                  ? "border-green-200 bg-green-50 dark:border-green-800 dark:bg-green-950/20"
                  : "border-amber-200 bg-amber-50 dark:border-amber-800 dark:bg-amber-950/20",
              )}
            >
              <div className="flex items-center gap-2">
                <span
                  className={cn("h-2 w-2 rounded-full", isFine ? "bg-green-600" : "bg-amber-600")}
                />
                <span className="text-[12px] font-semibold">
                  {isFine
                    ? "All categories inside target range"
                    : `${driftReport.outOfBandCount} categories outside target range`}
                </span>
              </div>
              <div className="text-muted-foreground mt-1 text-[11px]">
                Largest gap {formatPp(largestGapRow?.driftBps ?? driftReport.maxDriftBps)} · drift
                tolerance {formatTolerance(target?.driftBandBps ?? 0)}
              </div>
            </div>

            <div className="flex flex-wrap gap-2">
              <Button size="sm" variant="outline" onClick={onViewDrift}>
                Allocation vs target
              </Button>
              <Button size="sm" variant="ghost" onClick={onEditTarget}>
                Edit target
              </Button>
              <Button size="sm" onClick={onPlanRebalance}>
                Review rebalance
              </Button>
            </div>
          </div>
        ) : (
          <div className="space-y-3">
            <p className="text-muted-foreground text-[12px] leading-relaxed">
              Add a target mix to compare this allocation against your intended portfolio.
            </p>
            <Button size="sm" onClick={onCreateTarget}>
              Set target allocation
            </Button>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function WorkspaceBackButton({
  title,
  description,
  onBack,
}: {
  title: string;
  description?: string;
  onBack: () => void;
}) {
  return (
    <div className="mb-5 flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
      <div className="flex flex-wrap items-center gap-3">
        <Button variant="ghost" size="sm" className="-ml-2" onClick={onBack}>
          <Icons.ArrowLeft className="mr-1.5 h-4 w-4" />
          Back to allocation
        </Button>
        <span className="bg-border hidden h-5 w-px sm:block" />
        <h2 className="text-foreground text-[16px] font-semibold">{title}</h2>
      </div>
      {description && (
        <p className="text-muted-foreground text-[12px] sm:text-right">{description}</p>
      )}
    </div>
  );
}

export const HoldingsInsightsPage = ({
  accountId: accountIdProp,
  filter: filterProp,
}: HoldingsInsightsPageProps) => {
  const navigate = useNavigate();
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const accountFilter: AccountScope = useMemo(
    () =>
      filterProp ??
      (accountIdProp ? { type: "account", accountId: accountIdProp } : { type: "all" }),
    [accountIdProp, filterProp],
  );
  const { holdings, isLoading: holdingsLoading } = useHoldings(accountFilter);
  const { allocations, isLoading: allocationsLoading } = usePortfolioAllocations(accountFilter);
  const { targets, isLoading: targetsLoading } = useAllocationTargets();
  const selectedAccountScopeKey = accountScopeKey(accountFilter);
  const [workspaceView, setWorkspaceView] = useState<AllocationWorkspaceView>("current");
  const [selectedTargetSelection, setSelectedTargetSelection] = useState<{
    scopeKey: string;
    id: string;
  } | null>(null);
  const [targetEditorMode, setTargetEditorMode] = useState<TargetEditorMode>("edit");
  const [targetEditorDirty, setTargetEditorDirty] = useState(false);
  const [confirmDiscardTargetChanges, setConfirmDiscardTargetChanges] = useState(false);
  const selectedTargetId =
    selectedTargetSelection?.scopeKey === selectedAccountScopeKey
      ? selectedTargetSelection.id
      : null;
  const setScopedSelectedTargetId = useCallback(
    (targetId: string | null) => {
      setSelectedTargetSelection(
        targetId ? { scopeKey: selectedAccountScopeKey, id: targetId } : null,
      );
    },
    [selectedAccountScopeKey],
  );

  const { data: portfolios = [] } = usePortfolios();
  const filteredAccountIds = useMemo(() => {
    if (accountFilter.type === "account") return [accountFilter.accountId];
    if (accountFilter.type === "accounts") return accountFilter.accountIds;
    if (accountFilter.type === "portfolio") {
      return portfolios.find((p) => p.id === accountFilter.portfolioId)?.accountIds ?? [];
    }
    return undefined; // "all" → DrillableAccountChart shows every account
  }, [accountFilter, portfolios]);

  const scopedTargets = useMemo(
    () => filterTargetsByScope(targets, accountFilter),
    [targets, accountFilter],
  );

  const scopedLiveTargets = useMemo(
    () => scopedTargets.filter((p) => !p.archivedAt),
    [scopedTargets],
  );

  const effectiveTargetId = selectedTargetId ?? scopedLiveTargets[0]?.id ?? null;
  const effectiveTarget = targets.find((p) => p.id === effectiveTargetId) ?? null;

  const { driftReport, isLoading: driftLoading } = useAllocationTargetDrift(
    effectiveTargetId,
    accountFilter,
    { includeHoldings: workspaceView === "drift" },
  );

  const isLoading = holdingsLoading || allocationsLoading;
  const targetLoading = targetsLoading || driftLoading;

  function handleCreateTarget() {
    setTargetEditorMode("create");
    setWorkspaceView("targets");
  }

  function handleEditTarget() {
    setTargetEditorMode("edit");
    setWorkspaceView("targets");
  }

  function handleBackToCurrentAllocation() {
    if (targetEditorDirty) {
      setConfirmDiscardTargetChanges(true);
      return;
    }
    setTargetEditorDirty(false);
    setWorkspaceView("current");
  }

  function discardTargetChangesAndReturn() {
    setConfirmDiscardTargetChanges(false);
    setTargetEditorDirty(false);
    setWorkspaceView("current");
  }

  function handleTargetEditorCancel() {
    setTargetEditorDirty(false);
    setTargetEditorMode("edit");
    setWorkspaceView(effectiveTargetId ? "drift" : "current");
  }

  function handleTargetEditorSaved(targetId: string) {
    setTargetEditorDirty(false);
    setTargetEditorMode("edit");
    setScopedSelectedTargetId(targetId);
    setWorkspaceView("drift");
  }

  // State for allocation detail sheet
  const [isSheetOpen, setIsSheetOpen] = useState(false);
  const [selectedAllocation, setSelectedAllocation] = useState<TaxonomyAllocation | undefined>(
    undefined,
  );
  const [initialCategoryId, setInitialCategoryId] = useState<string | null>(null);

  // Handle chart section click - opens sheet with clicked category pre-selected
  const handleChartSectionClick = useCallback(
    (type: string, _name: string, _title?: string, categoryId?: string) => {
      const allocation = resolveAllocationForDrilldown(allocations, type);
      if (allocation) {
        setSelectedAllocation(allocation);
        setInitialCategoryId(categoryId ?? null);
        setIsSheetOpen(true);
      }
    },
    [allocations],
  );

  // Handle card click - opens sheet with first category selected
  const openAllocationSheet = useCallback((allocation: TaxonomyAllocation | undefined) => {
    if (allocation) {
      setSelectedAllocation(allocation);
      setInitialCategoryId(null); // Will default to first category
      setIsSheetOpen(true);
    }
  }, []);

  const { cashHoldings, nonCashHoldings } = useMemo(() => {
    const cash = holdings?.filter((holding) => holding.holdingType?.toLowerCase() === "cash") ?? [];
    const nonCash =
      holdings?.filter((holding) => {
        if (holding.holdingType?.toLowerCase() === "cash") return false;
        if (holding.assetKind && isAlternativeAssetKind(holding.assetKind)) return false;
        return true;
      }) ?? [];

    return { cashHoldings: cash, nonCashHoldings: nonCash };
  }, [holdings]);

  const hasNoHoldingsAtAll = !isLoading && (!holdings || holdings.length === 0);

  const hasRiskAllocations =
    allocations?.riskCategory && allocations.riskCategory.categories.length > 0;

  const hasCustomGroups =
    allocations?.customGroups?.some(
      (taxonomy) =>
        taxonomy.categories.length > 0 &&
        taxonomy.categories.some(
          (cat) => cat.value > 0 && cat.categoryName.toLowerCase() !== "unknown",
        ),
    ) ?? false;

  const renderEmptyState = () => (
    <div className="flex items-center justify-center py-16">
      <EmptyPlaceholder
        icon={<Icons.TrendingUp className="text-muted-foreground h-10 w-10" />}
        title="No holdings yet"
        description="Get started by adding your first transaction or quickly import your existing holdings from a CSV file."
      >
        <div className="flex flex-col items-center gap-3 sm:flex-row">
          <Button size="default" onClick={() => navigate("/activities/manage")}>
            <Icons.Plus className="mr-2 h-4 w-4" />
            Add Transaction
          </Button>
          <Button size="default" variant="outline" onClick={() => navigate("/import")}>
            <Icons.Import className="mr-2 h-4 w-4" />
            Import from CSV
          </Button>
        </div>
      </EmptyPlaceholder>
    </div>
  );

  const renderAnalyticsView = () => {
    if (hasNoHoldingsAtAll) {
      return renderEmptyState();
    }

    return (
      <div className="space-y-4">
        {/* Row 1: Cash Balance (full width) */}
        <CashHoldingsWidget cashHoldings={cashHoldings ?? []} isLoading={isLoading} />

        {/* Row 2: 4 semi-donut charts */}
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
          <HoldingCurrencyChart
            holdings={[...cashHoldings, ...nonCashHoldings]}
            baseCurrency={baseCurrency}
            isLoading={isLoading}
            onCurrencySectionClick={(currencyName) =>
              handleChartSectionClick("currency", currencyName, `Holdings in ${currencyName}`)
            }
          />

          <DrillableAccountChart isLoading={isLoading} accountIds={filteredAccountIds} />

          <DrillableDonutChart
            title="Classes"
            allocation={allocations?.assetClasses}
            baseCurrency={baseCurrency}
            isLoading={isLoading}
            onCategoryClick={(categoryId, categoryName) =>
              handleChartSectionClick(
                "class",
                categoryName,
                `Asset Class: ${categoryName}`,
                categoryId,
              )
            }
            onCardClick={() => openAllocationSheet(allocations?.assetClasses)}
          />

          <DrillableDonutChart
            title="Regions"
            allocation={allocations?.regions}
            baseCurrency={baseCurrency}
            isLoading={isLoading}
            onCategoryClick={(categoryId, categoryName) =>
              handleChartSectionClick(
                "country",
                categoryName,
                `Holdings in ${categoryName}`,
                categoryId,
              )
            }
            onCardClick={() => openAllocationSheet(allocations?.regions)}
          />
        </div>

        {/* Row 3: Composition (col-span-3) + Right column (Security Type, Risk Profile, Sectors) */}
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-4">
          <div className="col-span-1 lg:col-span-3">
            <PortfolioComposition holdings={nonCashHoldings ?? []} isLoading={isLoading} />
          </div>

          <div className="col-span-1 space-y-4">
            <TargetAllocationCard
              targets={scopedLiveTargets}
              selectedTargetId={effectiveTargetId}
              target={effectiveTarget}
              driftReport={driftReport}
              isLoading={targetLoading}
              onTargetChange={setScopedSelectedTargetId}
              onCreateTarget={handleCreateTarget}
              onEditTarget={handleEditTarget}
              onViewDrift={() => setWorkspaceView(effectiveTargetId ? "drift" : "targets")}
              onPlanRebalance={() => setWorkspaceView("rebalance")}
            />

            {hasRiskAllocations && (
              <CompactAllocationStrip
                title="Risk Composition"
                allocation={allocations?.riskCategory}
                baseCurrency={baseCurrency}
                isLoading={isLoading}
                variant="risk-composition"
                onSegmentClick={(categoryId, categoryName) =>
                  handleChartSectionClick(
                    "risk",
                    categoryName,
                    `Risk Category: ${categoryName}`,
                    categoryId,
                  )
                }
              />
            )}

            <CompactAllocationStrip
              title="Security Types"
              allocation={allocations?.securityTypes}
              baseCurrency={baseCurrency}
              isLoading={isLoading}
              variant="security-types"
              onSegmentClick={(categoryId, categoryName) =>
                handleChartSectionClick(
                  "securityType",
                  categoryName,
                  `Type: ${categoryName}`,
                  categoryId,
                )
              }
            />

            <SectorsChart
              allocation={allocations?.sectors}
              baseCurrency={baseCurrency}
              isLoading={isLoading}
              onSectorSectionClick={(categoryId, categoryName) =>
                handleChartSectionClick(
                  "sector",
                  categoryName,
                  `Holdings in Sector: ${categoryName}`,
                  categoryId,
                )
              }
            />
          </div>
        </div>

        {/* Row 4: Custom Groups (under composition, col-span-3) */}
        {hasCustomGroups && (
          <div className="grid grid-cols-1 gap-4 lg:grid-cols-4">
            <div className="col-span-1 space-y-4 lg:col-span-3">
              {allocations?.customGroups?.map(
                (taxonomy) =>
                  taxonomy.categories.length > 0 &&
                  taxonomy.categories.some(
                    (cat) => cat.value > 0 && cat.categoryName.toLowerCase() !== "unknown",
                  ) && (
                    <SegmentedAllocationBar
                      key={taxonomy.taxonomyId}
                      title={taxonomy.taxonomyName}
                      allocation={taxonomy}
                      baseCurrency={baseCurrency}
                      isLoading={isLoading}
                      compact={true}
                      onSegmentClick={(categoryId, categoryName) =>
                        handleChartSectionClick(
                          customAllocationDrilldownType(taxonomy.taxonomyId),
                          categoryName,
                          `${taxonomy.taxonomyName}: ${categoryName}`,
                          categoryId,
                        )
                      }
                    />
                  ),
              )}
            </div>
          </div>
        )}
      </div>
    );
  };

  if (workspaceView === "targets") {
    return (
      <>
        <div>
          <WorkspaceBackButton title="Target allocation" onBack={handleBackToCurrentAllocation} />
          {targetsLoading ? (
            <div className="space-y-4">
              <Skeleton className="h-40 w-full" />
              <Skeleton className="h-32 w-full" />
            </div>
          ) : (
            <TargetsTab
              key={selectedAccountScopeKey}
              targets={scopedLiveTargets}
              selectedTargetId={effectiveTargetId}
              onTargetChange={setScopedSelectedTargetId}
              editorMode={targetEditorMode}
              accountScope={accountFilter}
              onUnsavedChange={setTargetEditorDirty}
              onCancel={handleTargetEditorCancel}
              onSaved={handleTargetEditorSaved}
              actionsPlacement="page-header"
            />
          )}
        </div>
        <UnsavedTargetChangesDialog
          open={confirmDiscardTargetChanges}
          onOpenChange={setConfirmDiscardTargetChanges}
          onDiscard={discardTargetChangesAndReturn}
        />
      </>
    );
  }

  if (workspaceView === "drift") {
    return (
      <div>
        <TargetDetailHeader
          targets={scopedLiveTargets}
          selectedTargetId={effectiveTargetId}
          target={effectiveTarget}
          onBack={handleBackToCurrentAllocation}
          onTargetChange={setScopedSelectedTargetId}
          onCreateTarget={handleCreateTarget}
          onEditTarget={handleEditTarget}
        />
        {driftReport ? (
          <OverviewTab
            report={driftReport}
            taxonomyId={effectiveTarget?.taxonomyId ?? "asset_classes"}
            targetName={effectiveTarget?.name}
            onRebalanceClick={() => setWorkspaceView("rebalance")}
          />
        ) : targetLoading ? (
          <div className="space-y-5">
            <div className="grid grid-cols-1 gap-5 md:grid-cols-2">
              <Skeleton className="h-64 w-full" />
              <Skeleton className="h-64 w-full" />
            </div>
            <Skeleton className="h-48 w-full" />
          </div>
        ) : (
          <EmptyPlaceholder
            icon={<Icons.Target className="text-muted-foreground h-10 w-10" />}
            title="No target in use"
            description="Create a target allocation to compare current weights against intended weights."
          >
            <Button size="sm" onClick={handleCreateTarget}>
              Set target allocation
            </Button>
          </EmptyPlaceholder>
        )}
      </div>
    );
  }

  if (workspaceView === "rebalance") {
    return (
      <div>
        <WorkspaceBackButton
          title="Rebalance plan"
          description="Generate suggested manual trades from allocation drift."
          onBack={handleBackToCurrentAllocation}
        />
        <EmptyPlaceholder
          icon={<Icons.BarChart className="text-muted-foreground h-10 w-10" />}
          title="Coming soon"
          description="Rebalance plan generator will be available in the next milestone."
        />
      </div>
    );
  }

  return (
    <>
      {renderAnalyticsView()}

      {/* Allocation Detail Sheet */}
      <AllocationDetailSheet
        isOpen={isSheetOpen}
        onOpenChange={setIsSheetOpen}
        allocation={selectedAllocation}
        accountFilter={accountFilter}
        baseCurrency={baseCurrency}
        initialCategoryId={initialCategoryId}
      />
    </>
  );
};

export default HoldingsInsightsPage;
