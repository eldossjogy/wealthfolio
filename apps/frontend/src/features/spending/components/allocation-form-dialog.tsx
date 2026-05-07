import { useEffect, useState } from "react";

import {
  Button,
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  Label,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@wealthfolio/ui";
import type { TaxonomyCategory } from "@/lib/types";

import type { AllocationWithCategory } from "./allocation-list";

interface AllocationFormDialogProps {
  open: boolean;
  onClose: () => void;
  /** Called with (categoryId, amount) — taxonomy_id is implied by isIncome. */
  onSave: (categoryId: string, amount: string) => void;
  /** Categories available for this taxonomy (already filtered to top-level by the page). */
  categories: TaxonomyCategory[];
  existingAllocations: AllocationWithCategory[];
  editingAllocation?: AllocationWithCategory;
  isIncome: boolean;
  isPending?: boolean;
}

export function AllocationFormDialog({
  open,
  onClose,
  onSave,
  categories,
  existingAllocations,
  editingAllocation,
  isIncome,
  isPending,
}: AllocationFormDialogProps) {
  const [selectedCategoryId, setSelectedCategoryId] = useState<string>("");
  const [amount, setAmount] = useState<string>("");

  const availableCategories = categories.filter((cat) => {
    if (editingAllocation && cat.id === editingAllocation.categoryId) return true;
    return !existingAllocations.some((alloc) => alloc.categoryId === cat.id);
  });

  useEffect(() => {
    if (editingAllocation) {
      setSelectedCategoryId(editingAllocation.categoryId);
      setAmount(editingAllocation.amount.toString());
    } else {
      setSelectedCategoryId("");
      setAmount("");
    }
  }, [editingAllocation, open]);

  const handleSave = () => {
    if (selectedCategoryId && amount) {
      const numAmount = parseFloat(amount);
      if (!isNaN(numAmount) && numAmount > 0) {
        onSave(selectedCategoryId, amount);
        onClose();
      }
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && selectedCategoryId && amount) handleSave();
  };

  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent className="sm:max-w-[425px]">
        <DialogHeader>
          <DialogTitle>
            {editingAllocation ? "Edit" : "Add"} {isIncome ? "Income" : "Expense"} Allocation
          </DialogTitle>
        </DialogHeader>
        <div className="grid gap-4 py-4">
          <div className="grid gap-2">
            <Label htmlFor="category">Category</Label>
            <Select
              value={selectedCategoryId}
              onValueChange={setSelectedCategoryId}
              disabled={!!editingAllocation}
            >
              <SelectTrigger id="category">
                <SelectValue placeholder="Select a category" />
              </SelectTrigger>
              <SelectContent>
                {availableCategories.map((cat) => (
                  <SelectItem key={cat.id} value={cat.id}>
                    <div className="flex items-center gap-2">
                      {cat.color && (
                        <span
                          className="h-3 w-3 rounded-full"
                          style={{ backgroundColor: cat.color }}
                        />
                      )}
                      {cat.name}
                    </div>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="grid gap-2">
            <Label htmlFor="amount">Monthly Amount</Label>
            <Input
              id="amount"
              type="number"
              min="0"
              step="0.01"
              placeholder="0.00"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              onKeyDown={handleKeyDown}
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={!selectedCategoryId || !amount || isPending}>
            {isPending ? "Saving..." : "Save"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
