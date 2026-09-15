export const MAX_TASK_CONTINUATIONS = 10;
export const DEFAULT_TASK_CONTINUATIONS = 3;

export function taskContinuationLimitError(value: number | undefined): string | null {
  return value !== undefined && Number.isInteger(value) &&
    value >= 0 && value <= MAX_TASK_CONTINUATIONS
    ? null
    : "Enter a whole number from 0 to 10. Use 0 to disable bounded continuation.";
}
