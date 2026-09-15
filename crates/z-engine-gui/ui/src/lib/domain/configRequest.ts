export function shouldApplyConfigResponse<T>(
  alive: boolean,
  requestSnapshot: T,
  currentSnapshot: T,
): boolean {
  return alive && currentSnapshot === requestSnapshot;
}
