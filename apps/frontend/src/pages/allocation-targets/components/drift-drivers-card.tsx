import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@wealthfolio/ui";
import { cn, formatAmount } from "@/lib/utils";
import type { DriftReport, DriftRow } from "@/lib/types";

interface DriftDriversCardProps {
  report: DriftReport;
}

function buildDriver(row: DriftRow, currency: string) {
  const current = (row.currentBps / 100).toFixed(1);
  const target = (row.targetBps / 100).toFixed(0);
  const driftPct = (row.driftBps / 100).toFixed(1);
  const absDelta = Math.abs(row.valueDelta);
  const sign = row.driftBps > 0 ? "+" : "";

  if (row.status === "overweight") {
    return {
      title: `${row.categoryName} is overweight`,
      detail: `At ${current}%, it's ${driftPct}pp above the ${target}% target. ${formatAmount(absDelta, currency)} above target value.`,
      drift: `${sign}${driftPct}%`,
      isOver: true,
    };
  }
  return {
    title: `${row.categoryName} is underweight`,
    detail: `At ${current}%, it's ${Math.abs(row.driftBps / 100).toFixed(1)}pp below the ${target}% target. ${formatAmount(absDelta, currency)} below target value.`,
    drift: `${sign}${driftPct}%`,
    isOver: false,
  };
}

export function DriftDriversCard({ report }: DriftDriversCardProps) {
  const oobRows = report.rows.filter(
    (r) => r.status === "overweight" || r.status === "underweight",
  );

  return (
    <Card className="h-full">
      <CardHeader className="pb-3">
        <CardTitle className="text-base">What moved off target</CardTitle>
        <CardDescription>Drivers since last rebalance</CardDescription>
      </CardHeader>
      <CardContent>
        {oobRows.length === 0 ? (
          <p className="text-muted-foreground py-6 text-center text-[13px]">
            All sleeves are within the tolerance band. No action required.
          </p>
        ) : (
          <ul className="space-y-4">
            {oobRows.map((row) => {
              const driver = buildDriver(row, report.baseCurrency);
              return (
                <li
                  key={row.categoryId}
                  className={cn(
                    "border-l-2 pl-3",
                    driver.isOver ? "border-destructive" : "border-blue-500",
                  )}
                >
                  <div className="flex items-start justify-between gap-2">
                    <p className="text-foreground text-[13px] font-medium leading-snug">
                      {driver.title}
                    </p>
                    <span
                      className={cn(
                        "shrink-0 text-[12px] font-medium tabular-nums",
                        driver.isOver ? "text-destructive" : "text-blue-600 dark:text-blue-400",
                      )}
                    >
                      {driver.drift}
                    </span>
                  </div>
                  <p className="text-muted-foreground mt-0.5 text-[12px] leading-snug">
                    {driver.detail}
                  </p>
                </li>
              );
            })}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}
