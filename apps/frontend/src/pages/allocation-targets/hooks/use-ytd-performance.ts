import { useMemo } from "react";
import { useQueries } from "@tanstack/react-query";
import { calculatePerformanceSummary } from "@/adapters";
import { useAccounts } from "@/hooks/use-accounts";
import { usePortfolios } from "@/hooks/use-portfolios";
import type { AccountScope, Account } from "@/lib/types";

function scopedAccounts(
  accounts: Account[],
  scope: AccountScope,
  portfolioAccountIds?: string[],
): Account[] {
  if (scope.type === "all") return accounts;
  if (scope.type === "account") return accounts.filter((a) => a.id === scope.accountId);
  if (scope.type === "accounts") return accounts.filter((a) => scope.accountIds.includes(a.id));
  if (scope.type === "portfolio" && portfolioAccountIds) {
    return accounts.filter((a) => portfolioAccountIds.includes(a.id));
  }
  return accounts;
}

export interface YtdPerformance {
  gainAmount: number;
  gainPct: number | null;
}

export function useYtdPerformance(
  accountScope: AccountScope,
  totalValue: number,
): { ytd: YtdPerformance | null; isLoading: boolean } {
  const { accounts } = useAccounts();
  const { data: portfolios = [] } = usePortfolios();

  const today = new Date();
  const startDate = `${today.getFullYear()}-01-01`;
  const endDate = today.toISOString().split("T")[0];

  const portfolioAccountIds = useMemo(() => {
    if (accountScope.type !== "portfolio") return undefined;
    return portfolios.find((p) => p.id === accountScope.portfolioId)?.accountIds;
  }, [portfolios, accountScope]);

  const targetAccounts = useMemo(
    () => scopedAccounts(accounts, accountScope, portfolioAccountIds),
    [accounts, accountScope, portfolioAccountIds],
  );

  const queries = useQueries({
    queries: targetAccounts.map((acc) => ({
      queryKey: ["ytd-performance", acc.id, startDate, endDate, acc.trackingMode],
      queryFn: () =>
        calculatePerformanceSummary({
          itemType: "account",
          itemId: acc.id,
          startDate,
          endDate,
          trackingMode:
            acc.trackingMode === "HOLDINGS" || acc.trackingMode === "TRANSACTIONS"
              ? acc.trackingMode
              : undefined,
        }),
      staleTime: 60 * 1000,
      retry: false,
    })),
  });

  const isLoading = targetAccounts.length > 0 && queries.every((q) => q.isLoading);

  const ytd = useMemo(() => {
    const results = queries.filter((q) => q.data).map((q) => q.data!);
    if (results.length === 0) return null;

    // Sum gains in base currency
    const totalGain = results.reduce((s, r) => s + (r.periodGain ?? 0), 0);

    // YTD % = gain / start-of-year value (= current value - gain)
    const startValue = totalValue - totalGain;
    const gainPct = startValue > 0 ? (totalGain / startValue) * 100 : null;

    return { gainAmount: totalGain, gainPct };
  }, [queries, totalValue]);

  return { ytd, isLoading };
}
