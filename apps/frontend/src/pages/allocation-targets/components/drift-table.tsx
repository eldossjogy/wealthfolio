import { Badge, Card, CardContent, CardHeader, CardTitle, CardDescription } from "@wealthfolio/ui";
import { cn, formatAmount } from "@/lib/utils";
import type { DriftReport, DriftRow, DriftStatus } from "@/lib/types";

interface DriftTableProps {
  report: DriftReport;
}

function StatusBadge({ status }: { status: DriftStatus }) {
  if (status === "in_band") {
    return (
      <Badge
        variant="outline"
        className="border-green-200 text-[11px] font-medium text-green-700 dark:border-green-800 dark:text-green-400"
      >
        In band
      </Badge>
    );
  }
  if (status === "overweight") {
    return (
      <Badge
        variant="outline"
        className="text-destructive border-destructive/30 text-[11px] font-medium"
      >
        Over
      </Badge>
    );
  }
  if (status === "underweight") {
    return (
      <Badge
        variant="outline"
        className="border-blue-200 text-[11px] font-medium text-blue-600 dark:border-blue-800 dark:text-blue-400"
      >
        Under
      </Badge>
    );
  }
  return (
    <Badge variant="outline" className="text-muted-foreground text-[11px] font-medium">
      —
    </Badge>
  );
}

function driftCellColor(row: DriftRow): string {
  if (row.status === "in_band") return "text-muted-foreground";
  if (row.status === "overweight") return "text-destructive";
  return "text-blue-600 dark:text-blue-400";
}

function deltaCellColor(row: DriftRow): string {
  if (row.valueDelta > 0) return "text-green-700 dark:text-green-400";
  if (row.valueDelta < 0) return "text-destructive";
  return "text-muted-foreground";
}

export function DriftTable({ report }: DriftTableProps) {
  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="text-base">All holdings</CardTitle>
        <CardDescription>
          {report.rows.length} categories · {report.baseCurrency}
        </CardDescription>
      </CardHeader>
      <CardContent className="p-0">
        <div className="overflow-x-auto">
          <table className="w-full text-[13px]">
            <thead>
              <tr className="text-muted-foreground border-b text-[10px] uppercase tracking-wider">
                <th className="w-[28px] py-2.5 pl-6 text-left font-medium"></th>
                <th className="py-2.5 pl-2 text-left font-medium">Category</th>
                <th className="py-2.5 pr-3 text-right font-medium">Value</th>
                <th className="py-2.5 pr-3 text-right font-medium">Current</th>
                <th className="py-2.5 pr-3 text-right font-medium">Target</th>
                <th className="py-2.5 pr-3 text-right font-medium">Drift</th>
                <th className="py-2.5 pr-3 text-right font-medium">Δ Value</th>
                <th className="py-2.5 pr-6 text-right font-medium">Status</th>
              </tr>
            </thead>
            <tbody>
              {report.rows.map((row) => (
                <tr
                  key={row.categoryId}
                  className="hover:bg-muted/30 h-11 border-b last:border-b-0"
                >
                  <td className="pl-6">
                    <span
                      className="inline-block h-2.5 w-2.5 rounded-sm"
                      style={{ background: row.color || "var(--muted-foreground)" }}
                    />
                  </td>
                  <td className="text-foreground pl-2 font-medium">{row.categoryName}</td>
                  <td className="text-foreground pr-3 text-right tabular-nums">
                    {formatAmount(row.currentValue, report.baseCurrency)}
                  </td>
                  <td className="text-foreground pr-3 text-right font-medium tabular-nums">
                    {(row.currentBps / 100).toFixed(2)}%
                  </td>
                  <td className="text-muted-foreground pr-3 text-right tabular-nums">
                    {(row.targetBps / 100).toFixed(0)}%
                  </td>
                  <td
                    className={cn("pr-3 text-right font-medium tabular-nums", driftCellColor(row))}
                  >
                    {row.driftBps > 0 ? "+" : ""}
                    {(row.driftBps / 100).toFixed(2)}%
                  </td>
                  <td className={cn("pr-3 text-right tabular-nums", deltaCellColor(row))}>
                    {row.valueDelta > 0 ? "+" : row.valueDelta < 0 ? "−" : ""}
                    {formatAmount(Math.abs(row.valueDelta), report.baseCurrency)}
                  </td>
                  <td className="pr-6 text-right">
                    <StatusBadge status={row.status} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </CardContent>
    </Card>
  );
}
