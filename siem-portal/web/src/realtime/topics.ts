/** Canonical topic keys — must match `siem-portal/src/realtime.rs` `fetch_snapshot`. */

export function rtUiConfig(): string {
  return "ui.config";
}

export function rtStackStatus(): string {
  return "stack.status";
}

export function rtOverview(hours: number): string {
  return `overview:h:${hours}`;
}

export function rtInfrastructure(hours: number): string {
  return `infrastructure:h:${hours}`;
}

export function rtOperations(hours: number): string {
  return `operations:h:${hours}`;
}

export function rtDataQuality(hours: number): string {
  return `data_quality:h:${hours}`;
}

export function rtAlertsOverview(): string {
  return "alerts.overview";
}

/** Raw Alertmanager `/api/v2/alerts` JSON array (same as `getAlerts()`). */
export function rtAlertmanagerAlerts(): string {
  return "alertmanager.alerts";
}

export function rtDetectionsOverview(): string {
  return "detections.overview";
}

export function rtCorrelatorStats(): string {
  return "correlator.stats";
}

export function rtCorrelatorRules(): string {
  return "correlator.rules";
}

export function rtCasesList(params: { status?: string; severity?: string; q?: string; assignee?: string }): string {
  const p = new URLSearchParams();
  if (params.status) p.set("status", params.status);
  if (params.severity) p.set("severity", params.severity);
  if (params.q) p.set("q", params.q);
  if (params.assignee) p.set("assignee", params.assignee);
  const qs = p.toString();
  return qs ? `cases.list?${qs}` : "cases.list";
}

export function rtCaseDetail(caseId: string): string {
  return `case.detail:${caseId}`;
}

export function rtCaseInvestigate(caseId: string): string {
  return `case.investigate:${caseId}`;
}

/** Query string as returned by `URLSearchParams.toString()` (committed search / URL bar). */
export function rtEventsSearch(searchParams: URLSearchParams | Record<string, string>): string {
  const p = searchParams instanceof URLSearchParams ? searchParams : new URLSearchParams(searchParams);
  const qs = p.toString();
  return qs ? `events.search?${qs}` : "events.search";
}

export function rtEventDetail(eventId: string): string {
  return `event.detail:${eventId}`;
}

export function rtEntityContext(kind: string, value: string): string {
  return `entity.context:${JSON.stringify({ kind, value })}`;
}
