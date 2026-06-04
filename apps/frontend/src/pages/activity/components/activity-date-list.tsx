import { TickerAvatar } from "@/components/ticker-avatar";
import {
  calculateActivityValue,
  isAssetBackedIncomeActivity,
  isCashActivity,
  isCashTransfer,
  isFeeActivity,
  isIncomeActivity,
  isSplitActivity,
} from "@/lib/activity-utils";
import { ActivityType, ActivityTypeNames } from "@/lib/constants";
import { parseOccSymbol } from "@/lib/occ-symbol";
import { useSettingsContext } from "@/lib/settings-provider";
import type { ActivityDetails } from "@/lib/types";
import { cn, formatDateTime } from "@/lib/utils";
import { Card, EmptyPlaceholder, formatAmount } from "@wealthfolio/ui";
import { Link } from "react-router-dom";

interface ActivityDateListProps {
  activities: ActivityDetails[];
}

export function ActivityDateList({ activities }: ActivityDateListProps) {
  const { settings } = useSettingsContext();
  const appTimezone = settings?.timezone?.trim() || undefined;

  if (activities.length === 0) {
    return (
      <EmptyPlaceholder>
        <EmptyPlaceholder.Icon name="Activity" />
        <EmptyPlaceholder.Title>No activities</EmptyPlaceholder.Title>
        <EmptyPlaceholder.Description>
          No activities were found for this date.
        </EmptyPlaceholder.Description>
      </EmptyPlaceholder>
    );
  }

  return (
    <div className="space-y-2">
      {activities.map((activity) => (
        <ActivityDateListItem key={activity.id} activity={activity} appTimezone={appTimezone} />
      ))}
    </div>
  );
}

interface ActivityDateListItemProps {
  activity: ActivityDetails;
  appTimezone?: string;
}

function ActivityDateListItem({ activity, appTimezone }: ActivityDateListItemProps) {
  const symbol = activity.assetSymbol;
  const activityType = activity.activityType;
  const isTransferActivity =
    activityType === ActivityType.TRANSFER_IN || activityType === ActivityType.TRANSFER_OUT;
  const isAssetBackedIncome = isAssetBackedIncomeActivity(activityType, symbol, activity.assetId);
  const isCash = isTransferActivity
    ? isCashTransfer(activityType, symbol, activity.assetId)
    : isCashActivity(activityType) && !isAssetBackedIncome;
  const hasAsset = Boolean(activity.assetId?.trim());
  const isOptionActivity = activity.instrumentType === "OPTION";
  const parsedOption = isOptionActivity ? parseOccSymbol(symbol) : null;
  const displaySymbol = isCash ? "Cash" : parsedOption ? parsedOption.underlying : symbol;
  const avatarSymbol = isCash ? "$CASH" : symbol;
  const optionSubtitle = parsedOption
    ? `${new Date(parsedOption.expiration + "T12:00:00").toLocaleDateString("en-US", { month: "short", day: "numeric" })} $${parsedOption.strikePrice} ${parsedOption.optionType}`
    : null;
  const formattedDate = formatDateTime(activity.date, appTimezone);
  const displayValue = calculateActivityValue(activity);
  const activityTypeLabel = ActivityTypeNames[activity.activityType];
  const activityTone = getActivityTone(activity.activityType);
  const quantityLabel =
    !isCash &&
    !(isIncomeActivity(activity.activityType) && !isAssetBackedIncome) &&
    !isSplitActivity(activity.activityType) &&
    !isFeeActivity(activity.activityType) &&
    activity.quantity
      ? `${activity.quantity} ${isOptionActivity ? "contracts" : "shares"}`
      : null;

  const content = (
    <div className="flex min-w-0 flex-1 items-center gap-3">
      <TickerAvatar symbol={avatarSymbol} className="h-10 w-10 flex-shrink-0" />
      <div className="grid min-w-0 flex-1 grid-cols-[minmax(0,1fr)_auto] gap-x-3">
        <p className="truncate text-base font-semibold leading-5">{displaySymbol}</p>
        {activity.activityType !== ActivityType.SPLIT ? (
          <p className="text-right text-base font-semibold leading-5">
            {formatAmount(displayValue, activity.currency)}
          </p>
        ) : (
          <span />
        )}
        <div className="flex min-w-0 items-center gap-1.5 whitespace-nowrap text-sm leading-5">
          <span className={cn("font-semibold", activityTone.text)}>{activityTypeLabel}</span>
          {activity.accountName ? (
            <span className="text-muted-foreground min-w-0 truncate">{activity.accountName}</span>
          ) : null}
          <span className="text-muted-foreground shrink-0">•</span>
          <span className="text-muted-foreground shrink-0">{formattedDate.date}</span>
          {optionSubtitle ? (
            <>
              <span className="text-muted-foreground shrink-0">•</span>
              <span className="text-muted-foreground truncate">{optionSubtitle}</span>
            </>
          ) : null}
        </div>
        {activity.activityType !== ActivityType.SPLIT ? (
          <p className="text-muted-foreground text-right text-sm leading-5">{quantityLabel}</p>
        ) : null}
      </div>
    </div>
  );

  return (
    <Card key={activity.id} className="p-0">
      {isCash || !hasAsset ? (
        <div className="p-4">{content}</div>
      ) : (
        <Link to={`/holdings/${encodeURIComponent(activity.assetId)}`} className="block p-4">
          {content}
        </Link>
      )}
    </Card>
  );
}

function getActivityTone(type: ActivityType) {
  switch (type) {
    case ActivityType.BUY:
    case ActivityType.DEPOSIT:
    case ActivityType.TRANSFER_IN:
    case ActivityType.DIVIDEND:
    case ActivityType.INTEREST:
      return {
        text: "text-success",
      };
    case ActivityType.SELL:
    case ActivityType.WITHDRAWAL:
    case ActivityType.TRANSFER_OUT:
    case ActivityType.FEE:
    case ActivityType.TAX:
      return {
        text: "text-destructive",
      };
    default:
      return {
        text: "text-muted-foreground",
      };
  }
}
