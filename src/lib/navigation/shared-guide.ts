export function hasSelectedWaypoint(settings: Record<string, unknown>): boolean {
  const navigation = settings.navigation as Record<string, unknown> | undefined;
  const waypointId = navigation?.target_waypoint_id;
  return typeof waypointId === "string" && waypointId.trim().length > 0;
}

export function sharedGuideRequested(
  waterGuideRequested: boolean,
  waypointGuideRequested: boolean,
): boolean {
  return waterGuideRequested || waypointGuideRequested;
}
