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
import {
  isCreditCardAccountType,
  isSpendingAccountType,
} from "@/features/spending/lib/constants";
import { AccountType } from "@/lib/constants";

export function AccountsCard() {
  const { settings } = useSpendingSettings();
  const mutation = useSpendingSettingsMutation();
  const { accounts } = useAccounts({ filterActive: false });

  const accountIds = useMemo(() => settings?.accountIds ?? [], [settings?.accountIds]);

  const spendingAccounts = useMemo<Account[]>(
    () => (accounts ?? []).filter((a) => isSpendingAccountType(a.accountType)),
    [accounts],
  );

  const includedAccounts = useMemo(
    () => spendingAccounts.filter((a) => accountIds.includes(a.id)),
    [spendingAccounts, accountIds],
  );

  const availableAccounts = useMemo(
    () => spendingAccounts.filter((a) => a.isActive && !accountIds.includes(a.id)),
    [spendingAccounts, accountIds],
  );

  const hasMixedCashAndCreditAccounts = useMemo(() => {
    const hasCash = includedAccounts.some((account) => account.accountType === AccountType.CASH);
    const hasCreditCard = includedAccounts.some((account) =>
      isCreditCardAccountType(account.accountType),
    );
    return hasCash && hasCreditCard;
  }, [includedAccounts]);

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
            Cash and credit card accounts that participate in spending.
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
                All active spending accounts are already included.
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
        {spendingAccounts.length === 0 ? (
          <p className="text-muted-foreground text-xs">
            No cash or credit card accounts found. Create one in Settings → Accounts.
          </p>
        ) : includedAccounts.length === 0 ? (
          <div className="text-muted-foreground rounded-md border border-dashed py-6 text-center text-xs">
            No accounts selected. Use “Add account” to include spending accounts.
          </div>
        ) : (
          <div className="space-y-1">
            {includedAccounts.map((account) => (
              <div
                key={account.id}
                className="bg-muted/30 flex items-center gap-3 rounded-lg px-3 py-2.5"
              >
                <div className="min-w-0 flex-1">
                  <div className="flex min-w-0 items-center gap-2">
                    <span className="truncate text-sm font-semibold">{account.name}</span>
                    {!account.isActive && (
                      <span className="bg-muted text-muted-foreground shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium">
                        Inactive
                      </span>
                    )}
                  </div>
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
            {hasMixedCashAndCreditAccounts && (
              <div className="mt-2 flex gap-2 rounded-md border border-amber-500/30 bg-amber-500/10 p-2.5 text-[11px] leading-relaxed text-amber-700 dark:text-amber-300">
                <Icons.AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                <p>
                  Credit card payments from tracked cash accounts should be linked as transfers.
                  Unlinked payments can make both the card charge and cash payment count as
                  spending.
                </p>
              </div>
            )}
            <p className="text-muted-foreground pt-1 text-[11px]">
              {includedAccounts.length} of {spendingAccounts.length} spending accounts included
            </p>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
