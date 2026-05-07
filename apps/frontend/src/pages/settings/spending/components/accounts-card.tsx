import { useMemo } from "react";

import { useAccounts } from "@/hooks/use-accounts";
import type { Account } from "@/lib/types";
import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Icons,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@wealthfolio/ui";

import {
  useSpendingSettings,
  useSpendingSettingsMutation,
} from "@/features/spending/hooks/use-spending-settings";

export function AccountsCard() {
  const { settings } = useSpendingSettings();
  const mutation = useSpendingSettingsMutation();
  const { accounts } = useAccounts({ filterActive: true });

  const accountIds = settings?.accountIds ?? [];

  const cashAccounts = useMemo<Account[]>(
    () => (accounts ?? []).filter((a) => a.accountType === "CASH"),
    [accounts],
  );

  const includedAccounts = useMemo(
    () => cashAccounts.filter((a) => accountIds.includes(a.id)),
    [cashAccounts, accountIds],
  );

  const availableAccounts = useMemo(
    () => cashAccounts.filter((a) => !accountIds.includes(a.id)),
    [cashAccounts, accountIds],
  );

  const handleAdd = (id: string) => {
    const next = Array.from(new Set([...accountIds, id]));
    mutation.mutate({ accountIds: next });
  };

  const handleRemove = (id: string) => {
    const next = accountIds.filter((x) => x !== id);
    mutation.mutate({ accountIds: next });
  };

  return (
    <Card>
      <CardHeader className="flex flex-row items-start justify-between gap-2 space-y-0 p-4 pb-3">
        <div className="min-w-0 space-y-0.5">
          <CardTitle className="text-sm font-medium">Spending accounts</CardTitle>
          <CardDescription className="text-xs">
            Cash accounts that participate in spending. New CASH accounts auto-include.
          </CardDescription>
        </div>
        <Popover>
          <PopoverTrigger asChild>
            <Button
              variant="outline"
              size="sm"
              className="-mt-1 shrink-0"
              disabled={availableAccounts.length === 0}
            >
              <Icons.Plus className="mr-1 h-3.5 w-3.5" />
              Add account
            </Button>
          </PopoverTrigger>
          <PopoverContent className="w-72 p-0" align="end">
            {availableAccounts.length === 0 ? (
              <div className="text-muted-foreground p-3 text-xs">
                All cash accounts are already included.
              </div>
            ) : (
              <div className="max-h-72 overflow-y-auto py-1">
                {availableAccounts.map((account) => (
                  <button
                    key={account.id}
                    type="button"
                    onClick={() => handleAdd(account.id)}
                    disabled={mutation.isPending}
                    className="hover:bg-muted/60 flex w-full items-center justify-between gap-3 px-3 py-2 text-left transition-colors"
                  >
                    <div className="min-w-0">
                      <div className="text-foreground truncate text-sm font-medium">
                        {account.name}
                      </div>
                      <div className="text-muted-foreground truncate text-[11px]">
                        {account.currency}
                        {account.group ? ` · ${account.group}` : ""}
                      </div>
                    </div>
                    <Icons.Plus className="text-muted-foreground h-3.5 w-3.5 shrink-0" />
                  </button>
                ))}
              </div>
            )}
          </PopoverContent>
        </Popover>
      </CardHeader>
      <CardContent className="p-4 pt-0">
        {cashAccounts.length === 0 ? (
          <p className="text-muted-foreground text-xs">
            No CASH accounts found. Create one in Settings → Accounts.
          </p>
        ) : includedAccounts.length === 0 ? (
          <div className="text-muted-foreground rounded-md border border-dashed py-6 text-center text-xs">
            No accounts selected. Use “Add account” to include cash accounts.
          </div>
        ) : (
          <div className="space-y-1">
            {includedAccounts.map((account) => (
              <div
                key={account.id}
                className="bg-muted/30 flex items-center gap-3 rounded-lg px-3 py-2.5"
              >
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-semibold">{account.name}</div>
                  <div className="text-muted-foreground text-[10px]">
                    {account.currency}
                    {account.group ? ` · ${account.group}` : ""}
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => handleRemove(account.id)}
                  disabled={mutation.isPending}
                  className="text-muted-foreground hover:text-foreground shrink-0 rounded-md p-1 transition-colors"
                  aria-label={`Remove ${account.name}`}
                >
                  <Icons.X className="h-3.5 w-3.5" />
                </button>
              </div>
            ))}
            <p className="text-muted-foreground pt-1 text-[11px]">
              {includedAccounts.length} of {cashAccounts.length} cash accounts included
            </p>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
