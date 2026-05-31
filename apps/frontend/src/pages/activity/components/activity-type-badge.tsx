import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { ActivityType, ActivityTypeNames, SUBTYPE_DISPLAY_NAMES } from "@/lib/constants";
import { cn } from "@/lib/utils";

interface ActivityTypeBadgeProps {
  type: ActivityType;
  subtype?: string | null;
  className?: string;
}

function getActivityBadgeVariant(type: ActivityType) {
  switch (type) {
    case ActivityType.DIVIDEND:
    case ActivityType.INTEREST:
    case ActivityType.BUY:
    case ActivityType.DEPOSIT:
    case ActivityType.TRANSFER_IN:
      return "success";
    case ActivityType.SELL:
    case ActivityType.WITHDRAWAL:
    case ActivityType.TRANSFER_OUT:
    case ActivityType.FEE:
    case ActivityType.TAX:
      return "destructive";
    case ActivityType.SPLIT:
    case ActivityType.ADJUSTMENT:
      return "secondary";
    default:
      return "default";
  }
}

export function ActivityTypeBadge({ type, subtype, className }: ActivityTypeBadgeProps) {
  const variant = getActivityBadgeVariant(type);
  const normalizedSubtype = subtype?.trim().toUpperCase();
  const subtypeLabel = normalizedSubtype
    ? (SUBTYPE_DISPLAY_NAMES[normalizedSubtype] ?? subtype)
    : undefined;

  return (
    <span className="inline-flex max-w-full min-w-0 items-center gap-1.5 overflow-hidden">
      <Badge variant={variant} className={cn("rounded-sm", className)}>
        {ActivityTypeNames[type]}
      </Badge>
      {subtypeLabel && (
        <span className="text-muted-foreground min-w-0 flex-1 truncate text-xs font-normal">
          {subtypeLabel}
        </span>
      )}
    </span>
  );
}
