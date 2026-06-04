import React, { useState, useEffect } from "react";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AnimatedToggleGroup,
  Button,
  Icons,
  Skeleton,
} from "@wealthfolio/ui";

import { AccountScopeSelector } from "@/components/account-filter-selector";
import { useAccounts } from "@/hooks/use-accounts";
import { usePortfolioAllocations } from "@/hooks/use-portfolio-allocations";
import { usePortfolios } from "@/hooks/use-portfolios";
import { useTaxonomies, useTaxonomy } from "@/hooks/use-taxonomies";
import {
  useDeleteAllocationTarget,
  useAllocationTargetWeights,
  useSaveAllocationTargetWithWeights,
} from "../hooks/use-allocation-target-mutations";
import type {
  CategoryAllocation,
  PortfolioAllocations,
  AllocationTarget,
  AccountScope,
  RebalanceGoal,
  TargetScopeType,
  TaxonomyCategory,
} from "@/lib/types";
import { cn } from "@/lib/utils";
import { toast } from "sonner";
import { BUILT_IN_PRESETS, ModelPresetPicker, type ModelPreset } from "./model-preset-picker";
import { TargetWeightEditor, type WeightDraft } from "./target-weight-editor";
import { DriftBandSlider } from "./drift-band-slider";
import { accountScopeFromTarget, accountScopeKey } from "./target-scope";

type EditorMode =
  | { kind: "guided" }
  | {
      kind: "edit";
      targetId: string;
    };
type TargetEditorMode = "create" | "edit";

const UNKNOWN_ALLOCATION_CATEGORY_ID = "__UNKNOWN__";
const ROUNDING_TOLERANCE_BPS = 5;

function defaultScopeFromAccountScope(scope: AccountScope): {
  scopeType: TargetScopeType;
  scopeId: string | null;
} {
  if (scope.type === "account") return { scopeType: "account", scopeId: scope.accountId };
  if (scope.type === "portfolio") return { scopeType: "portfolio", scopeId: scope.portfolioId };
  return { scopeType: "all", scopeId: null };
}

function targetScopeLabel(
  target: AllocationTarget,
  accounts: { id: string; name: string }[],
  portfolios: { id: string; name: string }[],
): string {
  if (target.scopeType === "all") return "All Accounts";
  if (target.scopeType === "account" && target.scopeId) {
    return accounts.find((account) => account.id === target.scopeId)?.name ?? "Account target";
  }
  if (target.scopeType === "portfolio" && target.scopeId) {
    return (
      portfolios.find((portfolio) => portfolio.id === target.scopeId)?.name ?? "Portfolio target"
    );
  }
  return "Target scope";
}

function TargetScopeIcon({ scopeType }: { scopeType: TargetScopeType }) {
  if (scopeType === "portfolio") return <Icons.Folder className="h-4 w-4 shrink-0 opacity-70" />;
  if (scopeType === "account") {
    return <Icons.CreditCard className="h-4 w-4 shrink-0 opacity-70" />;
  }
  return <Icons.Wallet className="h-4 w-4 shrink-0 opacity-70" />;
}

function currentPreset(taxonomyId: string, categories: CategoryAllocation[]): ModelPreset {
  return {
    id: "current",
    taxonomyId,
    name: "Current allocation",
    description: "Start from what you hold today",
    risk: "From holdings",
    weights: Object.fromEntries(categories.map((c) => [c.categoryId, c.percentage])),
  };
}

function categoriesForTaxonomy(
  allocations: PortfolioAllocations | undefined,
  taxonomyId: string,
): CategoryAllocation[] {
  if (!allocations) return [];
  const byTaxonomy: Record<string, CategoryAllocation[]> = {
    asset_classes: allocations.assetClasses.categories,
    industries_gics: allocations.sectors.categories,
    regions: allocations.regions.categories,
    instrument_type: allocations.securityTypes.categories,
    risk_category: allocations.riskCategory.categories,
  };
  return (
    byTaxonomy[taxonomyId] ??
    allocations.customGroups.find((allocation) => allocation.taxonomyId === taxonomyId)
      ?.categories ??
    []
  );
}

function topLevelCategories(categories: CategoryAllocation[]): CategoryAllocation[] {
  return categories.filter(
    (c) =>
      c.categoryId !== UNKNOWN_ALLOCATION_CATEGORY_ID && (!c.children?.length || c.percentage > 0),
  );
}

function categoryLabelForTaxonomy(taxonomyName: string | undefined): string {
  if (!taxonomyName) return "Category";
  const normalized = taxonomyName.toLowerCase();
  if (normalized.includes("regions")) return "Region";
  if (normalized.includes("industries")) return "Industry";
  if (normalized.includes("risk")) return "Risk category";
  if (normalized.includes("custom")) return "Custom group";
  if (normalized.includes("asset classes")) return "Asset class";
  return "Category";
}

function normalizeWeights(
  weights: WeightDraft[],
  options: { roundingOnly?: boolean } = {},
): WeightDraft[] {
  const sum = weights.reduce((total, weight) => total + weight.targetBps, 0);
  if (sum <= 0 || sum === 10000) return weights;
  const diff = 10000 - sum;
  if (options.roundingOnly && Math.abs(diff) > ROUNDING_TOLERANCE_BPS) return weights;

  const maxIndex = weights.reduce(
    (max, weight, index) => (weight.targetBps > weights[max].targetBps ? index : max),
    0,
  );
  return weights.map((weight, index) =>
    index === maxIndex ? { ...weight, targetBps: weight.targetBps + diff } : weight,
  );
}

function buildGuidedWeights(
  startId: string,
  categories: TaxonomyCategory[],
  currentAllocation: Record<string, number>,
): WeightDraft[] {
  if (startId === "scratch") {
    return categories.map((category) => ({
      categoryId: category.id,
      targetBps: 0,
      isLocked: false,
    }));
  }

  if (startId === "current") {
    return normalizeWeights(
      categories.map((category) => ({
        categoryId: category.id,
        targetBps: Math.round((currentAllocation[category.id] ?? 0) * 100),
        isLocked: false,
      })),
      { roundingOnly: true },
    );
  }

  const preset = BUILT_IN_PRESETS.find((item) => item.id === startId);
  return normalizeWeights(
    categories.map((category) => ({
      categoryId: category.id,
      targetBps: Math.round((preset?.weights[category.id] ?? 0) * 100),
      isLocked: false,
    })),
  );
}

function StepHeader({
  number,
  children,
  className,
}: {
  number: number;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "text-muted-foreground flex items-center gap-2 text-[11px] font-medium uppercase tracking-normal",
        className,
      )}
    >
      <span className="bg-muted inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-[10px] font-semibold tabular-nums tracking-normal">
        {number}
      </span>
      <span>{children}</span>
    </div>
  );
}

function savedWeightsToDraft(
  weights: { categoryId: string; targetBps: number; isLocked: boolean }[],
): WeightDraft[] {
  return weights.map((weight) => ({
    categoryId: weight.categoryId,
    targetBps: weight.targetBps,
    isLocked: weight.isLocked,
  }));
}

function editorModeFromRequest(
  editorMode: TargetEditorMode | undefined,
  selectedTargetId: string | null,
  liveTargets: AllocationTarget[],
): EditorMode {
  if (editorMode === "create") return { kind: "guided" };
  if (selectedTargetId) return { kind: "edit", targetId: selectedTargetId };
  return liveTargets.length === 0
    ? { kind: "guided" }
    : { kind: "edit", targetId: liveTargets[0].id };
}

function TargetEditor({
  target,
  accountScope,
  onAccountScopeChange,
  allocations,
  actionsPlacement = "inline",
  onSaved,
  onCancel,
  onDelete,
  onUnsavedChange,
}: {
  target: AllocationTarget | null;
  accountScope: AccountScope;
  onAccountScopeChange?: (scope: AccountScope) => void;
  allocations?: PortfolioAllocations;
  actionsPlacement?: "inline" | "page-header";
  onSaved: (target: AllocationTarget) => void;
  onCancel: () => void;
  onDelete?: () => void;
  onUnsavedChange?: (dirty: boolean) => void;
}) {
  const { data: taxonomies = [] } = useTaxonomies({ scope: "asset" });
  const { accounts } = useAccounts({ filterActive: false, includeArchived: true });
  const { data: portfolios = [] } = usePortfolios();
  const guidedTaxonomies = taxonomies.filter((taxonomy) => taxonomy.id !== "instrument_type");
  const saveTarget = useSaveAllocationTargetWithWeights();
  const { data: existingWeightsData, isLoading: existingWeightsLoading } =
    useAllocationTargetWeights(target?.id ?? null);
  const [taxonomyId, setTaxonomyId] = useState(target?.taxonomyId ?? "asset_classes");
  const [startId, setStartId] = useState<string>(target ? "saved" : "current");
  const [targetName, setTargetName] = useState(target?.name ?? "");
  const [nameTouched, setNameTouched] = useState(!!target);
  const [driftBandPct, setDriftBandPct] = useState(target ? target.driftBandBps / 100 : 5);
  const [allowSells, setAllowSells] = useState(target?.allowSells ?? false);
  const [rebalanceGoal, setRebalanceGoal] = useState<RebalanceGoal>(
    target?.rebalanceGoal ?? "nearest_band",
  );
  const [minTradeAmount, setMinTradeAmount] = useState(target?.minTradeAmount ?? "0");
  const [wholeSharesOnly, setWholeSharesOnly] = useState(target?.wholeSharesOnly ?? false);
  const [weights, setWeights] = useState<WeightDraft[]>([]);
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const loadedWeightsTargetId = React.useRef<string | null>(null);
  const initializedGuidedWeightsKey = React.useRef<string | null>(null);
  const resetTargetId = target?.id ?? null;
  const resetTargetTaxonomyId = target?.taxonomyId ?? "asset_classes";
  const resetTargetName = target?.name ?? "";
  const resetTargetDriftBandBps = target?.driftBandBps ?? 500;

  const { data: taxonomy, isLoading: taxonomyLoading } = useTaxonomy(taxonomyId);
  const targetCategories = React.useMemo(
    () => taxonomy?.categories.filter((category) => !category.parentId) ?? [],
    [taxonomy],
  );
  const categories = React.useMemo(
    () => topLevelCategories(categoriesForTaxonomy(allocations, taxonomyId)),
    [allocations, taxonomyId],
  );
  const currentAllocation = React.useMemo(
    () =>
      Object.fromEntries(categories.map((category) => [category.categoryId, category.percentage])),
    [categories],
  );
  const guidedWeightsKey = React.useMemo(() => {
    if (target && startId === "saved") return null;
    const categoryKey = targetCategories.map((category) => category.id).join("|");
    const currentKey =
      startId === "current"
        ? Object.entries(currentAllocation)
            .sort(([left], [right]) => left.localeCompare(right))
            .map(([categoryId, percentage]) => `${categoryId}:${percentage}`)
            .join("|")
        : "";
    return `${taxonomyId}:${startId}:${categoryKey}:${currentKey}`;
  }, [currentAllocation, startId, target, targetCategories, taxonomyId]);
  const presets = React.useMemo(
    () => BUILT_IN_PRESETS.filter((preset) => preset.taxonomyId === taxonomyId),
    [taxonomyId],
  );
  const selectedPreset =
    startId === "scratch" || startId === "saved"
      ? null
      : startId === "current"
        ? currentPreset(taxonomyId, categories)
        : (presets.find((preset) => preset.id === startId) ?? null);
  const scope = target
    ? { scopeType: target.scopeType, scopeId: target.scopeId ?? null }
    : defaultScopeFromAccountScope(accountScope);
  const cannotTargetScope = !target && accountScope.type === "accounts";
  const selectedTaxonomy = taxonomies.find((taxonomy) => taxonomy.id === taxonomyId);
  const suggestedTargetName =
    startId === "scratch" || startId === "current"
      ? `${selectedTaxonomy?.name ?? "Custom"} target`
      : `${selectedPreset?.name ?? selectedTaxonomy?.name ?? "Custom"} target`;
  const savedWeightDrafts = React.useMemo(
    () => (existingWeightsData ? savedWeightsToDraft(existingWeightsData) : null),
    [existingWeightsData],
  );

  useEffect(() => {
    if (resetTargetId) return;
    if (!nameTouched) setTargetName(suggestedTargetName);
  }, [resetTargetId, nameTouched, suggestedTargetName]);

  useEffect(() => {
    if (resetTargetId) {
      setTaxonomyId(resetTargetTaxonomyId);
      setStartId("saved");
      setTargetName(resetTargetName);
      setNameTouched(true);
      setDriftBandPct(resetTargetDriftBandBps / 100);
      setAllowSells(target?.allowSells ?? false);
      setRebalanceGoal(target?.rebalanceGoal ?? "nearest_band");
      setMinTradeAmount(target?.minTradeAmount ?? "0");
      setWholeSharesOnly(target?.wholeSharesOnly ?? false);
    } else {
      setTaxonomyId("asset_classes");
      setStartId("current");
      setTargetName("");
      setNameTouched(false);
      setDriftBandPct(5);
      setAllowSells(false);
      setRebalanceGoal("nearest_band");
      setMinTradeAmount("0");
      setWholeSharesOnly(false);
    }
    setWeights([]);
    setHasUnsavedChanges(false);
    setDeleteOpen(false);
    loadedWeightsTargetId.current = null;
    initializedGuidedWeightsKey.current = null;
    onUnsavedChange?.(false);
  }, [
    resetTargetDriftBandBps,
    resetTargetId,
    resetTargetName,
    resetTargetTaxonomyId,
    onUnsavedChange,
    target?.allowSells,
    target?.rebalanceGoal,
    target?.minTradeAmount,
    target?.wholeSharesOnly,
  ]);

  useEffect(() => {
    if (!target || !savedWeightDrafts || loadedWeightsTargetId.current === target.id) return;
    loadedWeightsTargetId.current = target.id;
    setWeights(savedWeightDrafts);
    initializedGuidedWeightsKey.current = null;
  }, [target, savedWeightDrafts]);

  useEffect(() => {
    if (target && startId === "saved") return;
    if (!guidedWeightsKey || targetCategories.length === 0) return;
    if (hasUnsavedChanges && weights.length > 0) return;
    if (initializedGuidedWeightsKey.current === guidedWeightsKey) return;
    initializedGuidedWeightsKey.current = guidedWeightsKey;
    setWeights(buildGuidedWeights(startId, targetCategories, currentAllocation));
  }, [
    currentAllocation,
    guidedWeightsKey,
    hasUnsavedChanges,
    startId,
    target,
    targetCategories,
    weights.length,
  ]);

  function markDirty() {
    setHasUnsavedChanges(true);
    onUnsavedChange?.(true);
  }

  const handleTaxonomySelect = (id: string) => {
    if (id === taxonomyId) return;
    setTaxonomyId(id);
    if (target?.taxonomyId === id && savedWeightDrafts) {
      setStartId("saved");
      setWeights(savedWeightDrafts);
    } else {
      setStartId("current");
      setWeights([]);
    }
    initializedGuidedWeightsKey.current = null;
    if (!target) setNameTouched(false);
    markDirty();
  };

  const totalBps = weights.reduce((sum, weight) => sum + weight.targetBps, 0);
  const isSaving = saveTarget.isPending;
  const canSave = !cannotTargetScope && targetName.trim().length > 0 && totalBps === 10000;
  const selectedStartName =
    startId === "saved"
      ? "Saved target"
      : startId === "scratch"
        ? "Build from scratch"
        : (selectedPreset?.name ?? "Current allocation");
  const showEditorSkeleton =
    taxonomyLoading || (!!target && existingWeightsLoading && weights.length === 0);

  async function persistTarget() {
    if (!canSave || isSaving) return;

    try {
      const input = {
        name: targetName.trim(),
        scopeType: scope.scopeType,
        scopeId: scope.scopeType === "all" ? null : scope.scopeId,
        taxonomyId,
        triggerType: "threshold",
        driftBandBps: Math.round(driftBandPct * 100),
        allowSells,
        rebalanceGoal,
        minTradeAmount: minTradeAmount === "" ? "0" : minTradeAmount,
        wholeSharesOnly,
      } as const;

      const saved = await saveTarget.mutateAsync({
        id: target?.id ?? null,
        input,
        weights: weights.map((weight) => ({
          categoryId: weight.categoryId,
          targetBps: weight.targetBps,
          isLocked: weight.isLocked,
          isRequired: true,
        })),
      });

      setHasUnsavedChanges(false);
      onUnsavedChange?.(false);
      toast.success(target ? "Target saved" : "Target created");
      onSaved(saved.target);
    } catch (error) {
      toast.error(target ? "Failed to save target" : "Failed to create target");
      console.error(error);
    }
  }

  function handleCancel() {
    if (target) {
      setTaxonomyId(target.taxonomyId);
      setStartId("saved");
      setTargetName(target.name);
      setNameTouched(true);
      setDriftBandPct(target.driftBandBps / 100);
      setAllowSells(target.allowSells ?? false);
      setRebalanceGoal(target.rebalanceGoal ?? "nearest_band");
      setMinTradeAmount(target.minTradeAmount ?? "0");
      setWholeSharesOnly(target.wholeSharesOnly ?? false);
      if (savedWeightDrafts) setWeights(savedWeightDrafts);
    } else {
      setTaxonomyId("asset_classes");
      setStartId("current");
      setTargetName("");
      setNameTouched(false);
      setDriftBandPct(5);
      setRebalanceGoal("nearest_band");
      setMinTradeAmount("0");
      setWholeSharesOnly(false);
      setWeights([]);
    }
    initializedGuidedWeightsKey.current = null;
    setHasUnsavedChanges(false);
    onUnsavedChange?.(false);
    onCancel();
  }

  return (
    <div className="space-y-5">
      <div className={cn("flex justify-end", actionsPlacement === "page-header" && "-mt-14 mb-4")}>
        <div className="flex flex-wrap items-center gap-2 sm:justify-end">
          <Button variant="ghost" size="sm" onClick={handleCancel}>
            <Icons.X className="mr-1.5 h-4 w-4" />
            Cancel
          </Button>
          {target ? (
            <>
              <Button
                variant="outline"
                size="sm"
                disabled={!canSave || isSaving || !hasUnsavedChanges}
                onClick={() => persistTarget()}
              >
                {isSaving ? "Saving…" : "Save target"}
              </Button>
              {onDelete && (
                <Button
                  variant="ghost"
                  size="icon-sm"
                  className="text-destructive hover:text-destructive"
                  aria-label="Delete target"
                  title="Delete target"
                  onClick={() => setDeleteOpen(true)}
                >
                  <Icons.Trash className="h-4 w-4" />
                </Button>
              )}
            </>
          ) : (
            <Button size="sm" disabled={!canSave || isSaving} onClick={() => persistTarget()}>
              {isSaving ? "Creating…" : "Create target"}
            </Button>
          )}
        </div>
      </div>

      <div className="grid gap-4 lg:grid-cols-[320px_minmax(0,1fr)]">
        <div className="space-y-4">
          <section className="bg-card/80 rounded-lg border p-5 shadow-sm">
            <StepHeader number={1} className="mb-3">
              Name & scope
            </StepHeader>
            <label className="block">
              <span className="text-muted-foreground mb-1.5 block text-[11px] font-medium uppercase tracking-wider">
                Target name
              </span>
              <input
                value={targetName}
                onChange={(event) => {
                  setNameTouched(true);
                  setTargetName(event.target.value);
                  markDirty();
                }}
                placeholder="Target name"
                className="bg-background/70 text-foreground placeholder:text-muted-foreground focus:border-foreground w-full rounded-lg border px-3 py-2.5 text-[14px] font-semibold outline-none transition-colors placeholder:font-normal"
              />
            </label>
            <div className="mt-4">
              <div className="text-muted-foreground mb-1.5 text-[11px] font-medium uppercase tracking-wider">
                Account scope
              </div>
              {target ? (
                <div className="bg-muted/20 text-foreground flex items-center gap-2 rounded-lg border px-3 py-2.5 text-[14px] font-semibold">
                  <TargetScopeIcon scopeType={target.scopeType} />
                  <span className="min-w-0 truncate">
                    {targetScopeLabel(target, accounts, portfolios)}
                  </span>
                </div>
              ) : (
                <AccountScopeSelector
                  value={accountScope}
                  onChange={(nextScope) => {
                    onAccountScopeChange?.(nextScope);
                    markDirty();
                  }}
                  triggerVariant="input"
                  allowMultiAccount={false}
                />
              )}
            </div>
            {!target ? (
              <p className="text-muted-foreground mt-3 text-[12px] leading-relaxed">
                Targets are saved for the selected all-account, portfolio, or account scope.
              </p>
            ) : null}
            {cannotTargetScope && (
              <p className="text-destructive mt-3 text-[12px] leading-relaxed">
                Custom multi-account selections cannot have targets yet. Select all accounts, one
                portfolio, or one account.
              </p>
            )}
          </section>

          <section className="bg-card/80 rounded-lg border p-5 shadow-sm">
            <StepHeader number={2} className="mb-3">
              Allocation type
            </StepHeader>
            <div className="space-y-2">
              {guidedTaxonomies.map((taxonomy) => {
                const count = topLevelCategories(
                  categoriesForTaxonomy(allocations, taxonomy.id),
                ).length;
                const selected = taxonomyId === taxonomy.id;
                return (
                  <button
                    key={taxonomy.id}
                    type="button"
                    onClick={() => handleTaxonomySelect(taxonomy.id)}
                    className={cn(
                      "flex w-full items-center justify-between rounded-lg border px-3 py-2.5 text-left transition-colors",
                      selected ? "border-foreground bg-card" : "bg-muted/20 hover:bg-muted/40",
                    )}
                  >
                    <span className="min-w-0">
                      <span className="text-foreground block truncate text-[12.5px] font-semibold">
                        {taxonomy.name}
                      </span>
                      <span className="text-muted-foreground text-[11px]">
                        {count} current categories
                      </span>
                    </span>
                    {selected && (
                      <span className="bg-foreground text-background flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-[10px]">
                        ✓
                      </span>
                    )}
                  </button>
                );
              })}
            </div>
          </section>

          <section className="bg-card/80 rounded-lg border p-5 shadow-sm">
            <div className="text-muted-foreground mb-3 text-[11px] font-medium uppercase tracking-wider">
              Rebalance mode
            </div>
            <AnimatedToggleGroup<"buy_only" | "allow_sells">
              value={allowSells ? "allow_sells" : "buy_only"}
              onValueChange={(v) => {
                setAllowSells(v === "allow_sells");
                markDirty();
              }}
              items={[
                { value: "buy_only", label: "Buy only" },
                { value: "allow_sells", label: "Allow sells" },
              ]}
              rounded="lg"
              className="bg-muted/30 w-full border [&_button]:flex-1 [&_button]:py-2.5 [&_button]:text-[13px]"
            />
            <p className="text-muted-foreground mt-2 text-[11px]">
              {allowSells
                ? "Sell overweight positions to fund underweight ones."
                : "Deploy new cash only — no positions are sold."}
            </p>
          </section>

          <section className="bg-card/80 rounded-lg border p-5 shadow-sm">
            <div className="text-muted-foreground mb-3 text-[11px] font-medium uppercase tracking-wider">
              Rebalance goal
            </div>
            <AnimatedToggleGroup<RebalanceGoal>
              value={rebalanceGoal}
              onValueChange={(v) => {
                setRebalanceGoal(v);
                markDirty();
              }}
              items={[
                { value: "nearest_band", label: "Nearest band" },
                { value: "exact_target", label: "Exact target" },
              ]}
              rounded="lg"
              className="bg-muted/30 w-full border [&_button]:flex-1 [&_button]:py-2.5 [&_button]:text-[13px]"
            />
            <p className="text-muted-foreground mt-2 text-[11px]">
              {rebalanceGoal === "exact_target"
                ? "Deploy cash until each sleeve reaches exactly its target weight."
                : "Stop once each sleeve is within the drift tolerance band."}
            </p>
          </section>

          <section className="bg-card/80 rounded-lg border p-5 shadow-sm">
            <div className="text-muted-foreground mb-3 text-[11px] font-medium uppercase tracking-wider">
              Share sizing
            </div>
            <AnimatedToggleGroup<"fractional" | "whole">
              value={wholeSharesOnly ? "whole" : "fractional"}
              onValueChange={(v) => {
                setWholeSharesOnly(v === "whole");
                markDirty();
              }}
              items={[
                { value: "fractional", label: "Fractional" },
                { value: "whole", label: "Whole shares" },
              ]}
              rounded="lg"
              className="bg-muted/30 w-full border [&_button]:flex-1 [&_button]:py-2.5 [&_button]:text-[13px]"
            />
            <p className="text-muted-foreground mt-2 text-[11px]">
              {wholeSharesOnly
                ? "Suggest integer share quantities only."
                : "Allow fractional quantities for precise allocation."}
            </p>
          </section>

          <section className="bg-card/80 rounded-lg border p-5 shadow-sm">
            <label className="block">
              <div className="text-muted-foreground mb-3 text-[11px] font-medium uppercase tracking-wider">
                Minimum trade amount
              </div>
              <div className="border-input bg-background focus-within:ring-ring flex h-10 items-center rounded-md border px-3 focus-within:ring-2">
                <input
                  type="number"
                  min="0"
                  step="1"
                  value={minTradeAmount === "0" ? "" : minTradeAmount}
                  onChange={(e) => {
                    const v = e.target.value;
                    setMinTradeAmount(v === "" ? "0" : v);
                    markDirty();
                  }}
                  placeholder="0"
                  className="text-foreground placeholder:text-muted-foreground/60 w-full bg-transparent text-[14px] outline-none"
                />
              </div>
            </label>
            <p className="text-muted-foreground mt-2 text-[11px]">
              Trades below this amount are excluded from the plan.
            </p>
          </section>
        </div>

        <div className="space-y-4">
          <section className="bg-card/80 rounded-lg border p-5 shadow-sm">
            <div className="mb-3">
              <StepHeader number={3}>Starting point</StepHeader>
            </div>

            <ModelPresetPicker
              taxonomyId={taxonomyId}
              selected={startId === "saved" ? null : startId}
              onSelect={(presetId) => {
                setStartId(presetId);
                setWeights(buildGuidedWeights(presetId, targetCategories, currentAllocation));
                initializedGuidedWeightsKey.current = null;
                markDirty();
              }}
              currentCategories={categories}
              compact
            />
          </section>

          <section className="bg-card/80 rounded-lg border p-5 shadow-sm">
            <div className="mb-7 flex flex-col gap-3 xl:flex-row xl:items-start xl:justify-between">
              <div className="min-w-0">
                <h3 className="text-foreground text-[15px] font-semibold">
                  Target weights · {selectedStartName}
                </h3>
                <p className="text-muted-foreground mt-1 text-[12px]">
                  Set the intended mix. The total must equal 100%.
                </p>
                {cannotTargetScope && (
                  <p className="text-destructive mt-2 text-[11px] leading-relaxed">
                    Select all accounts, one portfolio, or one account before saving.
                  </p>
                )}
                {targetName.trim().length === 0 && (
                  <p className="text-destructive mt-2 text-[11px] leading-relaxed">
                    Add a target name before saving.
                  </p>
                )}
              </div>
              <DriftBandSlider
                value={driftBandPct}
                label="Drift tolerance"
                compact
                className="w-full xl:w-[300px]"
                onChange={(value) => {
                  setDriftBandPct(value);
                  markDirty();
                }}
              />
            </div>

            {showEditorSkeleton ? (
              <Skeleton className="h-64 w-full" />
            ) : targetCategories.length > 0 ? (
              <TargetWeightEditor
                categories={targetCategories}
                weights={weights}
                currentAllocation={currentAllocation}
                categoryLabel={categoryLabelForTaxonomy(selectedTaxonomy?.name)}
                onChange={(nextWeights) => {
                  setWeights(nextWeights);
                  markDirty();
                }}
              />
            ) : (
              <p className="text-muted-foreground rounded-lg border px-4 py-6 text-[12px]">
                No categories found for this allocation type.
              </p>
            )}
          </section>
        </div>
      </div>

      <AlertDialog open={deleteOpen} onOpenChange={setDeleteOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete target?</AlertDialogTitle>
            <AlertDialogDescription>
              This will permanently delete &ldquo;{targetName}&rdquo; and all its target weights.
              This action cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
              onClick={() => {
                setDeleteOpen(false);
                setHasUnsavedChanges(false);
                onUnsavedChange?.(false);
                onDelete?.();
              }}
            >
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

interface TargetsTabProps {
  targets: AllocationTarget[];
  selectedTargetId: string | null;
  onTargetChange: (id: string) => void;
  editorMode?: TargetEditorMode;
  accountScope: AccountScope;
  onAccountScopeChange?: (scope: AccountScope) => void;
  actionsPlacement?: "inline" | "page-header";
  onUnsavedChange?: (dirty: boolean) => void;
  onCancel?: () => void;
  onSaved?: (target: AllocationTarget) => void;
}

export function TargetsTab({
  targets,
  selectedTargetId,
  onTargetChange,
  editorMode,
  accountScope,
  onAccountScopeChange,
  actionsPlacement = "inline",
  onUnsavedChange,
  onCancel,
  onSaved,
}: TargetsTabProps) {
  const liveTargets = React.useMemo(() => targets.filter((p) => !p.archivedAt), [targets]);
  const parentAccountScopeKey = accountScopeKey(accountScope);
  const accountScopeRef = React.useRef(accountScope);
  const [draftAccountScope, setDraftAccountScope] = useState<AccountScope>(accountScope);

  const [mode, setMode] = useState<EditorMode>(() =>
    editorModeFromRequest(editorMode, selectedTargetId, liveTargets),
  );
  const modeTargetId = mode.kind === "edit" ? mode.targetId : null;

  useEffect(() => {
    accountScopeRef.current = accountScope;
  }, [accountScope]);

  // Sync editor when selected target changes from header dropdown.
  useEffect(() => {
    if (editorMode === "create") return;
    if (!selectedTargetId) return;
    if (mode.kind !== "edit" || modeTargetId !== selectedTargetId) {
      setMode({ kind: "edit", targetId: selectedTargetId });
    }
  }, [editorMode, mode.kind, modeTargetId, selectedTargetId]);

  // Explicit parent intent: create opens a blank target, edit opens the selected target.
  useEffect(() => {
    if (!editorMode) return;
    if (editorMode === "create") {
      setDraftAccountScope(accountScopeRef.current);
      setMode({ kind: "guided" });
      return;
    }
    setMode(editorModeFromRequest(editorMode, selectedTargetId, liveTargets));
  }, [editorMode, selectedTargetId, liveTargets, parentAccountScopeKey]);

  useEffect(() => {
    setDraftAccountScope(accountScopeRef.current);
  }, [parentAccountScopeKey]);

  const editingModeTargetId = modeTargetId;

  useEffect(() => {
    if (!editingModeTargetId) return;
    if (liveTargets.some((p) => p.id === editingModeTargetId)) return;
    const fallback = liveTargets[0] ?? null;
    setMode(fallback ? { kind: "edit", targetId: fallback.id } : { kind: "guided" });
  }, [liveTargets, editingModeTargetId]);

  const editingTarget = React.useMemo(
    () =>
      mode.kind === "edit" && mode.targetId
        ? (targets.find((p) => p.id === mode.targetId) ?? null)
        : null,
    [mode, targets],
  );
  const editorAccountScope = React.useMemo(
    () =>
      mode.kind === "guided"
        ? draftAccountScope
        : (accountScopeFromTarget(editingTarget) ?? accountScope),
    [accountScope, draftAccountScope, editingTarget, mode.kind],
  );

  const { allocations, isLoading: allocationsLoading } = usePortfolioAllocations(
    editorAccountScope,
    { keepPreviousData: true },
  );
  const deleteTarget = useDeleteAllocationTarget();

  function handleDraftAccountScopeChange(nextScope: AccountScope) {
    setDraftAccountScope(nextScope);
    onAccountScopeChange?.(nextScope);
  }

  function handleEditorSaved(target: AllocationTarget) {
    onTargetChange(target.id);
    setMode({ kind: "edit", targetId: target.id });
    onSaved?.(target);
  }

  function handleEditorCancel() {
    if (onCancel) {
      onCancel();
      return;
    }

    if (liveTargets.length === 0) {
      setMode({ kind: "guided" });
    } else {
      const fallbackId = selectedTargetId ?? liveTargets[0].id;
      setMode({ kind: "edit", targetId: fallbackId });
    }
  }

  function navigateAfterRemove(removedId: string) {
    const remaining = liveTargets.filter((p) => p.id !== removedId);
    const fallback = remaining[0] ?? null;
    if (fallback) {
      onTargetChange(fallback.id);
      setMode({ kind: "edit", targetId: fallback.id });
    } else {
      setMode({ kind: "guided" });
    }
  }

  function handleEditorDelete() {
    if (!editingTarget) return;
    deleteTarget.mutate(editingTarget.id, {
      onSuccess: () => navigateAfterRemove(editingTarget.id),
    });
  }

  if (allocationsLoading && !allocations) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-40 w-full" />
        <Skeleton className="h-32 w-full" />
      </div>
    );
  }

  if (mode.kind === "guided") {
    return (
      <TargetEditor
        key="new"
        target={null}
        accountScope={editorAccountScope}
        onAccountScopeChange={handleDraftAccountScopeChange}
        allocations={allocations}
        actionsPlacement={actionsPlacement}
        onSaved={handleEditorSaved}
        onCancel={handleEditorCancel}
        onUnsavedChange={onUnsavedChange}
      />
    );
  }

  return (
    <TargetEditor
      key={mode.targetId}
      target={editingTarget}
      accountScope={editorAccountScope}
      allocations={allocations}
      actionsPlacement={actionsPlacement}
      onSaved={handleEditorSaved}
      onCancel={handleEditorCancel}
      onDelete={editingTarget ? handleEditorDelete : undefined}
      onUnsavedChange={onUnsavedChange}
    />
  );
}
