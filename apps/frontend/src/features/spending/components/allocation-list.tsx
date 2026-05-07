import { Button, Icons } from "@wealthfolio/ui";
import { formatAmount } from "@wealthfolio/ui";

import type { BudgetAllocation } from "../types/budget";

export interface AllocationWithCategory extends BudgetAllocation {
  categoryName: string;
  categoryColor?: string | null;
}

interface AllocationListProps {
  allocations: AllocationWithCategory[];
  unallocated: number;
  currency: string;
  onEdit: (allocation: AllocationWithCategory) => void;
  onDelete: (allocationId: string) => void;
  isDeleting?: boolean;
}

export function AllocationList({
  allocations,
  unallocated,
  currency,
  onEdit,
  onDelete,
  isDeleting,
}: AllocationListProps) {
  return (
    <div className="space-y-1">
      {allocations.map((allocation) => (
        <div
          key={allocation.id}
          className="hover:bg-muted/50 group flex items-center justify-between rounded-lg border p-3 transition-colors"
        >
          <div className="flex items-center gap-3">
            <div
              className="h-3 w-3 rounded-full"
              style={{ backgroundColor: allocation.categoryColor || "#888" }}
            />
            <span className="font-medium">{allocation.categoryName}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-muted-foreground">
              {formatAmount(parseFloat(allocation.amount) || 0, currency)}
            </span>
            <div className="flex gap-1 opacity-0 transition-opacity group-hover:opacity-100">
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8"
                onClick={() => onEdit(allocation)}
              >
                <Icons.Pencil className="h-4 w-4" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className="text-destructive hover:text-destructive h-8 w-8"
                onClick={() => onDelete(allocation.id)}
                disabled={isDeleting}
              >
                <Icons.Trash className="h-4 w-4" />
              </Button>
            </div>
          </div>
        </div>
      ))}

      {unallocated > 0 && (
        <div className="text-muted-foreground flex items-center justify-between rounded-lg border border-dashed p-3">
          <div className="flex items-center gap-3">
            <div className="h-3 w-3 rounded-full border-2 border-current bg-transparent" />
            <div className="flex flex-col">
              <span className="font-medium">Flexible</span>
              <span className="text-xs">Applied to remaining categories</span>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <span>{formatAmount(unallocated, currency)}</span>
            <div className="w-[72px]" />
          </div>
        </div>
      )}

      {allocations.length === 0 && unallocated === 0 && (
        <div className="text-muted-foreground py-8 text-center">
          <p>No allocations yet. Set a target above to get started.</p>
        </div>
      )}
    </div>
  );
}
