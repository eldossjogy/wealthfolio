import { useState } from "react";
import { Button, Icons, Skeleton } from "@wealthfolio/ui";
import { useTaxonomy } from "@/hooks/use-taxonomies";
import { usePortfolioAllocations } from "@/hooks/use-portfolio-allocations";
import { useSettings } from "@/hooks/use-settings";
import { useDeleteTargetProfile, useArchiveTargetProfile } from "../hooks/use-target-mutations";
import type { TargetProfile } from "@/lib/types";
import { cn } from "@/lib/utils";
import { CurrentAllocationBar } from "./current-allocation-bar";
import { ModelPresetPicker } from "./model-preset-picker";
import { ProfileEditor } from "./profile-editor";

type EditorMode =
  | { kind: "none" }
  | { kind: "onboarding" }
  | { kind: "edit"; profileId: string | null; presetId: string | null };

interface TargetsTabProps {
  profiles: TargetProfile[];
  selectedProfileId: string | null;
  onProfileChange: (id: string) => void;
}

export function TargetsTab({ profiles, selectedProfileId, onProfileChange }: TargetsTabProps) {
  const [mode, setMode] = useState<EditorMode>(
    profiles.length === 0 ? { kind: "onboarding" } : { kind: "none" },
  );
  const [selectedPreset, setSelectedPreset] = useState<string | null>(null);

  const { data: taxonomy, isLoading: taxonomyLoading } = useTaxonomy("asset_classes");
  const { allocations, isLoading: allocationsLoading } = usePortfolioAllocations({ type: "all" });
  const { data: settings } = useSettings();

  const deleteProfile = useDeleteTargetProfile();
  const archiveProfile = useArchiveTargetProfile();

  const currentCategories = allocations?.assetClasses?.categories ?? [];
  const topLevelCurrent = currentCategories.filter((c) => !c.children?.length || c.percentage > 0);
  const currentAllocationMap = Object.fromEntries(
    topLevelCurrent.map((c) => [c.categoryId, c.percentage]),
  );

  const baseCurrency = settings?.baseCurrency ?? "USD";

  // Profile being edited
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

  function handleEditProfile(profileId: string) {
    setMode({ kind: "edit", profileId, presetId: null });
  }

  function handleEditorSaved(profileId: string) {
    onProfileChange(profileId);
    setMode({ kind: "none" });
  }

  function handleEditorCancel() {
    setMode(profiles.length === 0 ? { kind: "onboarding" } : { kind: "none" });
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

  // ── Editor mode ──────────────────────────────────────────────────────────────
  if (mode.kind === "edit") {
    return (
      <ProfileEditor
        profile={editingProfile}
        taxonomy={taxonomy}
        initialPresetId={mode.presetId}
        currentAllocation={currentAllocationMap}
        baseCurrency={baseCurrency}
        onSaved={handleEditorSaved}
        onCancel={handleEditorCancel}
      />
    );
  }

  // ── Onboarding (no profiles yet) ─────────────────────────────────────────────
  if (mode.kind === "onboarding") {
    return (
      <div className="space-y-6">
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
          />
        </div>
      </div>
    );
  }

  // ── Profile list (profiles exist, none being edited) ─────────────────────────
  return (
    <div className="space-y-4">
      {/* Profile list */}
      <div className="space-y-2">
        {profiles.map((p) => (
          <div
            key={p.id}
            className={cn(
              "flex items-center justify-between rounded-lg border px-4 py-3",
              selectedProfileId === p.id && "border-foreground/30 bg-muted/30",
            )}
          >
            <div className="flex items-center gap-3">
              <div>
                <div className="text-foreground text-[13px] font-medium">{p.name}</div>
                <div className="text-muted-foreground text-[11px]">
                  Drift band ±{(p.driftBandBps / 100).toFixed(1)}% · {p.triggerType}
                </div>
              </div>
              {p.status === "active" && (
                <span className="rounded-full bg-green-100 px-1.5 py-0.5 text-[10px] font-medium text-green-700 dark:bg-green-900/30 dark:text-green-400">
                  Active
                </span>
              )}
              {p.status === "draft" && (
                <span className="text-muted-foreground bg-muted rounded-full px-1.5 py-0.5 text-[10px] font-medium">
                  Draft
                </span>
              )}
              {p.status === "archived" && (
                <span className="text-muted-foreground bg-muted rounded-full px-1.5 py-0.5 text-[10px] font-medium opacity-60">
                  Archived
                </span>
              )}
            </div>
            <div className="flex items-center gap-1">
              <Button size="sm" variant="ghost" onClick={() => handleEditProfile(p.id)}>
                <Icons.Pencil className="h-4 w-4" />
              </Button>
              {p.status === "active" && (
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => archiveProfile.mutate(p.id)}
                  disabled={archiveProfile.isPending}
                >
                  <Icons.FileArchive className="h-4 w-4" />
                </Button>
              )}
              {p.status !== "active" && (
                <Button
                  size="sm"
                  variant="ghost"
                  className="text-destructive hover:text-destructive"
                  onClick={() => deleteProfile.mutate(p.id)}
                  disabled={deleteProfile.isPending}
                >
                  <Icons.Trash className="h-4 w-4" />
                </Button>
              )}
            </div>
          </div>
        ))}
      </div>

      {/* Add new profile */}
      <Button variant="outline" size="sm" onClick={() => setMode({ kind: "onboarding" })}>
        <Icons.PlusCircle className="mr-1.5 h-4 w-4" />
        New profile
      </Button>
    </div>
  );
}
