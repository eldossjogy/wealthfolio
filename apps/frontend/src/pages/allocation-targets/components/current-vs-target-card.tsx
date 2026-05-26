import { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@wealthfolio/ui";
import { cn } from "@/lib/utils";
import type { DriftReport, DriftRow } from "@/lib/types";
import { AllocationDonut } from "./allocation-donut";

interface CurrentVsTargetCardProps {
  report: DriftReport;
}

function driftColor(row: DriftRow): string {
  if (row.status === "in_band") return "text-muted-foreground";
  if (row.status === "overweight") return "text-destructive";
  return "text-blue-600 dark:text-blue-400";
}

function driftSign(bps: number): string {
  return bps > 0 ? "+" : "";
}

export function CurrentVsTargetCard({ report }: CurrentVsTargetCardProps) {
  const [hoveredId, setHoveredId] = useState<string | null>(null);
  const targetedRows = report.rows.filter((r) => r.status !== "not_targeted");

  return (
    <Card className="h-full">
      <CardHeader className="pb-3">
        <CardTitle className="text-base">Current vs target</CardTitle>
        <CardDescription>
          By asset class — weights and drift versus your saved targets
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div className="flex flex-col items-center gap-4 pt-4 md:flex-row md:items-start md:pt-6">
          <div className="ml-10 shrink-0">
            <AllocationDonut
              rows={targetedRows}
              totalValue={report.totalValue}
              currency={report.baseCurrency}
              size={300}
              hoveredId={hoveredId}
              onHoverChange={setHoveredId}
            />
          </div>

          <div className="flex min-w-0 flex-1 flex-col" style={{ height: 300 }}>
            {/* Column headers */}
            <div
              className="grid items-center px-2 pb-1.5"
              style={{ gridTemplateColumns: "auto 1fr auto" }}
            >
              <span />
              <span />
              <div className="flex items-center justify-end gap-6">
                <span className="text-muted-foreground text-[10px] font-medium uppercase tracking-wider">
                  Current
                </span>
                <span className="text-muted-foreground w-11 text-right text-[10px] font-medium uppercase tracking-wider">
                  Drift
                </span>
              </div>
            </div>

            {/* Asset class rows */}
            <div className="flex flex-1 flex-col overflow-hidden">
              {targetedRows.flatMap((row, i) => {
                const isHovered = hoveredId === row.categoryId;
                const dimmed = hoveredId !== null && !isHovered;
                const rowColor = row.color || "#888888";
                const rowEl = (
                  <div
                    key={row.categoryId}
                    className="grid cursor-default items-center gap-x-2 rounded-sm px-2"
                    style={{
                      gridTemplateColumns: "auto 1fr auto",
                      flex: 1,
                      transition: "background-color 0.15s ease",
                      backgroundColor: isHovered ? `${rowColor}22` : undefined,
                    }}
                    onMouseEnter={() => setHoveredId(row.categoryId)}
                    onMouseLeave={() => setHoveredId(null)}
                  >
                    <span
                      className="h-2 w-2 shrink-0 rounded-sm"
                      style={{ background: rowColor }}
                    />
                    <div className="min-w-0">
                      <div className="text-foreground truncate text-[12px] font-medium">
                        {row.categoryName}
                      </div>
                      <div className="text-muted-foreground text-[11px] tabular-nums">
                        target {(row.targetBps / 100).toFixed(0)}%
                      </div>
                    </div>
                    <div className="flex items-center justify-end gap-6">
                      <span className="text-foreground text-[12px] font-semibold tabular-nums">
                        {(row.currentBps / 100).toFixed(1)}%
                      </span>
                      <span
                        className={cn(
                          "w-11 text-right text-[11px] font-medium tabular-nums",
                          driftColor(row),
                        )}
                      >
                        {driftSign(row.driftBps)}
                        {(row.driftBps / 100).toFixed(1)}%
                      </span>
                    </div>
                  </div>
                );
                return i < targetedRows.length - 1
                  ? [rowEl, <div key={`sep-${i}`} className="bg-border/60 h-0.5 shrink-0" />]
                  : [rowEl];
              })}
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
