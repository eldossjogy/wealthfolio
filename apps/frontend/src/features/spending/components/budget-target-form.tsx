import { useEffect, useState } from "react";

import { Button, Card, CardContent, CardHeader, CardTitle, Icons, Input } from "@wealthfolio/ui";

import type { BudgetConfig, UpdateBudgetConfig } from "../types/budget";

interface BudgetTargetFormProps {
  config: BudgetConfig | null;
  currency: string;
  onSave: (patch: UpdateBudgetConfig) => void;
  isPending?: boolean;
}

export function BudgetTargetForm({ config, currency, onSave, isPending }: BudgetTargetFormProps) {
  const [spendingTarget, setSpendingTarget] = useState<string>("");
  const [incomeTarget, setIncomeTarget] = useState<string>("");
  const [isDirty, setIsDirty] = useState(false);

  useEffect(() => {
    if (config) {
      setSpendingTarget(config.monthlySpendingTarget.toString());
      setIncomeTarget(config.monthlyIncomeTarget.toString());
      setIsDirty(false);
    }
  }, [config]);

  const handleSpendingChange = (value: string) => {
    setSpendingTarget(value);
    setIsDirty(true);
  };

  const handleIncomeChange = (value: string) => {
    setIncomeTarget(value);
    setIsDirty(true);
  };

  const handleSave = () => {
    onSave({
      monthlySpendingTarget: spendingTarget || "0",
      monthlyIncomeTarget: incomeTarget || "0",
      currency,
    });
    setIsDirty(false);
  };

  return (
    <div className="grid gap-4 sm:grid-cols-2">
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="flex items-center gap-2 text-base">
            <Icons.TrendingDown className="text-destructive h-4 w-4" />
            Monthly Spending Target
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center gap-2">
            <span className="text-muted-foreground">{currency}</span>
            <Input
              type="number"
              min="0"
              step="0.01"
              placeholder="0.00"
              value={spendingTarget}
              onChange={(e) => handleSpendingChange(e.target.value)}
              className="text-lg font-semibold"
            />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="flex items-center gap-2 text-base">
            <Icons.TrendingUp className="text-success h-4 w-4" />
            Monthly Income Target
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center gap-2">
            <span className="text-muted-foreground">{currency}</span>
            <Input
              type="number"
              min="0"
              step="0.01"
              placeholder="0.00"
              value={incomeTarget}
              onChange={(e) => handleIncomeChange(e.target.value)}
              className="text-lg font-semibold"
            />
          </div>
        </CardContent>
      </Card>

      {isDirty && (
        <div className="sm:col-span-2">
          <Button onClick={handleSave} disabled={isPending} className="w-full sm:w-auto">
            {isPending ? (
              <>
                <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                Saving...
              </>
            ) : (
              "Save Targets"
            )}
          </Button>
        </div>
      )}
    </div>
  );
}
