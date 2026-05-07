import { Link } from "react-router-dom";

import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Icons,
} from "@wealthfolio/ui";

export interface OverviewChip {
  id: string;
  name: string;
  color?: string | null;
}

interface OverviewCardProps {
  title: string;
  description?: string;
  chips: OverviewChip[];
  totalCount?: number;
  manageHref: string;
  manageLabel?: string;
  emptyTitle: string;
  emptyDescription: string;
  emptyCtaLabel: string;
  isLoading?: boolean;
  maxVisible?: number;
}

export function OverviewCard({
  title,
  description,
  chips,
  totalCount,
  manageHref,
  manageLabel = "Manage",
  emptyTitle,
  emptyDescription,
  emptyCtaLabel,
  isLoading = false,
  maxVisible = 7,
}: OverviewCardProps) {
  const visible = chips.slice(0, maxVisible);
  const overflow = Math.max(0, chips.length - visible.length);
  const isEmpty = !isLoading && chips.length === 0;

  return (
    <Card>
      <CardHeader className="flex flex-row items-start justify-between gap-2 space-y-0 p-4 pb-3">
        <div className="min-w-0 space-y-0.5">
          <CardTitle className="text-sm font-medium">{title}</CardTitle>
          {description && <CardDescription className="text-xs">{description}</CardDescription>}
        </div>
        {!isEmpty && (
          <Button asChild variant="ghost" size="sm" className="-mt-1 shrink-0">
            <Link to={manageHref}>
              {manageLabel}
              <Icons.ChevronRight className="ml-1 h-3.5 w-3.5" />
            </Link>
          </Button>
        )}
      </CardHeader>
      <CardContent className="p-4 pt-0">
        {isLoading ? (
          <div className="bg-muted/40 h-6 w-full animate-pulse rounded-md" />
        ) : isEmpty ? (
          <div className="space-y-3 py-2">
            <div>
              <div className="text-foreground text-sm font-medium">{emptyTitle}</div>
              <p className="text-muted-foreground text-xs">{emptyDescription}</p>
            </div>
            <Button asChild size="sm">
              <Link to={manageHref}>
                <Icons.Plus className="mr-1.5 h-3.5 w-3.5" />
                {emptyCtaLabel}
              </Link>
            </Button>
          </div>
        ) : (
          <div className="flex flex-wrap items-center gap-1.5">
            {visible.map((chip) => (
              <span
                key={chip.id}
                className="bg-muted/60 text-foreground inline-flex max-w-[200px] items-center gap-1.5 rounded-full px-2 py-0.5 text-xs"
              >
                <span
                  className="h-2 w-2 shrink-0 rounded-full"
                  style={{ backgroundColor: chip.color ?? "var(--muted-foreground)" }}
                />
                <span className="truncate">{chip.name}</span>
              </span>
            ))}
            {overflow > 0 && (
              <span className="text-muted-foreground text-xs">+{overflow} more</span>
            )}
            {totalCount !== undefined && (
              <span className="text-muted-foreground ml-auto text-[11px]">{totalCount} total</span>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
