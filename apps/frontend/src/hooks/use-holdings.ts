import { useQuery } from "@tanstack/react-query";
import { AccountScope, Holding } from "@/lib/types";
import { getHoldings } from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";

export function useHoldings(accountFilter: AccountScope) {
  const isEnabled = accountFilter.type !== "account" || accountFilter.accountId.trim().length > 0;

  const {
    data: holdings = [],
    dataUpdatedAt,
    isLoading,
    isError,
    error,
  } = useQuery<Holding[], Error>({
    queryKey: [QueryKeys.HOLDINGS, accountFilter],
    queryFn: () => getHoldings(accountFilter),
    enabled: isEnabled,
  });

  return { holdings, dataUpdatedAt, isLoading, isError, error };
}
