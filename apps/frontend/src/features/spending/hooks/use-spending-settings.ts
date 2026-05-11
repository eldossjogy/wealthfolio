import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { QueryKeys } from "@/lib/query-keys";

import { getSpendingSettings, updateSpendingSettings } from "../adapters/settings";
import type { SpendingSettings, SpendingSettingsUpdate } from "../types";

export function useSpendingSettings() {
  const query = useQuery<SpendingSettings, Error>({
    queryKey: [QueryKeys.SPENDING_SETTINGS],
    queryFn: getSpendingSettings,
    staleTime: 60_000,
  });

  return {
    settings: query.data,
    isEnabled: query.data?.enabled ?? false,
    accountIds: query.data?.accountIds ?? [],
    isLoading: query.isLoading,
    error: query.error,
  };
}

export function useSpendingSettingsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (update: SpendingSettingsUpdate) => updateSpendingSettings(update),
    onSuccess: (data) => {
      queryClient.setQueryData([QueryKeys.SPENDING_SETTINGS], data);
      // Account opt-in changes the universe of cash activities — invalidate every
      // spending-scoped cache so each view refetches with the new account_ids.
      queryClient.invalidateQueries({ queryKey: [QueryKeys.SPENDING_SUMMARY] });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.SPENDING_TRANSACTIONS] });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.SPENDING_REPORT] });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.SPENDING_CATEGORY_BREAKDOWN] });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.SPENDING_EVENTS] });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.SPENDING_BUDGET] });
      toast.success("Spending settings updated.");
    },
    onError: () => toast.error("Failed to update spending settings."),
  });
}
