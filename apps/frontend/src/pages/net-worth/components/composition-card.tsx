import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@wealthfolio/ui/components/ui/tooltip";
import { useMemo } from "react";
import { SectionCard } from "./section-card";
import { CATEGORY_COLORS, CATEGORY_CSS_COLORS, type ParsedNetWorth } from "./utils";

interface CompositionItem {
  category: string;
  name: string;
  percentage: number;
}

export function CompositionCard({ data }: { data: ParsedNetWorth }) {
  const items = useMemo((): CompositionItem[] => {
    if (data.assets.total === 0) return [];
    return data.assets.breakdown
      .filter((item) => item.value > 0)
      .map((item) => ({
        category: item.category,
        name: item.name,
        percentage: (item.value / data.assets.total) * 100,
      }))
      .sort((a, b) => b.percentage - a.percentage);
  }, [data]);

  if (items.length === 0) return null;

  return (
    <SectionCard title="Composition" meta="% of assets">
      {/* Stacked bar */}
      <div className="mb-4 flex h-2.5 w-full overflow-hidden rounded-full">
        {items.map((item, index) => (
          <TooltipProvider key={item.category} delayDuration={0}>
            <Tooltip>
              <TooltipTrigger asChild>
                <div
                  className="h-full transition-opacity hover:opacity-80"
                  style={{
                    width: `${item.percentage}%`,
                    backgroundColor:
                      CATEGORY_CSS_COLORS[item.category] ?? "var(--muted-foreground)",
                    marginLeft: index > 0 ? "1px" : 0,
                  }}
                />
              </TooltipTrigger>
              <TooltipContent side="bottom" className="text-xs">
                <span className="font-medium">{item.name}</span>
                <span className="text-muted-foreground ml-2">{item.percentage.toFixed(1)}%</span>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        ))}
      </div>

      {/* Legend */}
      <div className="grid grid-cols-2 gap-x-4 gap-y-1.5">
        {items.map((item) => (
          <div key={item.category} className="flex items-center gap-2">
            <div
              className={`h-2.5 w-2.5 shrink-0 rounded-full ${CATEGORY_COLORS[item.category] ?? "bg-muted-foreground"}`}
            />
            <span className="text-muted-foreground truncate text-xs">{item.name}</span>
            <span className="text-muted-foreground/60 ml-auto text-xs tabular-nums">
              {item.percentage.toFixed(1)}%
            </span>
          </div>
        ))}
      </div>
    </SectionCard>
  );
}
