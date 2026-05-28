import {
  HOLDINGS_MODE_MAX_DRAWDOWN_INFO,
  HOLDINGS_MODE_VOLATILITY_INFO,
  IRR_RETURN_INFO,
  MAX_DRAWDOWN_INFO,
  MetricDisplay,
  TIME_WEIGHTED_RETURN_INFO,
  VALUE_RETURN_INFO,
  VOLATILITY_INFO,
} from "@/components/metric-display";
import { Card, CardContent } from "@wealthfolio/ui/components/ui/card";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
import { Icons } from "@wealthfolio/ui";
import { Alert, AlertDescription } from "@wealthfolio/ui/components/ui/alert";
import { PerformanceResult } from "@/lib/types";
import { performancePeriodPnl } from "@/lib/performance";
import { cn } from "@/lib/utils";
import React from "react";

export interface PerformanceGridProps {
  performance?: PerformanceResult | null;
  isLoading?: boolean;
  performanceError?: string;
  className?: string;
  /** If true, shows holdings-mode return cards instead of transaction return cards. */
  isHoldingsMode?: boolean;
}

export const PerformanceGrid: React.FC<PerformanceGridProps> = ({
  performance,
  isLoading,
  performanceError,
  className,
  isHoldingsMode = false,
}) => {
  if (performanceError) {
    return (
      <div className={cn("w-full", className)}>
        <Alert
          variant="warning"
          className="flex flex-col items-center gap-2 text-center [&>svg+div]:translate-y-0 [&>svg]:static [&>svg~*]:pl-0"
        >
          <Icons.AlertTriangle className="size-5" />
          <AlertDescription className="text-xs">{performanceError}</AlertDescription>
        </Alert>
      </div>
    );
  }

  if (isLoading || !performance) {
    return (
      <div className={cn("w-full", className)}>
        <Card className="border-none p-0 shadow-none">
          <CardContent className="p-0">
            <div className="grid grid-cols-2 gap-5">
              {[...Array(4)].map((_, index) => (
                <div
                  key={index}
                  className="border-muted/30 bg-muted/30 flex min-h-24 flex-col items-center justify-center space-y-2 rounded-md border p-4 md:p-6"
                >
                  <Skeleton className="h-3 w-32" />
                  <Skeleton className="h-5 w-16" />
                  <Skeleton className="h-3 w-20" />
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>
    );
  }

  const twrValue = performance.returns.twr ?? undefined;
  const twrAnnualized = performance.returns.annualizedTwr ?? undefined;
  const irrValue = performance.returns.irr ?? undefined;
  const irrAnnualized = performance.returns.annualizedIrr ?? undefined;
  const valueReturn = performance.returns.valueReturn ?? undefined;
  const periodPnl = performancePeriodPnl(performance) ?? undefined;
  const volatility = performance.risk.volatility ?? undefined;
  const maxDrawdown = performance.risk.maxDrawdown ?? undefined;
  const notApplicableReasons = performance.dataQuality.notApplicableReasons ?? [];
  const reasonFor = (needle: string) =>
    notApplicableReasons.find((reason) => reason.toLowerCase().includes(needle.toLowerCase()));

  // For HOLDINGS mode accounts:
  // - TWR/IRR are NOT available (require cash flow tracking)
  // - Volatility and Max Drawdown ARE available (computed from equity curve)
  if (isHoldingsMode) {
    return (
      <div className={cn("w-full", className)}>
        <Card className="border-none p-0 shadow-none">
          <CardContent className="p-0">
            <div className="grid grid-cols-2 gap-5">
              <MetricDisplay
                label="Value Return"
                value={valueReturn}
                emptyReason={reasonFor("value return")}
                infoText={VALUE_RETURN_INFO}
                isPercentage={true}
                className="border-muted/30 bg-muted/30 rounded-md border"
              />
              <MetricDisplay
                label="Total P&L"
                value={periodPnl}
                emptyReason={reasonFor("P&L") ?? reasonFor("performance")}
                infoText="Total profit or loss over the selected period."
                isPercentage={false}
                currency={performance.scope.currency}
                className="border-muted/30 bg-muted/30 rounded-md border"
              />
              <MetricDisplay
                label="Volatility"
                value={volatility}
                emptyReason={reasonFor("volatility")}
                infoText={HOLDINGS_MODE_VOLATILITY_INFO}
                isPercentage={true}
                tone="neutral"
                className="border-muted/30 bg-muted/30 rounded-md border"
              />
              <MetricDisplay
                label="Max Drawdown"
                value={maxDrawdown}
                emptyReason={reasonFor("drawdown")}
                infoText={HOLDINGS_MODE_MAX_DRAWDOWN_INFO}
                isPercentage={true}
                className="border-muted/30 bg-muted/30 rounded-md border"
              />
            </div>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className={cn("w-full", className)}>
      <Card className="border-none p-0 shadow-none">
        <CardContent className="p-0">
          <div className="grid grid-cols-2 gap-5">
            <MetricDisplay
              label="Time Weighted Return"
              value={twrValue}
              annualizedValue={twrAnnualized}
              emptyReason={reasonFor("TWR")}
              infoText={TIME_WEIGHTED_RETURN_INFO}
              isPercentage={true}
              className="border-muted/30 bg-muted/30 rounded-md border"
            />
            <MetricDisplay
              label="IRR"
              value={irrValue}
              annualizedValue={irrAnnualized}
              emptyReason={reasonFor("IRR")}
              infoText={IRR_RETURN_INFO}
              isPercentage={true}
              className="border-muted/30 bg-muted/30 rounded-md border"
            />
            <MetricDisplay
              label="Volatility"
              value={volatility}
              emptyReason={reasonFor("volatility")}
              infoText={VOLATILITY_INFO}
              isPercentage={true}
              tone="neutral"
              className="border-muted/30 bg-muted/30 rounded-md border"
            />
            <MetricDisplay
              label="Max Drawdown"
              value={maxDrawdown}
              emptyReason={reasonFor("drawdown")}
              infoText={MAX_DRAWDOWN_INFO}
              isPercentage={true}
              className="border-muted/30 bg-muted/30 rounded-md border"
            />
          </div>
        </CardContent>
      </Card>
    </div>
  );
};

// Default export for easy import
export default PerformanceGrid;
