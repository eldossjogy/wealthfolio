import React, { useState, useEffect, useRef, useMemo } from "react";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
  Icons,
  Switch,
} from "@wealthfolio/ui";
import { cn } from "@/lib/utils";
import { toast } from "sonner";
import type { TargetProfile, TaxonomyWithCategories, TargetScopeType } from "@/lib/types";
import { useTargetNodes } from "../hooks/use-target-mutations";
import { useSaveTargetNodes } from "../hooks/use-target-mutations";
import {
  useCreateTargetProfile,
  useUpdateTargetProfile,
  useActivateTargetProfile,
} from "../hooks/use-target-mutations";
import { TargetNodeEditor, type NodeDraft } from "./target-node-editor";
import { ModelPresetPicker } from "./model-preset-picker";
import { BUILT_IN_PRESETS } from "./model-preset-data";
import type { PortfolioStats } from "../hooks/use-portfolio-stats";

interface ProfileEditorProps {
  profile: TargetProfile | null;
  taxonomy: TaxonomyWithCategories;
  initialPresetId?: string | null;
  currentAllocation?: Record<string, number>;
  baseCurrency: string;
  portfolioStats?: PortfolioStats | null;
  defaultScopeType?: TargetScopeType;
  defaultScopeId?: string | null;
  onSaved: (profileId: string) => void;
  onCancel: () => void;
  onArchive?: () => void;
  onDelete?: () => void;
  onUnsavedChange?: (dirty: boolean) => void;
  saveRef?: React.MutableRefObject<(() => void) | null>;
}

const TRIGGER_OPTIONS = [
  {
    value: "threshold",
    label: "Threshold",
    description: "Rebalance when drift exceeds band",
    badge: "Recommended",
  },
  {
    value: "manual",
    label: "Manual",
    description: "Only rebalance when you trigger it",
    badge: "Simple",
  },
] as const;

const REBALANCE_TO_OPTIONS = [
  {
    value: "nearest_band",
    label: "Nearest band",
    description: "Minimum trades to come back in band",
  },
  {
    value: "exact_target",
    label: "Exact target",
    description: "Aim for precise target percentages",
  },
] as const;

function buildInitialNodes(
  presetId: string | null | undefined,
  categories: { id: string }[],
  currentAllocation: Record<string, number>,
): NodeDraft[] {
  if (!presetId) {
    return categories.map((c) => ({ categoryId: c.id, targetBps: 0, isLocked: false }));
  }

  if (presetId === "current") {
    const raw = categories.map((c) => ({
      categoryId: c.id,
      targetBps: Math.round((currentAllocation[c.id] ?? 0) * 100),
      isLocked: false,
    }));
    const sum = raw.reduce((s, n) => s + n.targetBps, 0);
    if (sum > 0 && sum !== 10000) {
      const diff = 10000 - sum;
      const maxIdx = raw.reduce((mi, n, i) => (n.targetBps > raw[mi].targetBps ? i : mi), 0);
      raw[maxIdx].targetBps += diff;
    }
    return raw;
  }

  const preset = BUILT_IN_PRESETS.find((p) => p.id === presetId);
  if (!preset) return categories.map((c) => ({ categoryId: c.id, targetBps: 0, isLocked: false }));

  return categories.map((c) => ({
    categoryId: c.id,
    targetBps: Math.round((preset.weights[c.id] ?? 0) * 100),
    isLocked: false,
  }));
}

export function ProfileEditor({
  profile,
  taxonomy,
  initialPresetId,
  currentAllocation = {},
  baseCurrency,
  portfolioStats,
  defaultScopeType,
  defaultScopeId,
  onSaved,
  onCancel,
  onArchive,
  onDelete,
  onUnsavedChange,
  saveRef,
}: ProfileEditorProps) {
  const topLevelCategories = useMemo(
    () => taxonomy.categories.filter((c) => !c.parentId),
    [taxonomy.categories],
  );

  const { data: existingNodesData } = useTargetNodes(profile?.id ?? null);

  const [name, setName] = useState(profile?.name ?? "");
  const [deleteOpen, setDeleteOpen] = useState(false);
  const scopeType: TargetScopeType = profile?.scopeType ?? defaultScopeType ?? "all";
  const scopeId: string | null = profile?.scopeId ?? defaultScopeId ?? null;
  const [driftBandPct, setDriftBandPct] = useState(profile ? profile.driftBandBps / 100 : 5);
  const [triggerType, setTriggerType] = useState<"threshold" | "manual">(
    (profile?.triggerType as "threshold" | "manual") ?? "threshold",
  );
  const [rebalanceTo, setRebalanceTo] = useState<"nearest_band" | "exact_target">(
    (profile?.rebalanceTo as "nearest_band" | "exact_target") ?? "nearest_band",
  );
  const [allowSells, setAllowSells] = useState(profile?.allowSells ?? false);
  const [minTradeAmount, setMinTradeAmount] = useState(profile?.minTradeAmount ?? "0");
  const [wholeSharesOnly, setWholeSharesOnly] = useState(profile?.wholeSharesOnly ?? false);
  const [selectedPreset, setSelectedPreset] = useState<string | null>(initialPresetId ?? null);
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);

  useEffect(() => {
    onUnsavedChange?.(hasUnsavedChanges);
  }, [hasUnsavedChanges, onUnsavedChange]);

  const [nodes, setNodes] = useState<NodeDraft[]>(() => {
    if (!profile) {
      return buildInitialNodes(initialPresetId ?? null, topLevelCategories, currentAllocation);
    }
    return [];
  });

  // Tracks the persisted profile ID so Activate after Save draft updates instead of creating
  const persistedProfileId = useRef<string | null>(profile?.id ?? null);
  const nodesInitialized = useRef(false);
  useEffect(() => {
    if (!profile || nodesInitialized.current || !existingNodesData) return;
    nodesInitialized.current = true;
    setNodes(
      existingNodesData.map((n) => ({
        categoryId: n.categoryId,
        targetBps: n.targetBps,
        isLocked: n.isLocked,
      })),
    );
  }, [profile, existingNodesData]);

  const createProfile = useCreateTargetProfile();
  const updateProfile = useUpdateTargetProfile();
  const activateProfile = useActivateTargetProfile();
  const saveNodes = useSaveTargetNodes();

  const totalBps = nodes.reduce((s, n) => s + n.targetBps, 0);
  const isValid = name.trim().length > 0 && totalBps === 10000;
  const isSaving = createProfile.isPending || updateProfile.isPending || saveNodes.isPending;
  const isActivating = isSaving || activateProfile.isPending;

  // Build color map for ModelPresetPicker
  const currentCategoriesForPicker = topLevelCategories.map((c) => ({
    categoryId: c.id,
    categoryName: c.name,
    color: c.color,
    value: 0,
    percentage: currentAllocation[c.id] ?? 0,
  }));

  function handlePresetSelect(presetId: string) {
    setSelectedPreset(presetId);
    setNodes(buildInitialNodes(presetId, topLevelCategories, currentAllocation));
    setHasUnsavedChanges(true);
  }

  async function persistProfile(andActivate: boolean) {
    try {
      const input = {
        name: name.trim(),
        scopeType,
        scopeId: scopeType === "all" ? null : scopeId,
        taxonomyId: "asset_classes",
        baseCurrency,
        triggerType,
        driftBandBps: Math.round(driftBandPct * 100),
        rebalanceTo,
        allowSells,
        minTradeAmount: minTradeAmount || "0",
        wholeSharesOnly,
      };

      let profileId: string;
      const persistedId = persistedProfileId.current;
      if (persistedId) {
        const updated = await updateProfile.mutateAsync({ id: persistedId, input });
        profileId = updated.id;
      } else {
        const created = await createProfile.mutateAsync(input);
        profileId = created.id;
        persistedProfileId.current = profileId;
      }

      await saveNodes.mutateAsync({
        profileId,
        nodes: nodes
          .filter((n) => n.targetBps > 0)
          .map((n) => ({
            profileId,
            categoryId: n.categoryId,
            targetBps: n.targetBps,
            isLocked: n.isLocked,
            isRequired: true,
          })),
      });

      if (andActivate) {
        await activateProfile.mutateAsync(profileId);
      }

      setHasUnsavedChanges(false);
      onSaved(profileId);
    } catch (err) {
      toast.error(andActivate ? "Failed to activate profile" : "Failed to save profile");
      console.error(err);
    }
  }

  if (saveRef) saveRef.current = () => persistProfile(false);

  return (
    <div className="space-y-5">
      {/* Metadata bar + archive + model — grouped tightly */}
      <div className="space-y-1">
        <div className="space-y-1">
          <div className="bg-muted/20 flex flex-wrap items-center gap-5 rounded-lg border px-5 py-4">
            {/* Profile name */}
            <div className="min-w-40">
              <div className="text-muted-foreground mb-0.5 text-[11px] font-medium uppercase tracking-wider">
                Profile
              </div>
              <input
                value={name}
                onChange={(e) => {
                  setName(e.target.value);
                  setHasUnsavedChanges(true);
                }}
                placeholder="Profile name…"
                className="bg-transparent text-[15px] font-semibold outline-none placeholder:font-normal placeholder:opacity-50"
              />
            </div>

            <div className="bg-border h-8 w-px" />

            {/* Total weight */}
            <div>
              <div className="text-muted-foreground mb-0.5 text-[11px] font-medium uppercase tracking-wider">
                Total weight
              </div>
              <span
                className={cn(
                  "text-[14px] font-semibold tabular-nums",
                  totalBps === 10000 ? "text-green-700 dark:text-green-400" : "text-destructive",
                )}
              >
                {(totalBps / 100).toFixed(1)}%{totalBps === 10000 && " ✓"}
              </span>
            </div>

            <div className="bg-border h-8 w-px" />

            {/* Drift band */}
            <div>
              <div className="text-muted-foreground mb-0.5 text-[11px] font-medium uppercase tracking-wider">
                Drift band
              </div>
              <span className="text-foreground text-[14px] font-medium tabular-nums">
                ±{driftBandPct.toFixed(1)}%
              </span>
            </div>

            <div className="bg-border h-8 w-px" />

            {/* Method */}
            <div>
              <div className="text-muted-foreground mb-0.5 text-[11px] font-medium uppercase tracking-wider">
                Method
              </div>
              <span className="text-foreground text-[14px] font-medium capitalize">
                {triggerType === "threshold" ? "Threshold" : "Manual"}
              </span>
            </div>

            <div className="bg-border h-8 w-px" />

            {/* Taxonomy (display only) */}
            <div>
              <div className="text-muted-foreground mb-0.5 text-[11px] font-medium uppercase tracking-wider">
                Taxonomy
              </div>
              <span className="text-foreground text-[14px] font-medium">Asset classes</span>
            </div>

            <div className="ml-auto flex items-center gap-2">
              <Button variant="ghost" size="sm" onClick={onCancel}>
                Discard
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled={!isValid || isSaving}
                onClick={() => persistProfile(false)}
              >
                {isSaving
                  ? "Saving…"
                  : profile?.status === "active"
                    ? "Save changes"
                    : "Save draft"}
              </Button>
              <Button
                size="sm"
                disabled={!isValid || isActivating}
                onClick={() => persistProfile(true)}
              >
                {isActivating ? "Activating…" : "Activate"}
              </Button>
            </div>
          </div>

          {/* Archive / delete actions — below metadata bar */}
          {(onArchive && profile?.status === "active") || onDelete ? (
            <div className="flex justify-end gap-1">
              {onArchive && profile?.status === "active" && (
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-muted-foreground hover:text-foreground text-[12px]"
                  onClick={onArchive}
                >
                  <Icons.FileArchive className="mr-1.5 h-4 w-4" />
                  Archive profile
                </Button>
              )}
              {onDelete && (
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-destructive/60 hover:text-destructive text-[12px]"
                  onClick={() => setDeleteOpen(true)}
                >
                  <Icons.Trash2 className="mr-1.5 h-4 w-4" />
                  Delete profile
                </Button>
              )}
            </div>
          ) : null}

          <AlertDialog open={deleteOpen} onOpenChange={setDeleteOpen}>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>Delete profile?</AlertDialogTitle>
                <AlertDialogDescription>
                  This will permanently delete &ldquo;{name}&rdquo; and all its target weights. This
                  action cannot be undone.
                </AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>Cancel</AlertDialogCancel>
                <AlertDialogAction
                  className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                  onClick={onDelete}
                >
                  Delete
                </AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
        </div>

        {/* Model presets */}
        <div>
          <div className="text-muted-foreground mb-0.5 pl-0.5 text-[11px] font-medium uppercase tracking-wider">
            Model
          </div>
          <p className="text-muted-foreground mb-3 pl-0.5 text-[12px]">
            Start from a known mix or roll your own
          </p>
          <ModelPresetPicker
            selected={selectedPreset}
            onSelect={handlePresetSelect}
            currentCategories={currentCategoriesForPicker}
            portfolioStats={portfolioStats}
          />
        </div>
      </div>

      {/* Target weights + Drift tolerance */}
      <div className="grid grid-cols-1 gap-5 md:grid-cols-[3fr_2fr]">
        {/* Target weights */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base">Target weights</CardTitle>
            <CardDescription>
              Edit a row — unlocked categories auto-adjust. Click the lock to pin.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <TargetNodeEditor
              categories={topLevelCategories}
              nodes={nodes}
              currentAllocation={currentAllocation}
              onChange={(n) => {
                setNodes(n);
                setHasUnsavedChanges(true);
              }}
            />
          </CardContent>
        </Card>

        {/* Drift tolerance */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base">Drift tolerance</CardTitle>
            <CardDescription>How far a sleeve can wander before flagging</CardDescription>
          </CardHeader>
          <CardContent className="space-y-5">
            {/* Band slider */}
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-[13px] font-medium">Tolerance band</span>
                <span className="text-foreground text-[13px] font-semibold tabular-nums">
                  ±{driftBandPct.toFixed(1)}%
                </span>
              </div>
              <input
                type="range"
                min={0.5}
                max={10}
                step={0.5}
                value={driftBandPct}
                onChange={(e) => {
                  setDriftBandPct(parseFloat(e.target.value));
                  setHasUnsavedChanges(true);
                }}
                className="accent-foreground w-full"
              />
              <div className="text-muted-foreground flex justify-between text-[10px]">
                <span>Tight (1%)</span>
                <span>Standard (5%)</span>
                <span>Loose (10%)</span>
              </div>
            </div>

            {/* Rebalance method */}
            <div className="space-y-2">
              <span className="text-[13px] font-medium">Rebalance method</span>
              <div className="space-y-1.5">
                {TRIGGER_OPTIONS.map((opt) => (
                  <label
                    key={opt.value}
                    className={cn(
                      "flex cursor-pointer items-start gap-3 rounded-md border p-3 transition-colors",
                      triggerType === opt.value
                        ? "border-foreground bg-muted/30"
                        : "hover:border-muted-foreground/40 border-border",
                    )}
                  >
                    <input
                      type="radio"
                      name="trigger"
                      value={opt.value}
                      checked={triggerType === opt.value}
                      onChange={() => {
                        setTriggerType(opt.value);
                        setHasUnsavedChanges(true);
                      }}
                      className="mt-0.5"
                    />
                    <div className="flex-1">
                      <div className="flex items-center gap-2">
                        <span className="text-[13px] font-medium">{opt.label}</span>
                        <span className="text-muted-foreground bg-muted rounded px-1.5 py-0.5 text-[10px]">
                          {opt.badge}
                        </span>
                      </div>
                      <p className="text-muted-foreground mt-0.5 text-[12px]">{opt.description}</p>
                    </div>
                  </label>
                ))}
              </div>
            </div>

            {/* Rebalance to */}
            <div className="space-y-2">
              <span className="text-[13px] font-medium">Rebalance to</span>
              <div className="space-y-1.5">
                {REBALANCE_TO_OPTIONS.map((opt) => (
                  <label
                    key={opt.value}
                    className={cn(
                      "flex cursor-pointer items-start gap-3 rounded-md border p-3 transition-colors",
                      rebalanceTo === opt.value
                        ? "border-foreground bg-muted/30"
                        : "hover:border-muted-foreground/40 border-border",
                    )}
                  >
                    <input
                      type="radio"
                      name="rebalanceTo"
                      value={opt.value}
                      checked={rebalanceTo === opt.value}
                      onChange={() => {
                        setRebalanceTo(opt.value);
                        setHasUnsavedChanges(true);
                      }}
                      className="mt-0.5"
                    />
                    <div className="flex-1">
                      <span className="text-[13px] font-medium">{opt.label}</span>
                      <p className="text-muted-foreground mt-0.5 text-[12px]">{opt.description}</p>
                    </div>
                  </label>
                ))}
              </div>
            </div>

            {/* Allow sells */}
            <div className="flex items-center justify-between rounded-md border p-3">
              <div>
                <p className="text-[13px] font-medium">Allow sells</p>
                <p className="text-muted-foreground text-[12px]">
                  Sell overweight sleeves when rebalancing
                </p>
              </div>
              <Switch
                checked={allowSells}
                onCheckedChange={(v) => {
                  setAllowSells(v);
                  setHasUnsavedChanges(true);
                }}
              />
            </div>

            {/* Whole shares only */}
            <div className="flex items-center justify-between rounded-md border p-3">
              <div>
                <p className="text-[13px] font-medium">Whole shares only</p>
                <p className="text-muted-foreground text-[12px]">
                  Round trade quantities to whole units
                </p>
              </div>
              <Switch
                checked={wholeSharesOnly}
                onCheckedChange={(v) => {
                  setWholeSharesOnly(v);
                  setHasUnsavedChanges(true);
                }}
              />
            </div>

            {/* Minimum trade amount */}
            <div className="flex items-center justify-between rounded-md border p-3">
              <div>
                <p className="text-[13px] font-medium">Min. trade amount</p>
                <p className="text-muted-foreground text-[12px]">Skip trades below this value</p>
              </div>
              <div className="flex items-center gap-1">
                <span className="text-muted-foreground text-[13px]">{baseCurrency}</span>
                <input
                  type="number"
                  min={0}
                  step={1}
                  value={minTradeAmount}
                  onChange={(e) => {
                    setMinTradeAmount(e.target.value);
                    setHasUnsavedChanges(true);
                  }}
                  className="w-20 bg-transparent text-right text-[13px] font-medium tabular-nums outline-none"
                />
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
