import { Button } from "@wealthfolio/ui/components/ui/button";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { Popover, PopoverContent, PopoverTrigger } from "@wealthfolio/ui/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@wealthfolio/ui/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { formatPercent, GainAmount, GainPercent } from "@wealthfolio/ui";
import React, { useState } from "react";

// Explanatory texts for info popovers
export const TIME_WEIGHTED_RETURN_INFO =
  "Time-Weighted Return (TWR) measures the compound growth rate of a portfolio, ignoring the impact of cash flows (deposits/withdrawals). It isolates the performance of the underlying investments.";
export const IRR_RETURN_INFO =
  "Internal Rate of Return (IRR) measures selected-period money-weighted performance using the size and timing of external cash flows. Annualized IRR shows the XIRR equivalent per year.";
export const SIMPLE_RETURN_INFO =
  "Simple return measures value growth over the selected period after adjusting for external cash flows. It is not time weighted.";
export const VALUE_RETURN_INFO =
  "Value return measures the change in account value over the selected period when transaction-level cash flows are not available.";
export const PRICE_RETURN_INFO =
  "Price return measures the selected symbol's price change over the selected period.";
export const VOLATILITY_INFO =
  "Volatility measures the dispersion of returns for a given investment. Higher volatility means the investment can change dramatically over a short period. It is annualized from calendar-daily returns.";
export const MAX_DRAWDOWN_INFO =
  "Maximum Drawdown represents the largest percentage decline from a peak to a subsequent trough in portfolio value during the specified period. It indicates downside risk.";
export const ANNUALIZED_RETURN_INFO =
  "Annualized Return shows the geometric average amount of money earned by an investment each year over the selected period, as if the returns were compounded annually.";

// Holdings mode specific info texts.
export const HOLDINGS_MODE_VOLATILITY_INFO =
  "Volatility of account value changes. Based on daily valuations without adjusting for deposits or withdrawals. Reflects how much the market value fluctuates.";
export const HOLDINGS_MODE_MAX_DRAWDOWN_INFO =
  "Largest peak-to-trough decline in account value. Based on daily valuations without adjusting for deposits or withdrawals.";

export interface MetricDisplayProps {
  label: string;
  value?: number; // Made optional as performance-page might only need label and info
  infoText: string;
  annualizedValue?: number | null;
  isPercentage?: boolean;
  currency?: string;
  className?: string;
  valueClassName?: string; // Added to allow custom styling for the value itself
  labelComponent?: React.ReactNode; // Allow passing a full component for label + info
  emptyReason?: string;
  tone?: "gain" | "neutral";
}

export const MetricDisplay: React.FC<MetricDisplayProps> = ({
  label,
  value,
  infoText,
  annualizedValue,
  isPercentage = true,
  currency = "USD",
  className,
  valueClassName,
  labelComponent,
  emptyReason,
  tone = "gain",
}) => {
  const [mobilePopoverOpen, setMobilePopoverOpen] = useState(false);

  const displayValue =
    value === undefined ? (
      <span className={cn("text-muted-foreground text-base font-medium", valueClassName)}>N/A</span>
    ) : isPercentage && tone === "neutral" ? (
      <span className={cn("text-foreground text-base font-medium", valueClassName)}>
        {formatPercent(value)}
      </span>
    ) : isPercentage ? (
      <GainPercent
        value={value}
        animated={true}
        showSign={isPercentage}
        className={cn("text-base font-medium", !isPercentage && "text-foreground", valueClassName)}
      />
    ) : (
      <GainAmount
        value={value}
        currency={currency}
        displayCurrency={false}
        className={cn("text-base font-medium", valueClassName)}
      />
    );

  const labelContent = labelComponent ?? (
    <div className="text-muted-foreground flex w-full items-center justify-center text-xs">
      <span className="text-center">{label}</span>
      <Popover>
        <PopoverTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            className="ml-1 hidden h-4 w-4 rounded-full p-0 md:inline-flex"
          >
            <Icons.Info className="h-3 w-3" />
            <span className="sr-only">More info about {label}</span>
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-60 text-xs" side="top" align="center">
          {infoText}
        </PopoverContent>
      </Popover>
    </div>
  );

  const content = (
    <>
      {labelContent}

      {annualizedValue !== undefined && annualizedValue !== null && value !== undefined ? (
        <TooltipProvider delayDuration={100}>
          <Tooltip>
            <TooltipTrigger asChild>
              <div className="cursor-help">{displayValue}</div>
            </TooltipTrigger>
            <TooltipContent>
              <p className="text-xs">
                Annualized: <GainPercent value={annualizedValue} animated={false} />
              </p>
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>
      ) : (
        <div>{displayValue}</div>
      )}

      {value === undefined && emptyReason && (
        <div className="text-muted-foreground line-clamp-2 max-w-[11rem] text-center text-[10px] leading-tight">
          {emptyReason}
        </div>
      )}
    </>
  );

  return (
    <Popover open={mobilePopoverOpen} onOpenChange={setMobilePopoverOpen}>
      <PopoverTrigger asChild>
        <div
          className={cn(
            "flex min-h-16 flex-col items-center justify-center space-y-1 p-4 md:cursor-default md:p-4",
            "cursor-pointer md:cursor-auto",
            className,
          )}
        >
          {content}
        </div>
      </PopoverTrigger>
      <PopoverContent className="w-60 text-xs md:hidden" side="top" align="center">
        {infoText}
      </PopoverContent>
    </Popover>
  );
};

// Simple component for displaying only the label with info popover
// This can be used by performance-page.tsx
export interface MetricLabelWithInfoProps {
  label: string;
  infoText: string;
  warningText?: string | string[];
  className?: string;
}

export const MetricLabelWithInfo: React.FC<MetricLabelWithInfoProps> = ({
  label,
  infoText,
  warningText,
  className,
}) => {
  const warningItems = (Array.isArray(warningText) ? warningText : warningText ? [warningText] : [])
    .map((warning) => warning.trim())
    .filter(Boolean);
  const hasWarnings = warningItems.length > 0;

  return (
    <div className={cn("text-muted-foreground flex items-center text-xs font-light", className)}>
      <span>{label}</span>
      <Popover>
        <PopoverTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            className={cn(
              "ml-1 h-4 w-4 rounded-full p-0",
              hasWarnings && "text-warning hover:text-warning",
            )}
          >
            {hasWarnings ? (
              <Icons.AlertTriangle className="h-3 w-3" />
            ) : (
              <Icons.Info className="h-3 w-3" />
            )}
            <span className="sr-only">
              {hasWarnings ? "Calculation note for" : "More info about"} {label}
            </span>
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-72 text-xs" side="top" align="center">
          <div className="space-y-2">
            <p>{infoText}</p>
            {hasWarnings && (
              <div className="border-warning/30 text-warning space-y-1 border-t pt-2">
                {warningItems.map((warning, index) => (
                  <p key={`${warning}-${index}`}>{warning}</p>
                ))}
              </div>
            )}
          </div>
        </PopoverContent>
      </Popover>
    </div>
  );
};
