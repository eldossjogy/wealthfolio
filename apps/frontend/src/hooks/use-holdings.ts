import { useQuery } from "@tanstack/react-query";
import { AccountFilter, Holding } from "@/lib/types";
import { getHoldings } from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";

export function useHoldings(filter: AccountFilter | string) {
  const accountFilter: AccountFilter =
    typeof filter === "string" ? { type: "account", accountId: filter } : filter;

  const {
    data: holdings = [],
    isLoading,
    isError,
    error,
  } = useQuery<Holding[], Error>({
    queryKey: [QueryKeys.HOLDINGS, accountFilter],
    queryFn: () => getHoldings(accountFilter),
    enabled: typeof filter === "string" ? !!filter : true,
  });

  return { holdings, isLoading, isError, error };
}
