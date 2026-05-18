import { useState } from "react";
import { useNavigate } from "react-router-dom";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
  Button,
  Icons,
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@wealthfolio/ui";
import type { TaxonomyCategory } from "@/lib/types";

import { buildCashflowUrl } from "../lib/navigation";
import { CategoryIcon } from "./category-chips";

export interface CategoryNode extends TaxonomyCategory {
  children?: CategoryNode[];
}

interface CategoryItemProps {
  category: CategoryNode;
  children?: CategoryNode[];
  onEdit: (category: CategoryNode) => void;
  onDelete: (category: CategoryNode) => void;
  onAddSubcategory: (parentCategory: CategoryNode) => void;
  isSubcategory?: boolean;
  activityCounts?: Record<string, number>;
}

export function CategoryItem({
  category,
  children,
  onEdit,
  onDelete,
  onAddSubcategory,
  isSubcategory = false,
  activityCounts,
}: CategoryItemProps) {
  const navigate = useNavigate();
  const [isExpanded, setIsExpanded] = useState(false);
  const hasChildren = children && children.length > 0;
  const activityCount = activityCounts?.[category.id] ?? 0;

  const handleViewTransactions = () => {
    navigate(
      buildCashflowUrl({
        categoryId: isSubcategory ? undefined : category.id,
        subcategoryId: isSubcategory ? category.id : undefined,
      }),
    );
  };

  return (
    <div className={isSubcategory ? "ml-6 border-l pl-4" : ""}>
      <div className="flex items-center justify-between py-3">
        <div className="flex items-center gap-3">
          {hasChildren && !isSubcategory && (
            <Button
              variant="ghost"
              size="sm"
              className="h-6 w-6 p-0"
              onClick={() => setIsExpanded(!isExpanded)}
            >
              {isExpanded ? (
                <Icons.ChevronDown className="h-4 w-4" />
              ) : (
                <Icons.ChevronRight className="h-4 w-4" />
              )}
            </Button>
          )}
          {!hasChildren && !isSubcategory && <div className="w-6" />}
          <span
            className={`flex shrink-0 items-center justify-center rounded-full ${
              isSubcategory ? "h-6 w-6" : "h-7 w-7"
            }`}
            style={{
              backgroundColor: category.color ? `${category.color}1F` : "var(--muted)",
              color: category.color ?? "var(--muted-foreground)",
            }}
          >
            <CategoryIcon
              icon={category.icon ?? null}
              fallback={category.name}
              className={isSubcategory ? "h-3 w-3" : "h-3.5 w-3.5"}
            />
          </span>
          <span className={isSubcategory ? "text-sm" : "font-medium"}>{category.name}</span>
          {activityCount > 0 && (
            <TooltipProvider>
              <Tooltip>
                <TooltipTrigger asChild>
                  <span className="text-muted-foreground cursor-default text-xs">
                    ({activityCount})
                  </span>
                </TooltipTrigger>
                <TooltipContent>
                  <p>
                    {activityCount} transaction{activityCount !== 1 ? "s" : ""}
                  </p>
                </TooltipContent>
              </Tooltip>
            </TooltipProvider>
          )}
        </div>
        <div className="flex items-center gap-1">
          {!isSubcategory && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => onAddSubcategory(category)}
              title="Add subcategory"
            >
              <Icons.Plus className="h-4 w-4" />
            </Button>
          )}
          <Button
            variant="ghost"
            size="sm"
            onClick={handleViewTransactions}
            title="View transactions"
          >
            <Icons.ExternalLink className="h-4 w-4" />
          </Button>
          <Button variant="ghost" size="sm" onClick={() => onEdit(category)} title="Edit category">
            <Icons.Pencil className="h-4 w-4" />
          </Button>
          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button variant="ghost" size="sm" title="Delete category">
                <Icons.Trash className="h-4 w-4" />
              </Button>
            </AlertDialogTrigger>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>Delete Category</AlertDialogTitle>
                <AlertDialogDescription>
                  Are you sure you want to delete &quot;{category.name}&quot;?
                  {hasChildren && (
                    <span className="text-destructive mt-2 block font-medium">
                      This will also delete all subcategories.
                    </span>
                  )}
                  This action cannot be undone.
                </AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>Cancel</AlertDialogCancel>
                <AlertDialogAction
                  onClick={() => onDelete(category)}
                  className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                >
                  Delete
                </AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
        </div>
      </div>
      {hasChildren && isExpanded && (
        <div className="space-y-0">
          {children.map((child) => (
            <CategoryItem
              key={child.id}
              category={child}
              // eslint-disable-next-line react/no-children-prop
              children={child.children}
              onEdit={onEdit}
              onDelete={onDelete}
              onAddSubcategory={onAddSubcategory}
              isSubcategory
              activityCounts={activityCounts}
            />
          ))}
        </div>
      )}
    </div>
  );
}
