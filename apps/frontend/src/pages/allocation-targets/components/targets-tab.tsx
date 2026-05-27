import { useState, useEffect } from "react";
import { Button, Icons, Skeleton } from "@wealthfolio/ui";

import { useTaxonomy } from "@/hooks/use-taxonomies";
import { usePortfolioAllocations } from "@/hooks/use-portfolio-allocations";
import { useSettings } from "@/hooks/use-settings";
import { useArchiveTargetProfile, useDeleteTargetProfile } from "../hooks/use-target-mutations";
import { usePortfolioStats } from "../hooks/use-portfolio-stats";
import type { TargetProfile, AccountScope, TargetScopeType } from "@/lib/types";
import { CurrentAllocationBar } from "./current-allocation-bar";
import { ModelPresetPicker } from "./model-preset-picker";
import { ProfileEditor } from "./profile-editor";

type EditorMode =
  | { kind: "onboarding" }
  | { kind: "edit"; profileId: string | null; presetId: string | null };

function defaultScopeFromAccountScope(scope: AccountScope): {
  scopeType: TargetScopeType;
  scopeId: string | null;
} {
  if (scope.type === "account") return { scopeType: "account", scopeId: scope.accountId };
  if (scope.type === "portfolio") return { scopeType: "portfolio", scopeId: scope.portfolioId };
  return { scopeType: "all", scopeId: null };
}

interface TargetsTabProps {
  profiles: TargetProfile[];
  selectedProfileId: string | null;
  onProfileChange: (id: string) => void;
  newProfileTrigger?: number;
  accountScope: AccountScope;
}

export function TargetsTab({
  profiles,
  selectedProfileId,
  onProfileChange,
  newProfileTrigger,
  accountScope,
}: TargetsTabProps) {
  const [mode, setMode] = useState<EditorMode>(
    profiles.length === 0
      ? { kind: "onboarding" }
      : { kind: "edit", profileId: selectedProfileId, presetId: null },
  );
  const [selectedPreset, setSelectedPreset] = useState<string | null>(null);

  const { data: taxonomy, isLoading: taxonomyLoading } = useTaxonomy("asset_classes");
  const { allocations, isLoading: allocationsLoading } = usePortfolioAllocations(accountScope);
  const { data: settings } = useSettings();

  const archiveProfile = useArchiveTargetProfile();
  const deleteProfile = useDeleteTargetProfile();
  const { stats: portfolioStats } = usePortfolioStats(accountScope);

  const currentCategories = allocations?.assetClasses?.categories ?? [];
  const topLevelCurrent = currentCategories.filter((c) => !c.children?.length || c.percentage > 0);
  const currentAllocationMap = Object.fromEntries(
    topLevelCurrent.map((c) => [c.categoryId, c.percentage]),
  );

  const baseCurrency = settings?.baseCurrency ?? "USD";

  // Sync editor when selected profile changes from header dropdown
  useEffect(() => {
    if (selectedProfileId && mode.kind === "edit" && mode.profileId !== selectedProfileId) {
      setMode({ kind: "edit", profileId: selectedProfileId, presetId: null });
    }
  }, [selectedProfileId]); // eslint-disable-line react-hooks/exhaustive-deps

  // Trigger new profile flow from header dropdown "+ New profile"
  useEffect(() => {
    if (newProfileTrigger && newProfileTrigger > 0) {
      setSelectedPreset(null);
      setMode({ kind: "onboarding" });
    }
  }, [newProfileTrigger]);

  const editingProfile =
    mode.kind === "edit" && mode.profileId
      ? (profiles.find((p) => p.id === mode.profileId) ?? null)
      : null;

  function handlePresetSelect(presetId: string) {
    setSelectedPreset(presetId);
    setMode({ kind: "edit", profileId: null, presetId });
  }

  function handleStartScratch() {
    setSelectedPreset(null);
    setMode({ kind: "edit", profileId: null, presetId: null });
  }

  function handleStartFromCurrent() {
    setSelectedPreset("current");
    setMode({ kind: "edit", profileId: null, presetId: "current" });
  }

  function handleEditorSaved(profileId: string) {
    onProfileChange(profileId);
    setMode({ kind: "edit", profileId, presetId: null });
  }

  function handleEditorCancel() {
    if (profiles.length === 0) {
      setMode({ kind: "onboarding" });
    } else {
      setMode({ kind: "edit", profileId: selectedProfileId, presetId: null });
    }
  }

  function handleEditorDelete() {
    if (!editingProfile) return;
    deleteProfile.mutate(editingProfile.id, {
      onSuccess: () => setMode({ kind: "onboarding" }),
    });
  }

  if (taxonomyLoading || allocationsLoading) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-40 w-full" />
        <Skeleton className="h-32 w-full" />
      </div>
    );
  }

  if (!taxonomy) return null;

  // ── Onboarding (no profiles yet, or creating new) ────────────────────────────
  if (mode.kind === "onboarding") {
    return (
      <div className="space-y-6">
        {profiles.length > 0 && (
          <div className="flex items-center justify-between">
            <h2 className="text-foreground text-[13px] font-semibold">New profile</h2>
            <Button variant="ghost" size="sm" onClick={handleEditorCancel}>
              <Icons.X className="mr-1.5 h-4 w-4" />
              Cancel
            </Button>
          </div>
        )}

        {/* Hero */}
        <div className="grid grid-cols-1 gap-6 rounded-lg border p-6 md:grid-cols-2">
          <div>
            <div className="text-muted-foreground mb-3 text-[11px] font-medium uppercase tracking-wider">
              Get started
            </div>
            <h2 className="text-foreground text-xl font-semibold">
              Set targets for your portfolio
            </h2>
            <p className="text-muted-foreground mt-2 text-[13px] leading-relaxed">
              Define a target mix and Wealthfolio will track how far you drift from it. Start from
              your current allocation, pick a known model, or build one from scratch.
            </p>
            <div className="mt-4 flex flex-wrap gap-2">
              <Button size="sm" onClick={handleStartFromCurrent}>
                Start from current allocation
              </Button>
              <Button size="sm" variant="outline" onClick={handleStartScratch}>
                Build from scratch
              </Button>
            </div>
            <p className="text-muted-foreground mt-3 text-[11px]">
              Drafts auto-save. Targets only apply once you activate the profile.
            </p>
          </div>
          <div>
            <div className="text-muted-foreground mb-2 text-[10px] font-medium uppercase tracking-wider">
              Your current allocation
            </div>
            {topLevelCurrent.length > 0 ? (
              <CurrentAllocationBar categories={topLevelCurrent} />
            ) : (
              <p className="text-muted-foreground text-[12px]">No holdings found.</p>
            )}
          </div>
        </div>

        {/* Model presets */}
        <div>
          <h3 className="text-foreground mb-1 text-[13px] font-semibold">
            Start from a known model
          </h3>
          <p className="text-muted-foreground mb-3 text-[12px]">
            Pick one — you can edit weights after.
          </p>
          <ModelPresetPicker
            selected={selectedPreset}
            onSelect={handlePresetSelect}
            currentCategories={topLevelCurrent}
            portfolioStats={portfolioStats}
          />
        </div>
      </div>
    );
  }

  // ── Editor ────────────────────────────────────────────────────────────────────
  const defaultScope = editingProfile ? null : defaultScopeFromAccountScope(accountScope);

  return (
    <ProfileEditor
      profile={editingProfile}
      taxonomy={taxonomy}
      initialPresetId={mode.presetId}
      currentAllocation={currentAllocationMap}
      baseCurrency={baseCurrency}
      portfolioStats={portfolioStats}
      defaultScopeType={defaultScope?.scopeType}
      defaultScopeId={defaultScope?.scopeId}
      onSaved={handleEditorSaved}
      onCancel={handleEditorCancel}
      onArchive={
        editingProfile?.status === "active"
          ? () => archiveProfile.mutate(editingProfile.id)
          : undefined
      }
      onDelete={editingProfile ? handleEditorDelete : undefined}
    />
  );
}
