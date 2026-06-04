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
} from "@wealthfolio/ui";
import { cn, formatAmount } from "@/lib/utils";
import { toast } from "sonner";
import type {
  AccountScope,
  DriftReport,
  RebalancePlan,
  RebalanceWarning,
  ScenarioMode,
  SuggestedManualTrade,
  AllocationTarget,
} from "@/lib/types";
import {
  allocationTargetColorForRow,
  buildAllocationTargetColorMap,
} from "./allocation-target-colors";
import { accountScopeKey } from "./target-scope";
import { useCalculateRebalancePlan } from "../hooks/use-rebalance";

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

function currencyFractionDigits(code: string): number {
  try {
    return (
      new Intl.NumberFormat(undefined, { style: "currency", currency: code }).resolvedOptions()
        .maximumFractionDigits ?? 2
    );
  } catch {
    return 2;
  }
}

function cashInputLimit(availableCash: number, currency: string): number {
  const factor = 10 ** currencyFractionDigits(currency);
  return Math.round((availableCash + Number.EPSILON) * factor) / factor;
}

function cashValueFromAvailable(availableCash: number, currency: string): string {
  const amount = cashInputLimit(availableCash, currency);
  return amount > 0 ? amount.toFixed(currencyFractionDigits(currency)) : "";
}

function parseCashValue(value: string): number {
  return parseFloat(value.replace(/,/g, "")) || 0;
}

function computeSleeveSummary(driftReport: DriftReport, plan: RebalancePlan) {
  const colorMap = buildAllocationTargetColorMap(driftReport.rows);
  return driftReport.rows
    .filter((r) => r.status !== "not_targeted")
    .map((row, i) => ({
      categoryId: row.categoryId,
      categoryName: row.categoryName,
      color: allocationTargetColorForRow(row, colorMap, i),
      currentBps: row.currentBps,
      targetBps: row.targetBps,
      // Use backend-computed after-bps (accounts for multi-category ETF exposure).
      afterBps: plan.afterBpsByCategory[row.categoryId] ?? row.currentBps,
    }));
}

function csvCell(value: string): string {
  const escaped = value.replace(/"/g, '""');
  return `"${escaped}"`;
}

function exportCsv(plan: RebalancePlan, currency: string, profileName: string) {
  const generated = new Date().toISOString().slice(0, 10);
  const fractionDigits = currencyFractionDigits(currency);

  const meta = [
    ["Generated", generated],
    ["Profile", profileName],
    ["Currency", currency],
    ["Cash deployed", plan.cashUsed.toFixed(fractionDigits)],
    ["Cash available", plan.availableCash.toFixed(fractionDigits)],
    ["Max drift before", fmtBps(plan.maxDriftBpsBefore)],
    ["Max drift after", fmtBps(plan.maxDriftBpsAfter)],
  ]
    .map((row) => row.map(csvCell).join(","))
    .join("\n");

  const header = [
    "Action",
    "Symbol",
    "Name",
    "Category",
    `Amount (${currency})`,
    "Shares",
    `Last Price (${currency})`,
    "Reason",
  ]
    .map(csvCell)
    .join(",");

  const rows = plan.trades.map((t) =>
    [
      t.action,
      t.symbol ?? "",
      t.name ?? "",
      t.categoryName,
      t.estimatedAmount.toFixed(fractionDigits),
      t.quantity != null ? t.quantity.toFixed(t.quantity % 1 === 0 ? 0 : 4) : "",
      t.estimatedPrice != null ? t.estimatedPrice.toFixed(fractionDigits) : "",
      t.reason,
    ]
      .map(csvCell)
      .join(","),
  );

  const csv = [meta, "", header, ...rows].join("\n");
  const blob = new Blob([csv], { type: "text/csv" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `rebalance-plan-${profileName.replace(/[^a-z0-9]/gi, "-").toLowerCase()}-${generated}.csv`;
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
        `${t.action.toUpperCase()}  ${t.symbol ?? t.categoryName}  ${formatAmount(t.estimatedAmount, currency)}` +
        (t.quantity != null ? `  ${t.quantity.toFixed(t.quantity % 1 === 0 ? 0 : 4)} sh` : "") +
        (t.estimatedPrice != null ? ` @ ${formatAmount(t.estimatedPrice, currency)}` : ""),
    ),
  ];
  if (plan.warnings.length) {
    lines.push("", `${plan.warnings.length} warning(s):`);
    plan.warnings.forEach((w) => lines.push(`  · ${w.message}`));
  }
  void navigator.clipboard.writeText(lines.join("\n"));
}

// ── Mode switcher ─────────────────────────────────────────────────────────────

function ModeSwitch({
  currency,
  allowSells,
  value,
  onChange,
}: {
  currency: string;
  allowSells: boolean;
  value: ScenarioMode;
  onChange: (mode: ScenarioMode) => void;
}) {
  const modes: { id: ScenarioMode; label: string; hint: string }[] = [
    {
      id: "cash_flow_only",
      label: "Cash-flow only",
      hint: `deploy new ${currencySymbol(currency)}`,
    },
    { id: "sell_to_rebalance", label: "Sell to rebalance", hint: "sells fund buys" },
    { id: "hybrid", label: "Hybrid", hint: "cash + sells" },
  ];

  return (
    <div className="border-border bg-card inline-flex items-center gap-1 rounded-lg border p-1">
      {modes.map((m) => {
        const disabled = !allowSells && m.id !== "cash_flow_only";
        const active = value === m.id;
        return (
          <button
            key={m.id}
            type="button"
            disabled={disabled}
            onClick={() => !disabled && onChange(m.id)}
            title={disabled ? "Enable 'Allow sells' on this target to use this mode" : undefined}
            className={cn(
              "flex items-center gap-1.5 whitespace-nowrap rounded-md px-3 py-1.5 text-[13px] transition-colors",
              active
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:text-foreground",
              disabled && "cursor-not-allowed opacity-40",
            )}
          >
            <span className="font-medium">{m.label}</span>
            <span
              className={cn("text-[11px]", active ? "text-primary-foreground/65" : "opacity-70")}
            >
              {m.hint}
            </span>
          </button>
        );
      })}
    </div>
  );
}

// ── KPI strip ─────────────────────────────────────────────────────────────────

function KpiStrip({ plan, currency }: { plan: RebalancePlan; currency: string }) {
  const buys = plan.trades.filter((t) => t.action === "buy").length;
  const sells = plan.trades.filter((t) => t.action === "sell").length;
  const tradeLabel =
    sells > 0
      ? `${buys} buy${buys !== 1 ? "s" : ""} · ${sells} sell${sells !== 1 ? "s" : ""}`
      : `${buys} trade${buys !== 1 ? "s" : ""}`;
  const cells = [
    {
      label: "Trades",
      value: String(plan.trades.length),
      sub: tradeLabel,
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
  unclassified_asset: "Unclassified",
  partial_classification: "Partial classification",
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
      <table className="w-full table-fixed text-[13px]">
        <colgroup>
          <col className="w-[6%]" />
          <col className="w-[23%]" />
          <col className="w-[10%]" />
          <col className="w-[13%]" />
          <col className="w-[9%]" />
          <col className="w-[12%]" />
          <col className="w-[27%]" />
        </colgroup>
        <thead>
          <tr className="border-border text-muted-foreground border-b text-[10px] uppercase tracking-wider">
            <th className="py-2.5 pl-5 pr-2 text-left font-medium">Action</th>
            <th className="py-2.5 pr-3 text-left font-medium">Ticker</th>
            <th className="py-2.5 pl-14 pr-3 text-left font-medium">Category</th>
            <th className="py-2.5 pr-3 text-right font-medium">Amount</th>
            <th className="py-2.5 pr-3 text-right font-medium">Shares</th>
            <th className="py-2.5 pr-7 text-right font-medium">Last price</th>
            <th className="py-2.5 pl-10 pr-5 text-left font-medium">Reason</th>
          </tr>
        </thead>
        <tbody>
          {trades.map((t, i) => (
            <tr key={i} className="border-border hover:bg-muted/30 h-12 border-b last:border-b-0">
              <td className="pl-5 pr-2">
                {t.action === "sell" ? (
                  <span className="inline-flex items-center rounded bg-red-100 px-1.5 py-0.5 text-[11px] font-semibold text-red-800 dark:bg-red-900/30 dark:text-red-400">
                    Sell
                  </span>
                ) : (
                  <span className="inline-flex items-center rounded bg-green-100 px-1.5 py-0.5 text-[11px] font-semibold text-green-800 dark:bg-green-900/30 dark:text-green-400">
                    Buy
                  </span>
                )}
              </td>
              <td className="pr-3">
                {t.symbol ? (
                  <>
                    <div className="text-foreground font-mono text-[12px] font-medium">
                      {t.symbol}
                    </div>
                    {t.name && (
                      <div className="text-muted-foreground truncate text-[11px]">{t.name}</div>
                    )}
                  </>
                ) : (
                  <span className="text-muted-foreground">—</span>
                )}
              </td>
              <td className="text-muted-foreground pl-14 pr-3 text-[12px]">{t.categoryName}</td>
              <td className="text-foreground pr-3 text-right font-semibold tabular-nums">
                {formatAmount(t.estimatedAmount, currency)}
              </td>
              <td className="text-muted-foreground pr-3 text-right tabular-nums">
                {t.quantity != null ? t.quantity.toFixed(t.quantity % 1 === 0 ? 0 : 4) : "—"}
              </td>
              <td className="text-muted-foreground pr-7 text-right tabular-nums">
                {t.estimatedPrice != null ? formatAmount(t.estimatedPrice, currency) : "—"}
              </td>
              <td
                className="text-muted-foreground max-w-0 truncate pl-10 pr-5 text-[12px]"
                title={t.reason}
              >
                {t.reason}
              </td>
            </tr>
          ))}
        </tbody>
        <tfoot>
          <tr className="text-[12px]">
            <td colSpan={3} className="text-muted-foreground py-3 pl-5">
              {trades.filter((t) => t.action === "buy").length} buy
              {trades.filter((t) => t.action === "buy").length !== 1 ? "s" : ""}
              {trades.some((t) => t.action === "sell") &&
                ` · ${trades.filter((t) => t.action === "sell").length} sell${trades.filter((t) => t.action === "sell").length !== 1 ? "s" : ""}`}
            </td>
            <td className="text-foreground py-3 pr-3 text-right font-semibold tabular-nums">
              {formatAmount(
                trades.filter((t) => t.action === "buy").reduce((s, t) => s + t.estimatedAmount, 0),
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

// ── Before · Target · After stacked bars ─────────────────────────────────────

type SleeveSummaryRow = ReturnType<typeof computeSleeveSummary>[number];

function BeforeAfterStack({ sleeves }: { sleeves: SleeveSummaryRow[] }) {
  function StackRow({
    label,
    field,
    bold,
  }: {
    label: string;
    field: "currentBps" | "targetBps" | "afterBps";
    bold?: boolean;
  }) {
    return (
      <div className="flex items-center gap-4">
        <span
          className={cn(
            "w-10 shrink-0 text-[12px]",
            bold ? "text-foreground font-semibold" : "text-muted-foreground",
          )}
        >
          {label}
        </span>
        <div className="flex h-6 flex-1 overflow-hidden rounded-md">
          {sleeves.map((s) => {
            const pct = s[field] / 100;
            return (
              <div
                key={s.categoryId}
                className="flex items-center justify-center text-[11px] font-medium text-white/90"
                style={{ width: `${pct}%`, background: s.color }}
                title={`${s.categoryName}: ${pct.toFixed(1)}%`}
              >
                {pct >= 8 ? `${pct.toFixed(0)}%` : ""}
              </div>
            );
          })}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <StackRow label="Now" field="currentBps" />
      <StackRow label="Target" field="targetBps" />
      <StackRow label="After" field="afterBps" bold />
      <div className="flex flex-wrap gap-x-5 gap-y-1.5 pt-2">
        {sleeves.map((s) => (
          <div
            key={s.categoryId}
            className="flex items-center gap-1.5 whitespace-nowrap text-[11px]"
          >
            <span className="h-2 w-2 shrink-0 rounded-sm" style={{ background: s.color }} />
            <span className="text-foreground font-medium">{s.categoryName}</span>
            <span className="text-muted-foreground tabular-nums">
              {(s.currentBps / 100).toFixed(0)}%→{(s.afterBps / 100).toFixed(0)}%
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

// ── Input bar ─────────────────────────────────────────────────────────────────

function InputBar({
  cashValue,
  availableCash,
  currency,
  onCashChange,
  onCalculate,
  hasPlan,
  isCalculating,
  isSourceLoading,
  isSellMode,
}: {
  cashValue: string;
  availableCash: number;
  currency: string;
  onCashChange: (v: string) => void;
  onCalculate: () => void;
  hasPlan: boolean;
  isCalculating: boolean;
  isSourceLoading: boolean;
  isSellMode: boolean;
}) {
  const deploy = parseCashValue(cashValue);
  const availableCashLimit = cashInputLimit(availableCash, currency);
  const overBudget = deploy > availableCashLimit;
  const canCalculate =
    !isCalculating &&
    !isSourceLoading &&
    (isSellMode || (availableCashLimit > 0 && deploy > 0)) &&
    (isSellMode || cashValue.trim().length > 0) &&
    !overBudget;

  return (
    <Card>
      <CardContent className="flex flex-wrap items-end justify-between gap-4 px-5 py-4">
        <div className="flex flex-wrap gap-6">
          {/* Available cash — read-only */}
          <div>
            <div className="text-muted-foreground mb-1.5 text-[11px] font-medium uppercase tracking-wider">
              Cash in scope
            </div>
            <div className="text-foreground text-[15px] font-semibold tabular-nums">
              {availableCash > 0 ? (
                formatAmount(availableCash, currency)
              ) : (
                <span className="text-muted-foreground font-normal">No cash detected</span>
              )}
            </div>
          </div>

          {/* Cash to deploy — editable */}
          <div>
            <label className="text-muted-foreground mb-1.5 block text-[11px] font-medium uppercase tracking-wider">
              Cash to deploy
            </label>
            <div
              className={cn(
                "border-input bg-background focus-within:ring-ring flex h-11 items-center rounded-md border px-3 focus-within:ring-2",
                overBudget && "border-destructive focus-within:ring-destructive",
              )}
            >
              <span className="text-muted-foreground mr-1 text-[15px]">
                {currencySymbol(currency)}
              </span>
              <input
                value={cashValue}
                onChange={(e) => onCashChange(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && canCalculate && onCalculate()}
                disabled={isSourceLoading || availableCash <= 0}
                inputMode="decimal"
                placeholder="0"
                className="text-foreground placeholder:text-muted-foreground/60 disabled:text-muted-foreground w-32 bg-transparent text-[15px] font-medium tabular-nums outline-none disabled:cursor-not-allowed"
              />
            </div>
            {overBudget && (
              <p className="text-destructive mt-1 text-[11px]">Exceeds available cash</p>
            )}
          </div>
        </div>

        <Button onClick={onCalculate} disabled={!canCalculate}>
          <Icons.BarChart className="mr-1.5 h-4 w-4" />
          {isCalculating
            ? "Calculating…"
            : isSourceLoading
              ? "Loading…"
              : hasPlan
                ? "Recalculate"
                : "Calculate plan"}
        </Button>
      </CardContent>
    </Card>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

interface RebalanceTabProps {
  profile: AllocationTarget | null;
  driftReport: DriftReport | null;
  accountScope: AccountScope;
  availableCash: number;
  sourceVersion: string;
  isSourceLoading: boolean;
}

export function RebalanceTab({
  profile,
  driftReport,
  accountScope,
  availableCash,
  sourceVersion,
  isSourceLoading,
}: RebalanceTabProps) {
  const [cashDraft, setCashDraft] = useState<{ key: string; value: string } | null>(null);
  const [scenarioMode, setScenarioMode] = useState<ScenarioMode>("cash_flow_only");
  const [planResult, setPlanResult] = useState<{
    key: string;
    sourceKey: string;
    inputContextKey: string;
    plan: RebalancePlan;
  } | null>(null);

  const calculatePlan = useCalculateRebalancePlan();
  const currency = driftReport?.baseCurrency ?? "USD";
  const inputContextKey = `${profile?.id ?? "no-profile"}:${accountScopeKey(accountScope)}:${currency}`;
  const cashValue =
    cashDraft?.key === inputContextKey
      ? cashDraft.value
      : cashValueFromAvailable(availableCash, currency);
  const cash = parseCashValue(cashValue);
  const availableCashLimit = cashInputLimit(availableCash, currency);
  const sourceReady = !isSourceLoading && !!driftReport;
  const sourceKey = `${inputContextKey}:${availableCash}:${sourceVersion}`;
  const planKey = `${sourceKey}:${cash}:${scenarioMode}`;
  const plan = planResult?.key === planKey ? planResult.plan : null;
  const hasStalePlan =
    !!planResult &&
    planResult.inputContextKey === inputContextKey &&
    planResult.sourceKey !== sourceKey;
  const isSellMode = scenarioMode !== "cash_flow_only";

  function handleCashChange(value: string) {
    setCashDraft({ key: inputContextKey, value });
  }

  function handleCalculate() {
    if (!profile) return;
    if (!sourceReady) {
      toast.error("Portfolio data is still loading");
      return;
    }
    if (availableCashLimit <= 0 && !isSellMode) {
      toast.error("No cash available in scope");
      return;
    }
    if (cash <= 0 && !isSellMode) {
      toast.error("Enter a valid cash amount");
      return;
    }
    if (cash > availableCashLimit) {
      toast.error("Cash to deploy exceeds available cash");
      return;
    }
    calculatePlan.mutate(
      { targetId: profile.id, availableCash: cash, filter: accountScope, scenarioMode },
      {
        onSuccess: (result) =>
          setPlanResult({ key: planKey, sourceKey, inputContextKey, plan: result }),
        onError: (err) => toast.error(`Failed to calculate plan: ${err.message}`),
      },
    );
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
      <div className="flex items-start justify-between gap-4">
        <div>
          <h2 className="text-foreground text-xl font-semibold tracking-tight">Rebalance</h2>
          <p className="text-muted-foreground mt-1 text-[13px]">
            {isSellMode
              ? "Sells overweight positions to fund underweight ones. Tax impact not estimated."
              : "Deploy new cash to pull underweight categories back toward target. Buys only — nothing is sold."}
          </p>
        </div>
        <span className="border-border text-muted-foreground mt-1 shrink-0 rounded border px-2 py-0.5 text-[11px] font-medium">
          Drift planner
        </span>
      </div>

      <ModeSwitch
        currency={currency}
        allowSells={profile?.allowSells ?? false}
        value={scenarioMode}
        onChange={setScenarioMode}
      />

      <InputBar
        cashValue={cashValue}
        availableCash={availableCash}
        currency={currency}
        onCashChange={handleCashChange}
        onCalculate={handleCalculate}
        hasPlan={!!plan || hasStalePlan}
        isCalculating={isCalculating}
        isSourceLoading={!sourceReady}
        isSellMode={isSellMode}
      />

      {hasStalePlan && sourceReady && !isCalculating && (
        <div className="border-border bg-muted/40 text-muted-foreground rounded-lg border px-4 py-3 text-[13px]">
          Portfolio data changed. Recalculate to refresh this plan.
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

      {/* Plan result */}
      {plan && !isCalculating && (
        <div className="space-y-5">
          <KpiStrip plan={plan} currency={currency} />
          <Warnings items={plan.warnings} />

          {/* Sleeve changes */}
          {sleeveSummary.length > 0 && (
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-base">Before · Target · After</CardTitle>
                <CardDescription>
                  How deploying this cash reshapes the portfolio by sleeve
                </CardDescription>
              </CardHeader>
              <CardContent className="px-5 pb-6 pt-2">
                <BeforeAfterStack sleeves={sleeveSummary} />
              </CardContent>
            </Card>
          )}

          {/* Trades */}
          <Card>
            <CardHeader className="pb-2">
              <div className="flex items-start justify-between gap-4">
                <div>
                  <CardTitle className="text-base">Proposed trades</CardTitle>
                  <CardDescription>
                    {(() => {
                      const buys = plan.trades.filter((t) => t.action === "buy").length;
                      const sells = plan.trades.filter((t) => t.action === "sell").length;
                      const sellProceeds = plan.trades
                        .filter((t) => t.action === "sell")
                        .reduce((s, t) => s + t.estimatedAmount, 0);
                      const netDeployed = plan.cashUsed - sellProceeds;
                      return sells > 0
                        ? `${buys} buy${buys !== 1 ? "s" : ""} · ${sells} sell${sells !== 1 ? "s" : ""} · ${formatAmount(netDeployed, currency)} new cash`
                        : `${buys} buy${buys !== 1 ? "s" : ""} · ${formatAmount(plan.cashUsed, currency)} deployed`;
                    })()}
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
        </div>
      )}

      {/* Footer */}
      {plan && !isCalculating && (
        <div className="border-border flex items-center justify-between border-t pt-4">
          <span className="text-muted-foreground text-[11px]">
            Profile: {profile.name} · Calculated{" "}
            {new Date().toLocaleDateString(undefined, {
              year: "numeric",
              month: "short",
              day: "numeric",
            })}
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
            <Button
              size="sm"
              onClick={() => {
                exportCsv(plan, currency, profile.name);
                toast.success("CSV downloaded");
              }}
            >
              <Icons.Download className="mr-1.5 h-4 w-4" />
              Export CSV
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
