export function hasReviewableActivityWarnings(warnings: number | null | undefined): boolean {
  return (warnings ?? 0) > 0;
}
