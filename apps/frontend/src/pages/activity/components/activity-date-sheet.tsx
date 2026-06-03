import type { ActivityDetails } from "@/lib/types";
import { Icons } from "@wealthfolio/ui";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@wealthfolio/ui/components/ui/sheet";
import { format, parseISO } from "date-fns";
import { ActivityDateList } from "./activity-date-list";

interface ActivityDateSheetProps {
  activities: ActivityDetails[];
  date: string | null;
  isLoading: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function ActivityDateSheet({
  activities,
  date,
  isLoading,
  open,
  onOpenChange,
}: ActivityDateSheetProps) {
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="flex h-full w-full flex-col p-0 sm:max-w-xl">
        <SheetHeader className="border-b px-6 py-4">
          <SheetTitle>Activities on {date ? formatActivityDate(date) : ""}</SheetTitle>
          <SheetDescription>{activities.length} activities recorded on this date</SheetDescription>
        </SheetHeader>
        <div className="flex-1 overflow-auto px-4 py-4">
          {isLoading ? (
            <div className="flex items-center justify-center py-8">
              <Icons.Spinner className="size-6 animate-spin" />
            </div>
          ) : (
            <ActivityDateList activities={activities} />
          )}
        </div>
      </SheetContent>
    </Sheet>
  );
}

function formatActivityDate(date: string): string {
  try {
    return format(parseISO(date), "MMMM d, yyyy");
  } catch {
    return date;
  }
}
