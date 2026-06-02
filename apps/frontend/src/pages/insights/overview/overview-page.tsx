import { useAccounts } from "@/hooks/use-accounts";
import { useHoldings } from "@/hooks/use-holdings";
import { usePortfolioAllocations } from "@/hooks/use-portfolio-allocations";
import { usePortfolios } from "@/hooks/use-portfolios";
import { isAlternativeAssetKind } from "@/lib/constants";
import { useSettingsContext } from "@/lib/settings-provider";
import type { AccountScope, TaxonomyAllocation } from "@/lib/types";
import { OverviewTab } from "@/pages/allocation-targets/components/overview-tab";
import {
  TargetDetailHeader,
  TargetToolbarActions,
} from "@/pages/allocation-targets/components/target-detail-header";
import {
  accountScopeFromTarget,
  accountScopeKey,
  filterTargetsByScope,
} from "@/pages/allocation-targets/components/target-scope";
import { TargetsTab } from "@/pages/allocation-targets/components/targets-tab";
import { UnsavedTargetChangesDialog } from "@/pages/allocation-targets/components/unsaved-target-changes-dialog";
import { useAllocationTargetDrift } from "@/pages/allocation-targets/hooks/use-allocation-target-drift";
import { useAllocationTargets } from "@/pages/allocation-targets/hooks/use-allocation-targets";
import { AllocationDetailSheet } from "@/pages/holdings/components/allocation-detail-sheet";
import { Button, EmptyPlaceholder, Icons, Skeleton } from "@wealthfolio/ui";
import { PortfolioComposition } from "@/pages/holdings/components/composition-chart";
import { DrillableAccountChart } from "@/pages/holdings/components/drillable-account-chart";
import { DrillableDonutChart } from "@/pages/holdings/components/drillable-donut-chart";
import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { computeValueStrip } from "./allocation-derivations";
import { PortfolioExplorer } from "./portfolio-explorer";
import { TargetRailsCard } from "./target-rails-card";
import { ValueStrip } from "./value-strip";

interface OverviewPageProps {
  filter?: AccountScope;
  onToolbarActionsChange?: (actions: ReactNode | null) => void;
}

type WorkspaceView = "current" | "details" | "targets";
type TargetEditorMode = "create" | "edit";

export function OverviewPage({ filter: filterProp, onToolbarActionsChange }: OverviewPageProps) {
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const accountFilter: AccountScope = useMemo(() => filterProp ?? { type: "all" }, [filterProp]);
  const selectedAccountScopeKey = accountScopeKey(accountFilter);

  const { holdings, isLoading: holdingsLoading } = useHoldings(accountFilter);
  const { allocations, isLoading: allocationsLoading } = usePortfolioAllocations(accountFilter);
  const { accounts } = useAccounts();
  const { data: portfolios = [] } = usePortfolios();
  const { targets, isLoading: targetsLoading } = useAllocationTargets();

  const [selectedTargetId, setSelectedTargetId] = useState<string | null>(null);
  const [workspaceView, setWorkspaceView] = useState<WorkspaceView>("current");
  const [targetEditorMode, setTargetEditorMode] = useState<TargetEditorMode>("edit");
  const [targetEditorDirty, setTargetEditorDirty] = useState(false);
  const [pendingWorkspaceView, setPendingWorkspaceView] = useState<WorkspaceView | null>(null);
  const [pendingTargetId, setPendingTargetId] = useState<string | null>(null);

  useEffect(() => {
    setSelectedTargetId(null);
  }, [selectedAccountScopeKey]);

  const scopedTargets = useMemo(
    () => filterTargetsByScope(targets, accountFilter),
    [targets, accountFilter],
  );
  const scopedLiveTargets = useMemo(
    () => scopedTargets.filter((p) => !p.archivedAt),
    [scopedTargets],
  );
  const isCreatingTarget = workspaceView === "targets" && targetEditorMode === "create";
  const effectiveTargetId =
    !isCreatingTarget &&
    selectedTargetId &&
    scopedLiveTargets.some((p) => p.id === selectedTargetId)
      ? selectedTargetId
      : !isCreatingTarget
        ? (scopedLiveTargets[0]?.id ?? null)
        : null;
  const effectiveTarget = targets.find((p) => p.id === effectiveTargetId) ?? null;
  const targetEditorAccountScope = isCreatingTarget
    ? accountFilter
    : (accountScopeFromTarget(effectiveTarget) ?? accountFilter);

  const { driftReport, isLoading: driftLoading } = useAllocationTargetDrift(
    effectiveTargetId,
    accountFilter,
    { includeHoldings: workspaceView === "details" },
  );

  const isLoading = holdingsLoading || allocationsLoading;
  const targetLoading = targetsLoading || driftLoading;

  const filteredAccountIds = useMemo(() => {
    if (accountFilter.type === "account") return [accountFilter.accountId];
    if (accountFilter.type === "accounts") return accountFilter.accountIds;
    if (accountFilter.type === "portfolio") {
      return portfolios.find((p) => p.id === accountFilter.portfolioId)?.accountIds ?? [];
    }
    return undefined; // "all" → every account
  }, [accountFilter, portfolios]);

  const portfolioHoldings = useMemo(
    () =>
      holdings?.filter((h) => {
        if (h.assetKind && isAlternativeAssetKind(h.assetKind)) return false;
        return true;
      }) ?? [],
    [holdings],
  );

  const nonCashHoldings = useMemo(
    () => portfolioHoldings.filter((h) => h.holdingType?.toLowerCase() !== "cash"),
    [portfolioHoldings],
  );

  const valueStrip = useMemo(
    () => computeValueStrip(portfolioHoldings, accounts),
    [portfolioHoldings, accounts],
  );

  // Detail sheet
  const [isSheetOpen, setIsSheetOpen] = useState(false);
  const [selectedAllocation, setSelectedAllocation] = useState<TaxonomyAllocation | undefined>();
  const [initialCategoryId, setInitialCategoryId] = useState<string | null>(null);

  function openAllocationSheet(allocation: TaxonomyAllocation | undefined, categoryId?: string) {
    if (!allocation) return;
    setSelectedAllocation(allocation);
    setInitialCategoryId(categoryId ?? null);
    setIsSheetOpen(true);
  }

  const startCreateTarget = useCallback(() => {
    setTargetEditorMode("create");
    setWorkspaceView("targets");
  }, []);

  const handleCreateTarget = useCallback(() => {
    if (targetEditorDirty) {
      setPendingTargetId(null);
      setPendingWorkspaceView("targets");
      return;
    }
    startCreateTarget();
  }, [startCreateTarget, targetEditorDirty]);

  const handleEditTarget = useCallback(() => {
    setTargetEditorMode("edit");
    setWorkspaceView("targets");
  }, []);

  const requestTargetChange = useCallback(
    (targetId: string) => {
      if (targetEditorDirty) {
        setPendingTargetId(targetId);
        return;
      }
      setTargetEditorMode("edit");
      setSelectedTargetId(targetId);
    },
    [targetEditorDirty],
  );

  function backTo(view: WorkspaceView) {
    if (targetEditorDirty) {
      setPendingWorkspaceView(view);
      return;
    }
    setTargetEditorDirty(false);
    setWorkspaceView(view);
  }

  function discardTargetChanges() {
    setTargetEditorDirty(false);
    if (pendingTargetId) {
      setTargetEditorMode("edit");
      setSelectedTargetId(pendingTargetId);
      setPendingTargetId(null);
      setPendingWorkspaceView(null);
      return;
    }
    if (!pendingWorkspaceView) return;
    if (pendingWorkspaceView === "targets") {
      startCreateTarget();
    } else {
      setWorkspaceView(pendingWorkspaceView);
    }
    setPendingWorkspaceView(null);
  }

  function handleTargetEditorCancel() {
    const fallbackTargetId =
      selectedTargetId && scopedLiveTargets.some((target) => target.id === selectedTargetId)
        ? selectedTargetId
        : (scopedLiveTargets[0]?.id ?? null);
    setTargetEditorDirty(false);
    setTargetEditorMode("edit");
    setWorkspaceView(fallbackTargetId ? "details" : "current");
  }

  function handleTargetEditorSaved(targetId: string) {
    setTargetEditorDirty(false);
    setTargetEditorMode("edit");
    setSelectedTargetId(targetId);
    setWorkspaceView("details");
  }

  const toolbarActions = useMemo(() => {
    if (workspaceView === "current") return null;

    return (
      <TargetToolbarActions
        targets={scopedLiveTargets}
        selectedTargetId={effectiveTargetId}
        target={effectiveTarget}
        onTargetChange={requestTargetChange}
        onCreateTarget={handleCreateTarget}
        onEditTarget={workspaceView === "details" ? handleEditTarget : undefined}
      />
    );
  }, [
    effectiveTarget,
    effectiveTargetId,
    handleCreateTarget,
    handleEditTarget,
    requestTargetChange,
    scopedLiveTargets,
    workspaceView,
  ]);

  useEffect(() => {
    onToolbarActionsChange?.(toolbarActions);
    return () => onToolbarActionsChange?.(null);
  }, [onToolbarActionsChange, toolbarActions]);

  if (workspaceView === "targets") {
    return (
      <>
        <div>
          <div className="mb-5 flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex flex-wrap items-center gap-3">
              <Button
                variant="ghost"
                size="sm"
                className="-ml-2"
                onClick={() => backTo(effectiveTargetId ? "details" : "current")}
              >
                <Icons.ArrowLeft className="mr-1.5 h-4 w-4" />
                Back to allocation
              </Button>
              <span className="bg-border hidden h-5 w-px sm:block" />
              <h2 className="text-foreground text-[16px] font-semibold">Target allocation</h2>
            </div>
          </div>
          {targetsLoading ? (
            <div className="space-y-4">
              <Skeleton className="h-40 w-full" />
              <Skeleton className="h-32 w-full" />
            </div>
          ) : (
            <TargetsTab
              targets={scopedLiveTargets}
              selectedTargetId={effectiveTargetId}
              onTargetChange={(targetId) => {
                setTargetEditorMode("edit");
                setSelectedTargetId(targetId);
              }}
              editorMode={targetEditorMode}
              accountScope={targetEditorAccountScope}
              onUnsavedChange={setTargetEditorDirty}
              onCancel={handleTargetEditorCancel}
              onSaved={handleTargetEditorSaved}
              actionsPlacement="page-header"
            />
          )}
        </div>
        <UnsavedTargetChangesDialog
          open={pendingWorkspaceView !== null || pendingTargetId !== null}
          onOpenChange={(open) => {
            if (!open) {
              setPendingWorkspaceView(null);
              setPendingTargetId(null);
            }
          }}
          onDiscard={discardTargetChanges}
        />
      </>
    );
  }

  if (workspaceView === "details") {
    return (
      <div>
        <TargetDetailHeader
          targets={scopedLiveTargets}
          selectedTargetId={effectiveTargetId}
          target={effectiveTarget}
          onBack={() => backTo("current")}
          onTargetChange={requestTargetChange}
          onCreateTarget={handleCreateTarget}
          onEditTarget={handleEditTarget}
          showActions={false}
        />
        {driftReport ? (
          <OverviewTab
            report={driftReport}
            taxonomyId={effectiveTarget?.taxonomyId ?? "asset_classes"}
            targetName={effectiveTarget?.name}
            onRebalanceClick={() => undefined}
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
            title="No target selected"
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

  return (
    <>
      <div className="space-y-4">
        {/* Row 1 — compact value strip */}
        <ValueStrip data={valueStrip} currency={baseCurrency} isLoading={isLoading} compact />

        {/* Row 2 — exploration previews */}
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
          <DrillableAccountChart isLoading={isLoading} accountIds={filteredAccountIds} />
          <DrillableDonutChart
            title="Classes"
            allocation={allocations?.assetClasses}
            baseCurrency={baseCurrency}
            isLoading={isLoading}
            onCategoryClick={(categoryId) =>
              openAllocationSheet(allocations?.assetClasses, categoryId)
            }
            onCardClick={() => openAllocationSheet(allocations?.assetClasses)}
          />
          <DrillableDonutChart
            title="Regions"
            allocation={allocations?.regions}
            baseCurrency={baseCurrency}
            isLoading={isLoading}
            onCategoryClick={(categoryId) => openAllocationSheet(allocations?.regions, categoryId)}
            onCardClick={() => openAllocationSheet(allocations?.regions)}
          />
          <DrillableDonutChart
            title="Sectors"
            allocation={allocations?.sectors}
            baseCurrency={baseCurrency}
            isLoading={isLoading}
            onCategoryClick={(categoryId) => openAllocationSheet(allocations?.sectors, categoryId)}
            onCardClick={() => openAllocationSheet(allocations?.sectors)}
          />
        </div>

        {/* Row 3 — treemap + target rails, aligned to the 4-column grid above */}
        <div className="grid grid-cols-1 gap-4 xl:grid-cols-4">
          <div className="xl:col-span-3">
            <PortfolioComposition holdings={nonCashHoldings} isLoading={isLoading} />
          </div>
          <TargetRailsCard
            targets={scopedLiveTargets}
            selectedTargetId={effectiveTargetId}
            onTargetChange={requestTargetChange}
            driftReport={driftReport}
            driftBandBps={effectiveTarget?.driftBandBps ?? 0}
            isLoading={driftLoading && !driftReport}
            onCreateTarget={handleCreateTarget}
            onViewDetails={() => setWorkspaceView("details")}
          />
        </div>

        {/* Row 4 — breakdown */}
        <PortfolioExplorer
          allocations={allocations}
          holdings={holdings ?? []}
          accounts={accounts}
          accountIds={filteredAccountIds}
          currency={baseCurrency}
          isLoading={isLoading}
          onOpenAllocation={openAllocationSheet}
        />
      </div>

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
}

export default OverviewPage;
