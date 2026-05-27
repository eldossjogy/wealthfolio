import { useState, useEffect, useMemo, useRef } from "react";
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
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
  EmptyPlaceholder,
  Icons,
  Skeleton,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@wealthfolio/ui";
import { AccountScopeSelector } from "@/components/account-filter-selector";
import { cn, formatAmount } from "@/lib/utils";
import type { AccountScope, TargetProfile } from "@/lib/types";
import { useTargetProfiles } from "./hooks/use-target-profiles";
import { useTargetDrift } from "./hooks/use-target-drift";
import { useYtdPerformance } from "./hooks/use-ytd-performance";
import { OverviewTab } from "./components/overview-tab";
import { TargetsTab } from "./components/targets-tab";

const DEFAULT_SCOPE: AccountScope = { type: "all" };

function KpiItem({
  label,
  value,
  sub,
  inlineSub,
  tone,
}: {
  label: string;
  value: string;
  sub?: string;
  inlineSub?: string;
  tone?: "warn" | "ok";
}) {
  return (
    <div className="text-left">
      <div className="text-muted-foreground text-[11px] uppercase tracking-wider">{label}</div>
      <div
        className={cn(
          "mt-0.5 flex items-baseline gap-1.5 text-[22px] font-semibold tabular-nums leading-none",
          tone === "warn" && "text-destructive",
          tone === "ok" && "text-green-700 dark:text-green-400",
          !tone && "text-foreground",
        )}
      >
        {value}
        {inlineSub && <span className="text-[14px] font-medium opacity-80">{inlineSub}</span>}
      </div>
      {sub && <div className="text-muted-foreground text-[12px] tabular-nums">{sub}</div>}
    </div>
  );
}

function filterProfilesByScope(profiles: TargetProfile[], scope: AccountScope): TargetProfile[] {
  if (scope.type === "all") return profiles.filter((p) => p.scopeType === "all");
  if (scope.type === "account")
    return profiles.filter((p) => p.scopeType === "account" && p.scopeId === scope.accountId);
  if (scope.type === "portfolio")
    return profiles.filter((p) => p.scopeType === "portfolio" && p.scopeId === scope.portfolioId);
  // "accounts" (multi-account ad-hoc) → no dedicated profile scope type; show all-portfolio profiles
  return profiles.filter((p) => p.scopeType === "all");
}

function scopeKey(scope: AccountScope): string {
  if (scope.type === "all") return "all";
  if (scope.type === "account") return `account:${scope.accountId}`;
  if (scope.type === "portfolio") return `portfolio:${scope.portfolioId}`;
  return `accounts:${[...scope.accountIds].sort().join(",")}`;
}

export function AllocationTargetsPage() {
  const [accountScope, setAccountScope] = useState<AccountScope>(DEFAULT_SCOPE);
  const { profiles, isLoading: profilesLoading } = useTargetProfiles();
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState("overview");
  const [newProfileTrigger, setNewProfileTrigger] = useState(0);
  const [editorHasUnsavedChanges, setEditorHasUnsavedChanges] = useState(false);
  const [pendingTab, setPendingTab] = useState<string | null>(null);
  const [navigateToTabOnSave, setNavigateToTabOnSave] = useState<string | null>(null);
  const editorSaveRef = useRef<(() => void) | null>(null);

  function handleTabChange(tab: string) {
    if (editorHasUnsavedChanges && activeTab === "targets" && tab !== "targets") {
      setPendingTab(tab);
    } else {
      setActiveTab(tab);
    }
  }

  function handleProfileChange(id: string) {
    setSelectedProfileId(id);
    if (navigateToTabOnSave) {
      setActiveTab(navigateToTabOnSave);
      setNavigateToTabOnSave(null);
    }
  }

  // Reset profile selection when scope changes
  useEffect(() => {
    setSelectedProfileId(null);
  }, [accountScope]);

  const scopedProfiles = useMemo(
    () => filterProfilesByScope(profiles, accountScope),
    [profiles, accountScope],
  );

  // Non-archived profiles drive empty-state checks and default selection
  const scopedLiveProfiles = useMemo(
    () => scopedProfiles.filter((p) => p.status !== "archived"),
    [scopedProfiles],
  );

  const scopedActiveProfile =
    scopedLiveProfiles.find((p) => p.status === "active") ?? scopedLiveProfiles[0] ?? null;

  const effectiveProfileId = selectedProfileId ?? scopedActiveProfile?.id ?? null;
  const effectiveProfile = profiles.find((p) => p.id === effectiveProfileId) ?? null;

  const { driftReport, isLoading: driftLoading } = useTargetDrift(effectiveProfileId, accountScope);

  const isLoading = profilesLoading || driftLoading;
  const bandPct = effectiveProfile ? (effectiveProfile.driftBandBps / 100).toFixed(1) : "—";
  const totalTargeted = driftReport
    ? driftReport.rows.filter((r) => r.status !== "not_targeted").length
    : 0;

  const { ytd, isLoading: ytdLoading } = useYtdPerformance(
    accountScope,
    driftReport?.totalValue ?? 0,
  );

  return (
    <div className="flex h-full flex-col">
      {/* Page header */}
      <div className="border-b px-6 pb-0 pt-6">
        <div className="flex items-start justify-between gap-6">
          <div>
            <h1 className="text-foreground text-2xl font-semibold tracking-tight">
              Allocation targets
            </h1>
            <p className="text-muted-foreground mt-1 text-[13px]">
              Define a target mix and track drift against your current holdings.
            </p>
          </div>

          {/* KPIs only */}
          <div className="flex items-center gap-10">
            {driftReport ? (
              <>
                <KpiItem
                  label="Portfolio"
                  value={formatAmount(driftReport.totalValue, driftReport.baseCurrency)}
                />
                {ytd && !ytdLoading && (
                  <KpiItem
                    label="YTD"
                    value={`${ytd.gainAmount >= 0 ? "+" : ""}${formatAmount(ytd.gainAmount, driftReport.baseCurrency)}`}
                    inlineSub={
                      ytd.gainPct !== null
                        ? `${ytd.gainPct >= 0 ? "+" : ""}${ytd.gainPct.toFixed(2)}%`
                        : undefined
                    }
                    tone={ytd.gainAmount >= 0 ? "ok" : "warn"}
                  />
                )}
                <KpiItem
                  label="Max drift"
                  value={`${driftReport.maxDriftBps > 0 ? "+" : ""}${(driftReport.maxDriftBps / 100).toFixed(2)}%`}
                  tone={
                    Math.abs(driftReport.maxDriftBps) > (effectiveProfile?.driftBandBps ?? 0)
                      ? "warn"
                      : undefined
                  }
                />
                <KpiItem
                  label="Out of band"
                  value={`${driftReport.outOfBandCount} / ${totalTargeted}`}
                  tone={driftReport.outOfBandCount > 0 ? "warn" : "ok"}
                />
                <KpiItem label="Drift band" value={`±${bandPct}%`} />
              </>
            ) : isLoading ? (
              <div className="flex items-center gap-10">
                {[1, 2, 3, 4].map((i) => (
                  <Skeleton key={i} className="h-9 w-20" />
                ))}
              </div>
            ) : null}
          </div>
        </div>

        {/* Tabs + selectors on same row */}
        <div className="mt-14">
          <Tabs value={activeTab} onValueChange={handleTabChange} className="w-full">
            <div className="flex items-end justify-between border-b">
              <TabsList className="h-auto rounded-none bg-transparent p-0">
                <TabsTrigger
                  value="overview"
                  className="data-[state=active]:border-foreground rounded-none border-b-2 border-transparent px-3 pb-3 pt-0 text-[13px] font-medium data-[state=active]:bg-transparent data-[state=active]:shadow-none"
                >
                  Overview
                </TabsTrigger>
                <TabsTrigger
                  value="targets"
                  className="data-[state=active]:border-foreground rounded-none border-b-2 border-transparent px-3 pb-3 pt-0 text-[13px] font-medium data-[state=active]:bg-transparent data-[state=active]:shadow-none"
                >
                  Targets
                </TabsTrigger>
                <TabsTrigger
                  value="rebalance"
                  className="data-[state=active]:border-foreground rounded-none border-b-2 border-transparent px-3 pb-3 pt-0 text-[13px] font-medium data-[state=active]:bg-transparent data-[state=active]:shadow-none"
                >
                  Rebalance
                </TabsTrigger>
              </TabsList>

              {/* Profile + scope selectors aligned with tabs */}
              <div className="flex items-center gap-2 pb-2">
                {scopedProfiles.length > 0 && (
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button
                        variant="outline"
                        size="sm"
                        className="bg-secondary/30 hover:bg-muted/80 flex items-center gap-1.5 rounded-full border-none text-sm font-medium"
                      >
                        <Icons.Target className="h-4 w-4 shrink-0 opacity-70" />
                        <span>{effectiveProfile?.name ?? "Select profile"}</span>
                        {effectiveProfile?.status === "active" && (
                          <span className="rounded-full bg-green-100 px-1.5 py-0.5 text-[10px] font-medium text-green-700 dark:bg-green-900/30 dark:text-green-400">
                            Active
                          </span>
                        )}
                        <Icons.ChevronsUpDown className="h-3 w-3 shrink-0 opacity-50" />
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end" className="w-52">
                      <DropdownMenuLabel>Target profiles</DropdownMenuLabel>
                      <DropdownMenuSeparator />
                      {scopedProfiles.map((p) => (
                        <DropdownMenuItem
                          key={p.id}
                          onSelect={() => setSelectedProfileId(p.id)}
                          className={cn(
                            effectiveProfileId === p.id && "font-medium",
                            p.status === "archived" && "opacity-50",
                          )}
                        >
                          <span className="flex-1">{p.name}</span>
                          {p.status === "active" && (
                            <span className="text-[10px] text-green-600 dark:text-green-400">
                              Active
                            </span>
                          )}
                          {p.status === "archived" && (
                            <span className="text-muted-foreground text-[10px]">Archived</span>
                          )}
                        </DropdownMenuItem>
                      ))}
                      <DropdownMenuSeparator />
                      <DropdownMenuItem
                        onSelect={() => {
                          setActiveTab("targets");
                          setNewProfileTrigger((n) => n + 1);
                        }}
                      >
                        <Icons.PlusCircle className="mr-2 h-4 w-4" />
                        New profile
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                )}
                <AccountScopeSelector value={accountScope} onChange={setAccountScope} />
              </div>
            </div>

            <div className="px-6 pb-6 pt-10">
              <TabsContent value="overview" className="m-0">
                {scopedLiveProfiles.length === 0 && !isLoading ? (
                  <EmptyPlaceholder
                    icon={<Icons.Target className="text-muted-foreground h-10 w-10" />}
                    title="No target profile yet"
                    description="Create a target profile to start tracking drift against your holdings."
                  >
                    <Button size="sm" onClick={() => setActiveTab("targets")}>
                      Create a target profile
                    </Button>
                  </EmptyPlaceholder>
                ) : !driftReport && isLoading ? (
                  <div className="space-y-5">
                    <div className="grid grid-cols-2 gap-5">
                      <Skeleton className="h-64 w-full" />
                      <Skeleton className="h-64 w-full" />
                    </div>
                    <Skeleton className="h-48 w-full" />
                  </div>
                ) : !driftReport ? (
                  <EmptyPlaceholder
                    icon={<Icons.PieChart className="text-muted-foreground h-10 w-10" />}
                    title="No targets set"
                    description="Set your target weights on the Targets tab to see drift data."
                  />
                ) : (
                  <OverviewTab
                    report={driftReport}
                    driftBandBps={effectiveProfile?.driftBandBps ?? 0}
                    accountScope={accountScope}
                    onRebalanceClick={() => setActiveTab("rebalance")}
                  />
                )}
              </TabsContent>

              <TabsContent value="targets" className="m-0">
                {profilesLoading ? (
                  <div className="space-y-4">
                    <Skeleton className="h-40 w-full" />
                    <Skeleton className="h-32 w-full" />
                  </div>
                ) : (
                  <TargetsTab
                    key={scopeKey(accountScope)}
                    profiles={scopedProfiles}
                    selectedProfileId={effectiveProfileId}
                    onProfileChange={handleProfileChange}
                    newProfileTrigger={newProfileTrigger}
                    accountScope={accountScope}
                    onUnsavedChange={setEditorHasUnsavedChanges}
                    saveRef={editorSaveRef}
                  />
                )}
              </TabsContent>

              <TabsContent value="rebalance" className="m-0">
                <EmptyPlaceholder
                  icon={<Icons.BarChart className="text-muted-foreground h-10 w-10" />}
                  title="Coming soon"
                  description="Rebalance plan generator will be available in the next milestone."
                />
              </TabsContent>
            </div>
          </Tabs>
        </div>
      </div>

      <AlertDialog
        open={pendingTab !== null}
        onOpenChange={(open) => {
          if (!open) setPendingTab(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Unsaved changes</AlertDialogTitle>
            <AlertDialogDescription>
              You have unsaved changes in your profile. Leaving this tab will discard them.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel onClick={() => setPendingTab(null)}>Stay</AlertDialogCancel>
            <AlertDialogAction
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
              onClick={() => {
                setEditorHasUnsavedChanges(false);
                setActiveTab(pendingTab!);
                setPendingTab(null);
              }}
            >
              Discard changes
            </AlertDialogAction>
            <AlertDialogAction
              onClick={() => {
                setNavigateToTabOnSave(pendingTab!);
                setPendingTab(null);
                editorSaveRef.current?.();
              }}
            >
              Save
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
