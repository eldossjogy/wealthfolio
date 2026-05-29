import { useMemo } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@wealthfolio/ui";
import { cn, formatAmount } from "@/lib/utils";
import { useHoldings } from "@/hooks/use-holdings";
import { useAccounts } from "@/hooks/use-accounts";
import type { AccountScope, DriftReport, DriftRow, Holding } from "@/lib/types";

interface HoldingsTableProps {
  report: DriftReport;
  accountScope: AccountScope;
}

function findDriftRow(holding: Holding, rows: DriftRow[]): DriftRow | undefined {
  if (holding.holdingType === "cash") {
    return rows.find(
      (r) =>
        r.categoryId.toLowerCase().includes("cash") ||
        r.categoryName.toLowerCase().includes("cash"),
    );
  }
  const topCatId = holding.instrument?.classifications?.assetClasses?.[0]?.topLevelCategory?.id;
  if (topCatId) {
    return rows.find((r) => r.categoryId === topCatId);
  }
  return undefined;
}

function driftColor(driftBps: number): string {
  if (driftBps > 0) return "text-destructive";
  if (driftBps < 0) return "text-blue-600 dark:text-blue-400";
  return "text-muted-foreground";
}

export function HoldingsTable({ report, accountScope }: HoldingsTableProps) {
  const { holdings, isLoading } = useHoldings(accountScope);
  const { accounts } = useAccounts();

  const accountMap = useMemo(() => new Map(accounts.map((a) => [a.id, a.name])), [accounts]);

  const showAccountCol = accountScope.type !== "account";

  const rows = useMemo(
    () =>
      holdings.map((h) => {
        const driftRow = findDriftRow(h, report.rows);
        const currentPct = h.weight;
        let targetPct: number | null = null;
        let driftPct: number | null = null;
        if (driftRow) {
          const categoryCurrentPct = driftRow.currentBps / 100;
          const categoryTargetPct = driftRow.targetBps / 100;
          if (categoryCurrentPct > 0) {
            targetPct = (currentPct / categoryCurrentPct) * categoryTargetPct;
            driftPct = currentPct - targetPct;
          }
        }

        const symbol = h.holdingType === "cash" ? h.localCurrency : (h.instrument?.symbol ?? "—");
        const name =
          h.holdingType === "cash" ? `Cash (${h.localCurrency})` : (h.instrument?.name ?? symbol);
        const categoryName = driftRow?.categoryName ?? "—";
        const categoryColor = driftRow?.color ?? null;

        let accountLabel = "—";
        if (h.sourceAccountIds && h.sourceAccountIds.length > 0) {
          const names = h.sourceAccountIds.map((id) => accountMap.get(id) ?? id).filter(Boolean);
          accountLabel =
            names.length === 1 ? names[0] : names.length > 1 ? `${names.length} accounts` : "—";
        } else {
          accountLabel = accountMap.get(h.accountId) ?? h.accountId;
        }

        return {
          id: h.id,
          symbol,
          name,
          categoryName,
          categoryColor,
          value: h.marketValue.base,
          currentPct,
          targetPct,
          driftPct,
          driftBps: driftPct !== null ? Math.round(driftPct * 100) : null,
          accountLabel,
        };
      }),
    [holdings, report.rows, accountMap],
  );

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="text-base">All holdings</CardTitle>
        <CardDescription>
          {isLoading ? "Loading…" : `${holdings.length} positions · ${report.baseCurrency}`}
        </CardDescription>
      </CardHeader>
      <CardContent className="p-0">
        <div className="overflow-x-auto">
          <table className="w-full text-[13px]">
            <thead>
              <tr className="text-muted-foreground border-b text-[10px] uppercase tracking-wider">
                <th className="py-2.5 pl-6 pr-2 text-left font-medium">Ticker</th>
                <th className="py-2.5 pl-2 pr-3 text-left font-medium">Name</th>
                <th className="py-2.5 pr-3 text-left font-medium">Class</th>
                <th className="py-2.5 pr-3 text-right font-medium">Value</th>
                <th className="py-2.5 pr-3 text-right font-medium">Current</th>
                <th className="py-2.5 pr-3 text-right font-medium">Target</th>
                <th className="py-2.5 pr-3 text-right font-medium">Drift</th>
                {showAccountCol && (
                  <th className="py-2.5 pl-2 pr-6 text-right font-medium">Account</th>
                )}
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.id} className="hover:bg-muted/30 h-11 border-b last:border-b-0">
                  <td className="text-foreground pl-6 pr-2 font-mono text-[12px]">{row.symbol}</td>
                  <td className="text-foreground max-w-[220px] truncate pl-2 pr-3">{row.name}</td>
                  <td className="pr-3">
                    <div className="flex items-center gap-1.5">
                      {row.categoryColor && (
                        <span
                          className="h-1.5 w-1.5 shrink-0 rounded-full"
                          style={{ background: row.categoryColor }}
                        />
                      )}
                      <span className="text-muted-foreground text-[12px]">{row.categoryName}</span>
                    </div>
                  </td>
                  <td className="text-foreground pr-3 text-right tabular-nums">
                    {formatAmount(row.value, report.baseCurrency)}
                  </td>
                  <td className="text-foreground pr-3 text-right font-medium tabular-nums">
                    {row.currentPct.toFixed(2)}%
                  </td>
                  <td className="text-muted-foreground pr-3 text-right tabular-nums">
                    {row.targetPct !== null ? `${row.targetPct.toFixed(2)}%` : "—"}
                  </td>
                  <td
                    className={cn(
                      "pr-3 text-right font-medium tabular-nums",
                      row.driftBps !== null ? driftColor(row.driftBps) : "text-muted-foreground",
                    )}
                  >
                    {row.driftPct !== null
                      ? `${row.driftPct > 0 ? "+" : ""}${row.driftPct.toFixed(2)}%`
                      : "—"}
                  </td>
                  {showAccountCol && (
                    <td className="text-muted-foreground pl-2 pr-6 text-right">
                      {row.accountLabel}
                    </td>
                  )}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </CardContent>
    </Card>
  );
}
