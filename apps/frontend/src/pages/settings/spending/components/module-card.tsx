import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Label,
  Switch,
} from "@wealthfolio/ui";

import {
  useSpendingSettings,
  useSpendingSettingsMutation,
} from "@/features/spending/hooks/use-spending-settings";
import { isSpendingAccountType } from "@/features/spending/lib/constants";
import { useAccounts } from "@/hooks/use-accounts";
import type { Account } from "@/lib/types";

export function ModuleCard() {
  const { settings, isLoading } = useSpendingSettings();
  const mutation = useSpendingSettingsMutation();
  const { accounts } = useAccounts({ filterActive: true });

  const enabled = settings?.enabled ?? false;
  const accountIds = settings?.accountIds ?? [];

  const handleToggle = (next: boolean) => {
    let nextIds = accountIds;
    if (next && accountIds.length === 0) {
      const spendingAccounts = (accounts ?? []).filter((a: Account) =>
        isSpendingAccountType(a.accountType),
      );
      if (spendingAccounts.length > 0) nextIds = spendingAccounts.map((a) => a.id);
    }
    mutation.mutate({ enabled: next, accountIds: nextIds });
  };

  return (
    <Card>
      <CardHeader className="p-4 pb-3">
        <CardTitle className="text-sm font-medium">Enable spending tracking</CardTitle>
        <CardDescription className="text-xs">
          Adds a Spending section to the sidebar and dashboard. Off by default. Disabling hides
          spending UI — your data is preserved.
        </CardDescription>
      </CardHeader>
      <CardContent className="p-4 pt-0">
        <div className="flex items-center justify-between">
          <Label htmlFor="spending-enabled" className="text-sm">
            {enabled ? "Enabled" : "Disabled"}
          </Label>
          <Switch
            id="spending-enabled"
            checked={enabled}
            onCheckedChange={handleToggle}
            disabled={isLoading || mutation.isPending}
          />
        </div>
      </CardContent>
    </Card>
  );
}
