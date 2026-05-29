import { useMemo } from "react";
import { useQueries } from "@tanstack/react-query";
import { calculatePerformanceHistory } from "@/adapters";
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

export interface PortfolioStats {
  annualizedReturn: number | null;
  volatility: number | null;
}

export function usePortfolioStats(accountScope: AccountScope): {
  stats: PortfolioStats | null;
  isLoading: boolean;
} {
  const { accounts } = useAccounts();
  const { data: portfolios = [] } = usePortfolios();

  const portfolioAccountIds = useMemo(() => {
    if (accountScope.type !== "portfolio") return undefined;
    return portfolios.find((p) => p.id === accountScope.portfolioId)?.accountIds;
  }, [portfolios, accountScope]);

  const targetAccounts = useMemo(
    () => scopedAccounts(accounts, accountScope, portfolioAccountIds),
    [accounts, accountScope, portfolioAccountIds],
  );

  const today = new Date();
  const startDate = `${today.getFullYear() - 3}-${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}`;
  const endDate = today.toISOString().split("T")[0];

  const queries = useQueries({
    queries: targetAccounts.map((acc) => ({
      queryKey: ["portfolio-stats", acc.id, acc.trackingMode, startDate, accountScope],
      queryFn: () =>
        calculatePerformanceHistory(
          "account",
          acc.id,
          startDate,
          endDate,
          acc.trackingMode === "HOLDINGS" || acc.trackingMode === "TRANSACTIONS"
            ? acc.trackingMode
            : undefined,
        ),
      staleTime: 5 * 60 * 1000,
      retry: false,
    })),
  });

  const isLoading = targetAccounts.length > 0 && queries.some((q) => q.isLoading);

  const stats = useMemo(() => {
    const results = queries.filter((q) => q.data).map((q) => q.data!);
    if (results.length === 0) return null;

    const returnValues = results
      .map((r) => r.annualizedTwr ?? r.annualizedModifiedDietz ?? r.annualizedSimpleReturn ?? null)
      .filter((v): v is number => v != null && isFinite(v));

    const volValues = results
      .map((r) => r.volatility)
      .filter((v): v is number => v != null && isFinite(v));

    const annualizedReturn =
      returnValues.length > 0
        ? (returnValues.reduce((s, v) => s + v, 0) / returnValues.length) * 100
        : null;

    const volatility =
      volValues.length > 0 ? (volValues.reduce((s, v) => s + v, 0) / volValues.length) * 100 : null;

    if (annualizedReturn === null && volatility === null) return null;

    return { annualizedReturn, volatility };
  }, [queries]);

  return { stats, isLoading };
}
