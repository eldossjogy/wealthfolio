import { useQuery, keepPreviousData } from "@tanstack/react-query";
import { AccountScope, AccountValuation, DateRange } from "@/lib/types";
import { getHistoricalValuations } from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";
import { format } from "date-fns";

export function useValuationHistory(
  dateRange: DateRange | undefined,
  filter: AccountScope = { type: "all" },
) {
  const scopeKey = JSON.stringify(filter);
  const {
    data: valuationHistory,
    isLoading,
    isFetching,
  } = useQuery<AccountValuation[], Error>({
    queryKey: [
      ...QueryKeys.valuationHistory(scopeKey),
      dateRange?.from ? format(dateRange.from, "yyyy-MM-dd") : null,
      dateRange?.to ? format(dateRange.to, "yyyy-MM-dd") : null,
    ],
    queryFn: () => {
      if (dateRange === undefined) {
        return getHistoricalValuations(filter, undefined, undefined);
      }

      if (!dateRange?.from || !dateRange?.to) {
        console.error("Invalid date range provided to useValuationHistory", dateRange);
        return Promise.resolve([]);
      }

      return getHistoricalValuations(
        filter,
        format(dateRange.from, "yyyy-MM-dd"),
        format(dateRange.to, "yyyy-MM-dd"),
      );
    },
    enabled: dateRange === undefined || (!!dateRange?.from && !!dateRange?.to),
    placeholderData: keepPreviousData,
  });

  return {
    valuationHistory,
    isLoading: isLoading || isFetching,
  };
}
