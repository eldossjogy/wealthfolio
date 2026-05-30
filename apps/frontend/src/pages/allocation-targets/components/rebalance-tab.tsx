import { useState } from "react";
import {
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
  Icons,
  Skeleton,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@wealthfolio/ui";
import { cn, formatAmount } from "@/lib/utils";
import { toast } from "sonner";
import type {
  AccountScope,
  DriftReport,
  RebalanceDraft,
  RebalancePlan,
  RebalanceWarning,
  SuggestedManualTrade,
  AllocationTarget,
} from "@/lib/types";
import {
  useCalculateRebalancePlan,
  useDeleteRebalanceDraft,
  useRebalanceDrafts,
  useSaveRebalanceDraft,
} from "../hooks/use-rebalance";

// ── Helpers ───────────────────────────────────────────────────────────────────

function fmtBps(bps: number) {
  return `${(bps / 100).toFixed(2)}%`;
}

function currencySymbol(code: string): string {
  try {
    return (
      new Intl.NumberFormat(undefined, { style: "currency", currency: code })
        .formatToParts(0)
        .find((p) => p.type === "currency")?.value ?? code
    );
  } catch {
    return code;
  }
}

function computeSleeveSummary(driftReport: DriftReport, plan: RebalancePlan) {
  const newTotal = driftReport.totalValue + plan.cashUsed;
  return driftReport.rows
    .filter((r) => r.status !== "not_targeted")
    .map((row) => {
      const deployed = plan.trades
        .filter((t) => t.categoryId === row.categoryId)
        .reduce((sum, t) => sum + t.estimatedAmount, 0);
      const afterValue = row.currentValue + deployed;
      const afterBps = newTotal > 0 ? Math.round((afterValue / newTotal) * 10000) : 0;
      return {
        categoryId: row.categoryId,
        categoryName: row.categoryName,
        color: row.color,
        currentBps: row.currentBps,
        targetBps: row.targetBps,
        afterBps,
      };
    });
}

function exportCsv(plan: RebalancePlan, currency: string) {
  const header = "Action,Symbol,Name,Category,Amount,Shares,Est. Price,Reason";
  const rows = plan.trades.map((t) =>
    [
      t.action,
      t.symbol ?? "",
      t.name ?? "",
      t.categoryName,
      t.estimatedAmount.toFixed(2),
      t.quantity != null ? t.quantity.toFixed(4) : "",
      t.estimatedPrice != null ? t.estimatedPrice.toFixed(2) : "",
      `"${t.reason}"`,
    ].join(","),
  );
  const csv = [header, ...rows].join("\n");
  const blob = new Blob([csv], { type: "text/csv" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `rebalance-plan-${currency}.csv`;
  a.click();
  URL.revokeObjectURL(url);
}

function copyToText(plan: RebalancePlan, currency: string) {
  const lines = [
    `Rebalance plan · ${new Date().toLocaleDateString()}`,
    `Cash deployed: ${formatAmount(plan.cashUsed, currency)} of ${formatAmount(plan.availableCash, currency)}`,
    `Max drift: ${fmtBps(plan.maxDriftBpsBefore)} → ${fmtBps(plan.maxDriftBpsAfter)}`,
    "",
    "PROPOSED TRADES",
    ...plan.trades.map(
      (t) =>
        `BUY  ${t.symbol ?? t.categoryName}  ${formatAmount(t.estimatedAmount, currency)}` +
        (t.quantity != null ? `  ${t.quantity.toFixed(t.quantity % 1 === 0 ? 0 : 4)} sh` : "") +
        (t.estimatedPrice != null ? ` @ ${formatAmount(t.estimatedPrice, currency)}` : ""),
    ),
  ];
  if (plan.warnings.length) {
    lines.push("", `${plan.warnings.length} warning(s):`);
    plan.warnings.forEach((w) => lines.push(`  · ${w.message}`));
  }
  navigator.clipboard.writeText(lines.join("\n")).then(() => {});
}

// ── Mode switcher ─────────────────────────────────────────────────────────────

function ModeSwitch() {
  const modes = [
    { id: "full", label: "Full rebalance", hint: "buy + sell", soon: true },
    { id: "cash", label: "Cash-only", hint: "deploy new $", soon: false },
    { id: "min", label: "Minimal trades", hint: "fewest moves", soon: true },
  ] as const;

  return (
    <div className="border-border bg-card inline-flex items-center gap-1 rounded-lg border p-1">
      {modes.map((m) => {
        const active = m.id === "cash";
        return (
          <div
            key={m.id}
            title={m.soon ? "Coming in a later milestone" : undefined}
            className={cn(
              "flex items-center gap-1.5 whitespace-nowrap rounded-md px-3 py-1.5 text-[13px] transition-colors",
              active ? "bg-primary text-primary-foreground" : "text-muted-foreground",
              m.soon && "cursor-not-allowed opacity-50",
            )}
          >
            <span className="font-medium">{m.label}</span>
            <span
              className={cn("text-[11px]", active ? "text-primary-foreground/65" : "opacity-70")}
            >
              {m.hint}
            </span>
            {m.soon && (
              <span className="border-border text-muted-foreground ml-0.5 rounded border px-1 py-px text-[9px] font-medium uppercase tracking-wider">
                soon
              </span>
            )}
          </div>
        );
      })}
    </div>
  );
}

// ── KPI strip ─────────────────────────────────────────────────────────────────

function KpiStrip({ plan, currency }: { plan: RebalancePlan; currency: string }) {
  const cells = [
    {
      label: "Buys",
      value: String(plan.trades.length),
      sub: `${plan.trades.length} trade${plan.trades.length !== 1 ? "s" : ""}`,
    },
    {
      label: "Cash deployed",
      value: formatAmount(plan.cashUsed, currency),
      sub: `of ${formatAmount(plan.availableCash, currency)} available`,
    },
    {
      label: "Unused cash",
      value: formatAmount(plan.cashRemaining, currency),
      sub: "below min trade / lot size",
      muted: true,
    },
    {
      label: "Max drift after",
      value: fmtBps(plan.maxDriftBpsAfter),
      sub: `from ${fmtBps(plan.maxDriftBpsBefore)}`,
      ok: plan.maxDriftBpsAfter < plan.maxDriftBpsBefore,
    },
  ];

  return (
    <div className="border-border bg-card divide-border grid grid-cols-4 divide-x overflow-hidden rounded-lg border">
      {cells.map((c, i) => (
        <div key={i} className="px-5 py-4">
          <div className="text-muted-foreground text-[11px] uppercase tracking-wider">
            {c.label}
          </div>
          <div
            className={cn(
              "mt-1 text-[26px] font-semibold tabular-nums leading-none",
              c.ok
                ? "text-green-700 dark:text-green-400"
                : c.muted
                  ? "text-muted-foreground"
                  : "text-foreground",
            )}
          >
            {c.value}
          </div>
          {c.sub && (
            <div className="text-muted-foreground mt-1 text-[12px] tabular-nums">{c.sub}</div>
          )}
        </div>
      ))}
    </div>
  );
}

// ── Warnings ──────────────────────────────────────────────────────────────────

const WARN_LABEL: Record<string, string> = {
  missing_quote: "Missing quote",
  no_buy_candidate: "No buy candidate",
  whole_share_residue: "Residual cash",
};

function Warnings({ items }: { items: RebalanceWarning[] }) {
  const [open, setOpen] = useState(false);
  if (!items.length) return null;
  return (
    <div className="rounded-lg border border-amber-200 bg-amber-50 dark:border-amber-900 dark:bg-amber-950/30">
      <button
        onClick={() => setOpen((o) => !o)}
        className="flex w-full items-center gap-2 px-4 py-2.5 text-left"
      >
        <Icons.AlertTriangle className="h-4 w-4 shrink-0 text-amber-600 dark:text-amber-400" />
        <span className="flex-1 text-[12px] font-semibold text-amber-800 dark:text-amber-300">
          {items.length} thing{items.length > 1 ? "s" : ""} to know about this plan
        </span>
        <Icons.ChevronDown
          className={cn(
            "h-4 w-4 text-amber-600 transition-transform dark:text-amber-400",
            open && "rotate-180",
          )}
        />
      </button>
      {open && (
        <ul className="divide-y divide-amber-200/60 border-t border-amber-200/70 dark:divide-amber-900/60 dark:border-amber-900/70">
          {items.map((w, i) => (
            <li key={i} className="flex items-start gap-3 px-4 py-2.5">
              <span className="mt-px shrink-0 whitespace-nowrap rounded border border-amber-300 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide text-amber-700 dark:border-amber-700 dark:text-amber-400">
                {WARN_LABEL[w.kind] ?? w.kind}
              </span>
              <span className="text-foreground/80 text-[12px] leading-snug">{w.message}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

// ── Trades table ──────────────────────────────────────────────────────────────

function TradesTable({ trades, currency }: { trades: SuggestedManualTrade[]; currency: string }) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-[13px]">
        <thead>
          <tr className="border-border text-muted-foreground border-b text-[10px] uppercase tracking-wider">
            <th className="py-2.5 pl-5 pr-2 text-left font-medium">Action</th>
            <th className="py-2.5 pr-3 text-left font-medium">Ticker</th>
            <th className="py-2.5 pr-3 text-left font-medium">Category</th>
            <th className="py-2.5 pr-3 text-right font-medium">Amount</th>
            <th className="py-2.5 pr-3 text-right font-medium">Shares</th>
            <th className="py-2.5 pr-3 text-right font-medium">Est. price</th>
            <th className="py-2.5 pr-5 text-left font-medium">Reason</th>
          </tr>
        </thead>
        <tbody>
          {trades.map((t, i) => (
            <tr key={i} className="border-border hover:bg-muted/30 h-12 border-b last:border-b-0">
              <td className="pl-5 pr-2">
                <span className="inline-flex items-center rounded bg-green-100 px-1.5 py-0.5 text-[11px] font-semibold text-green-800 dark:bg-green-900/30 dark:text-green-400">
                  Buy
                </span>
              </td>
              <td className="pr-3">
                {t.symbol ? (
                  <>
                    <div className="text-foreground font-mono text-[12px] font-medium">
                      {t.symbol}
                    </div>
                    {t.name && (
                      <div className="text-muted-foreground max-w-[180px] truncate text-[11px]">
                        {t.name}
                      </div>
                    )}
                  </>
                ) : (
                  <span className="text-muted-foreground">—</span>
                )}
              </td>
              <td className="text-muted-foreground pr-3 text-[12px]">{t.categoryName}</td>
              <td className="text-foreground pr-3 text-right font-semibold tabular-nums">
                {formatAmount(t.estimatedAmount, currency)}
              </td>
              <td className="text-muted-foreground pr-3 text-right tabular-nums">
                {t.quantity != null ? t.quantity.toFixed(t.quantity % 1 === 0 ? 0 : 4) : "—"}
              </td>
              <td className="text-muted-foreground pr-3 text-right tabular-nums">
                {t.estimatedPrice != null ? formatAmount(t.estimatedPrice, currency) : "—"}
              </td>
              <td className="text-muted-foreground pr-5 text-[12px]">{t.reason}</td>
            </tr>
          ))}
        </tbody>
        <tfoot>
          <tr className="text-[12px]">
            <td colSpan={3} className="text-muted-foreground py-3 pl-5">
              Total deployed across {trades.length} buy{trades.length !== 1 ? "s" : ""}
            </td>
            <td className="text-foreground py-3 pr-3 text-right font-semibold tabular-nums">
              {formatAmount(
                trades.reduce((s, t) => s + t.estimatedAmount, 0),
                currency,
              )}
            </td>
            <td colSpan={3} />
          </tr>
        </tfoot>
      </table>
    </div>
  );
}

// ── Sleeve changes ────────────────────────────────────────────────────────────

const BAR_MAX = 3500;

function SleeveBar({
  s,
}: {
  s: {
    categoryName: string;
    color: string;
    currentBps: number;
    targetBps: number;
    afterBps: number;
  };
}) {
  const w = (bps: number) => Math.min(100, (bps / BAR_MAX) * 100);
  const delta = s.afterBps - s.currentBps;
  const tone = delta > 0 ? "text-green-700 dark:text-green-400" : "text-muted-foreground";

  return (
    <div className="grid items-center gap-x-4" style={{ gridTemplateColumns: "150px 1fr 150px" }}>
      <div className="flex min-w-0 items-center gap-2">
        <span className="h-2.5 w-2.5 shrink-0 rounded-sm" style={{ background: s.color }} />
        <span className="text-foreground truncate text-[13px] font-medium">{s.categoryName}</span>
      </div>
      <div className="relative h-7">
        <div className="bg-muted absolute inset-0 rounded" />
        <div
          className="absolute top-1 h-2 rounded-sm opacity-40"
          style={{ background: s.color, width: `${w(s.currentBps)}%` }}
        />
        <div
          className="absolute bottom-1 h-2 rounded-sm"
          style={{ background: s.color, width: `${w(s.afterBps)}%` }}
        />
        <div
          className="bg-foreground/50 absolute inset-y-0 w-0.5"
          style={{ left: `${w(s.targetBps)}%` }}
          title={`target ${fmtBps(s.targetBps)}`}
        />
      </div>
      <div className="flex items-center justify-end gap-2 text-[12px] tabular-nums">
        <span className="text-muted-foreground">{fmtBps(s.currentBps)}</span>
        <Icons.ArrowRight className="text-muted-foreground/60 h-3 w-3" />
        <span className="text-foreground font-semibold">{fmtBps(s.afterBps)}</span>
        <span className={cn("w-12 text-right font-medium", tone)}>
          {delta >= 0 ? "+" : ""}
          {fmtBps(delta)}
        </span>
      </div>
    </div>
  );
}

// ── Draft dropdown ────────────────────────────────────────────────────────────

function DraftDropdown({
  targetId,
  currency,
  onLoad,
}: {
  targetId: string;
  currency: string;
  onLoad: (draft: RebalanceDraft) => void;
}) {
  const { data: drafts = [] } = useRebalanceDrafts(targetId);
  const deleteDraft = useDeleteRebalanceDraft(targetId);

  if (!drafts.length) return null;

  function fmtDate(iso: string) {
    return new Date(iso).toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="outline" size="sm">
          <Icons.FolderOpen className="mr-1.5 h-4 w-4" />
          Load draft
          <Icons.ChevronDown className="ml-1 h-3 w-3 opacity-60" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-64">
        <DropdownMenuLabel>Saved drafts</DropdownMenuLabel>
        <DropdownMenuSeparator />
        {drafts.map((d) => {
          let cashLabel = "";
          try {
            const input = JSON.parse(d.inputJson) as { available_cash?: number };
            if (input.available_cash != null)
              cashLabel = ` · ${currencySymbol(currency)}${input.available_cash.toLocaleString()}`;
          } catch {}
          return (
            <DropdownMenuItem
              key={d.id}
              className="flex items-center justify-between gap-2"
              onSelect={() => onLoad(d)}
            >
              <span className="flex-1 text-[13px]">
                {fmtDate(d.createdAt)}
                {cashLabel}
              </span>
              <button
                className="text-muted-foreground hover:text-destructive"
                onClick={(e) => {
                  e.stopPropagation();
                  deleteDraft.mutate(d.id);
                }}
              >
                <Icons.X className="h-3.5 w-3.5" />
              </button>
            </DropdownMenuItem>
          );
        })}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

// ── Input bar ─────────────────────────────────────────────────────────────────

function InputBar({
  targetId,
  cashValue,
  currency,
  onCashChange,
  onCalculate,
  onSaveDraft,
  onLoadDraft,
  hasPlan,
  isCalculating,
  isSaving,
}: {
  targetId: string;
  cashValue: string;
  currency: string;
  onCashChange: (v: string) => void;
  onCalculate: () => void;
  onSaveDraft: () => void;
  onLoadDraft: (draft: RebalanceDraft) => void;
  hasPlan: boolean;
  isCalculating: boolean;
  isSaving: boolean;
}) {
  return (
    <Card>
      <CardContent className="flex flex-wrap items-end justify-between gap-4 px-5 py-4">
        <div className="min-w-[220px]">
          <label className="text-muted-foreground mb-1.5 block text-[11px] font-medium uppercase tracking-wider">
            Available cash to deploy
          </label>
          <div className="border-input bg-background focus-within:ring-ring flex h-11 items-center rounded-md border px-3 focus-within:ring-2">
            <span className="text-muted-foreground mr-1 text-[15px]">
              {currencySymbol(currency)}
            </span>
            <input
              value={cashValue}
              onChange={(e) => onCashChange(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && onCalculate()}
              inputMode="decimal"
              placeholder="25,000"
              className="text-foreground placeholder:text-muted-foreground/60 w-full bg-transparent text-[15px] font-medium tabular-nums outline-none"
            />
          </div>
        </div>

        <div className="flex items-center gap-2">
          <DraftDropdown targetId={targetId} currency={currency} onLoad={onLoadDraft} />
          {hasPlan && (
            <Button variant="outline" size="sm" onClick={onSaveDraft} disabled={isSaving}>
              <Icons.FileText className="mr-1.5 h-4 w-4" />
              {isSaving ? "Saving…" : "Save as draft"}
            </Button>
          )}
          <Button onClick={onCalculate} disabled={isCalculating || !cashValue.trim()}>
            <Icons.BarChart className="mr-1.5 h-4 w-4" />
            {isCalculating ? "Calculating…" : hasPlan ? "Recalculate" : "Calculate plan"}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

// ── Empty state (V2 style) ────────────────────────────────────────────────────

function EmptyState({
  targetId,
  cashValue,
  currency,
  onCashChange,
  onCalculate,
  onLoadDraft,
  isCalculating,
}: {
  targetId: string;
  cashValue: string;
  currency: string;
  onCashChange: (v: string) => void;
  onCalculate: () => void;
  onLoadDraft: (draft: RebalanceDraft) => void;
  isCalculating: boolean;
}) {
  return (
    <Card>
      <CardContent className="flex flex-col items-center gap-5 px-8 py-14 text-center">
        <div className="bg-muted text-muted-foreground flex h-12 w-12 items-center justify-center rounded-full">
          <Icons.Coins className="h-6 w-6" />
        </div>
        <div>
          <div className="text-foreground text-[17px] font-semibold">
            How much cash do you want to deploy?
          </div>
          <div className="text-muted-foreground mt-1 max-w-sm text-[13px]">
            We'll spread it across your underweight sleeves to cut drift as much as possible —
            without selling a thing.
          </div>
        </div>
        <div className="w-full max-w-xs space-y-3 pt-1">
          <div>
            <label className="text-muted-foreground mb-1.5 block text-[11px] font-medium uppercase tracking-wider">
              Available cash to deploy
            </label>
            <div className="border-input bg-background focus-within:ring-ring flex h-11 items-center rounded-md border px-3 focus-within:ring-2">
              <span className="text-muted-foreground mr-1 text-[15px]">
                {currencySymbol(currency)}
              </span>
              <input
                value={cashValue}
                onChange={(e) => onCashChange(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && onCalculate()}
                inputMode="decimal"
                placeholder="25,000"
                className="text-foreground placeholder:text-muted-foreground/60 w-full bg-transparent text-[15px] font-medium tabular-nums outline-none"
              />
            </div>
          </div>
          <div className="flex gap-2">
            <Button
              className="flex-1"
              onClick={onCalculate}
              disabled={isCalculating || !cashValue.trim()}
            >
              <Icons.BarChart className="mr-1.5 h-4 w-4" />
              {isCalculating ? "Calculating…" : "Calculate plan"}
            </Button>
            <DraftDropdown targetId={targetId} currency={currency} onLoad={onLoadDraft} />
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

// ── Plan impact sidebar ───────────────────────────────────────────────────────

function ImpactRow({
  label,
  sub,
  value,
  last,
}: {
  label: string;
  sub?: string;
  value: React.ReactNode;
  last?: boolean;
}) {
  return (
    <div
      className={cn(
        "flex items-center justify-between gap-4 py-3",
        !last && "border-border/60 border-b",
      )}
    >
      <div>
        <div className="text-foreground whitespace-nowrap text-[13px] font-medium">{label}</div>
        {sub && <div className="text-muted-foreground text-[12px]">{sub}</div>}
      </div>
      <div className="whitespace-nowrap text-right text-[14px] font-semibold tabular-nums">
        {value}
      </div>
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

interface RebalanceTabProps {
  profile: AllocationTarget | null;
  driftReport: DriftReport | null;
  accountScope: AccountScope;
}

export function RebalanceTab({ profile, driftReport, accountScope }: RebalanceTabProps) {
  const [cashValue, setCashValue] = useState("");
  const [plan, setPlan] = useState<RebalancePlan | null>(null);
  const [draftLabel, setDraftLabel] = useState<string | null>(null);

  const calculatePlan = useCalculateRebalancePlan();
  const saveDraft = useSaveRebalanceDraft(profile?.id ?? "");

  const currency = driftReport?.baseCurrency ?? "USD";

  function parseCash(): number {
    return parseFloat(cashValue.replace(/,/g, "")) || 0;
  }

  function handleCalculate() {
    if (!profile) return;
    const cash = parseCash();
    if (cash <= 0) {
      toast.error("Enter a valid cash amount");
      return;
    }
    setDraftLabel(null);
    calculatePlan.mutate(
      { targetId: profile.id, availableCash: cash, filter: accountScope },
      {
        onSuccess: (result) => setPlan(result),
        onError: (err) => toast.error(`Failed to calculate plan: ${err.message}`),
      },
    );
  }

  function handleSaveDraft() {
    if (!plan || !profile) return;
    saveDraft.mutate(
      { availableCash: parseCash(), filter: accountScope, plan },
      {
        onSuccess: () => toast.success("Draft saved"),
        onError: (err) => toast.error(`Failed to save draft: ${err.message}`),
      },
    );
  }

  function handleLoadDraft(draft: RebalanceDraft) {
    try {
      const loadedPlan = JSON.parse(draft.resultJson) as RebalancePlan;
      const loadedInput = JSON.parse(draft.inputJson) as { available_cash?: number };
      if (loadedInput.available_cash != null) {
        setCashValue(String(loadedInput.available_cash));
      }
      setPlan(loadedPlan);
      setDraftLabel(
        new Date(draft.createdAt).toLocaleDateString(undefined, {
          year: "numeric",
          month: "short",
          day: "numeric",
        }),
      );
    } catch {
      toast.error("Failed to load draft");
    }
  }

  if (!profile) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 rounded-lg border border-dashed py-20 text-center">
        <Icons.Target className="text-muted-foreground h-10 w-10" />
        <div className="text-foreground text-[15px] font-semibold">No profile selected</div>
        <div className="text-muted-foreground max-w-sm text-[13px]">
          Select a target profile to calculate a rebalance plan.
        </div>
      </div>
    );
  }

  const sleeveSummary = plan && driftReport ? computeSleeveSummary(driftReport, plan) : [];
  const isCalculating = calculatePlan.isPending;

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h2 className="text-foreground text-xl font-semibold tracking-tight">Rebalance</h2>
        <p className="text-muted-foreground mt-1 text-[13px]">
          Deploy new cash to pull underweight sleeves back toward target. Buys only — nothing is
          sold.
        </p>
      </div>

      <ModeSwitch />

      {/* Input bar — only shown once a plan exists; empty state has its own input */}
      {plan ? (
        <InputBar
          targetId={profile.id}
          cashValue={cashValue}
          currency={currency}
          onCashChange={setCashValue}
          onCalculate={handleCalculate}
          onSaveDraft={handleSaveDraft}
          onLoadDraft={handleLoadDraft}
          hasPlan
          isCalculating={isCalculating}
          isSaving={saveDraft.isPending}
        />
      ) : null}

      {/* Draft loaded label */}
      {draftLabel && (
        <div className="text-muted-foreground flex items-center gap-2 text-[12px]">
          <Icons.FolderOpen className="h-3.5 w-3.5" />
          Loaded from draft · {draftLabel}
        </div>
      )}

      {/* Loading skeletons */}
      {isCalculating && (
        <div className="space-y-5">
          <div className="border-border bg-border grid grid-cols-4 gap-px overflow-hidden rounded-lg border">
            {[0, 1, 2, 3].map((i) => (
              <div key={i} className="bg-card px-5 py-4">
                <Skeleton className="h-3 w-16" />
                <Skeleton className="mt-3 h-6 w-24" />
                <Skeleton className="mt-2 h-3 w-20" />
              </div>
            ))}
          </div>
          <Skeleton className="h-48 w-full" />
          <Skeleton className="h-64 w-full" />
        </div>
      )}

      {/* Empty state */}
      {!plan && !isCalculating && (
        <EmptyState
          targetId={profile.id}
          cashValue={cashValue}
          currency={currency}
          onCashChange={setCashValue}
          onCalculate={handleCalculate}
          onLoadDraft={handleLoadDraft}
          isCalculating={isCalculating}
        />
      )}

      {/* Plan result */}
      {plan && !isCalculating && (
        <div className="space-y-5">
          <KpiStrip plan={plan} currency={currency} />
          <Warnings items={plan.warnings} />

          {/* Trades */}
          <Card>
            <CardHeader className="pb-2">
              <div className="flex items-start justify-between gap-4">
                <div>
                  <CardTitle className="text-base">Proposed trades</CardTitle>
                  <CardDescription>
                    {plan.trades.length} buy{plan.trades.length !== 1 ? "s" : ""} ·{" "}
                    {formatAmount(plan.cashUsed, currency)} deployed
                  </CardDescription>
                </div>
              </div>
            </CardHeader>
            <CardContent className="p-0 pb-1">
              {plan.trades.length > 0 ? (
                <TradesTable trades={plan.trades} currency={currency} />
              ) : (
                <p className="text-muted-foreground px-5 py-4 text-[13px]">
                  No trades — all sleeves are already within band or no holdings found.
                </p>
              )}
            </CardContent>
          </Card>

          {/* Sleeve changes + Plan impact */}
          {sleeveSummary.length > 0 && (
            <div className="grid grid-cols-[3fr_2fr] gap-5">
              <Card>
                <CardHeader className="pb-2">
                  <CardTitle className="text-base">Sleeve changes</CardTitle>
                  <CardDescription>current · target ▎ · after</CardDescription>
                </CardHeader>
                <CardContent className="px-5 pb-6 pt-2">
                  <div className="space-y-3.5">
                    {sleeveSummary.map((s) => (
                      <SleeveBar key={s.categoryId} s={s} />
                    ))}
                  </div>
                </CardContent>
              </Card>

              <Card>
                <CardHeader className="pb-2">
                  <CardTitle className="text-base">Plan impact</CardTitle>
                  <CardDescription>What this plan does to the portfolio</CardDescription>
                </CardHeader>
                <CardContent className="px-5 pb-2 pt-0">
                  <ImpactRow
                    label="Cash deployed"
                    sub={`of ${formatAmount(plan.availableCash, currency)} available`}
                    value={formatAmount(plan.cashUsed, currency)}
                  />
                  <ImpactRow
                    label="Unused cash"
                    sub="below min trade / lot size"
                    value={
                      <span className="text-muted-foreground">
                        {formatAmount(plan.cashRemaining, currency)}
                      </span>
                    }
                  />
                  <ImpactRow label="Buys" value={String(plan.trades.length)} />
                  <ImpactRow
                    label="Max drift"
                    value={
                      <span className="flex items-center gap-1">
                        <span className="text-muted-foreground">
                          {fmtBps(plan.maxDriftBpsBefore)}
                        </span>
                        <Icons.ArrowRight className="text-muted-foreground/60 h-3 w-3" />
                        <span className="text-green-700 dark:text-green-400">
                          {fmtBps(plan.maxDriftBpsAfter)}
                        </span>
                      </span>
                    }
                    last
                  />
                </CardContent>
              </Card>
            </div>
          )}
        </div>
      )}

      {/* Footer */}
      {plan && !isCalculating && (
        <div className="border-border flex items-center justify-between border-t pt-4">
          <span className="text-muted-foreground text-[11px]">
            Prices are estimates. Not financial advice.
          </span>
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                copyToText(plan, currency);
                toast.success("Copied to clipboard");
              }}
            >
              <Icons.Copy className="mr-1.5 h-4 w-4" />
              Copy as text
            </Button>
            <Button variant="outline" size="sm" onClick={() => exportCsv(plan, currency)}>
              <Icons.Download className="mr-1.5 h-4 w-4" />
              Export CSV
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
