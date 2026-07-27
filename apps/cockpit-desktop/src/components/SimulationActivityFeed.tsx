import { useMemo, useState } from "react";
import { Ban, Bot, Check, ChevronLeft, ChevronRight, Download, X, Zap } from "lucide-react";
import { APP_CONFIG } from "../config/constants";
import { useSimulator } from "../hooks/useSimulator";
import {
  exportActionResultsAsJSON,
  exportEventsAsCSV,
  exportEventsAsJSON,
  exportTracesAsCSV,
  exportTracesAsJSON
} from "../utils/export";
import { simulatorClient } from "../simulatorClient";
import type { SimulationAction } from "../state/simulationReducer";
import type {
  ActionResult,
  HumanTurnTrace,
  SimulationEvent,
  SimulationModel,
  ToolCallTrace
} from "../types/simulation";
import { useI18n } from "../i18n";
import { describeError } from "../utils/describeError";
import {
  actionStatusLabel,
  capabilityLabel,
  commandLabel,
  eventLabel,
  isScenarioInteractionEvent
} from "../utils/domainPresentation";

interface Props {
  model: SimulationModel;
  dispatch: React.Dispatch<SimulationAction>;
}

/// Unified, chronologically-merged replacement for what used to be two
/// separate panels: Timeline (raw SimulationEvents) and Agent Trace (tool
/// calls + action results). Both were answering the same question - "what
/// just happened in this run" - from different angles, which made it unclear
/// where to look first. Merging them into one feed, ordered by tick with a
/// severity/emphasis cue, gives a single place to watch and makes pending
/// approvals impossible to miss since they're inline instead of in a
/// separately-scrolled panel.
type FeedItem =
  | { kind: "event"; tick: number; sequence: number; data: SimulationEvent }
  | { kind: "toolCall"; tick: number; sequence: number; data: ToolCallTrace }
  | { kind: "humanTurn"; tick: number; sequence: number; data: HumanTurnTrace }
  | { kind: "actionResult"; tick: number; sequence: number; data: ActionResult };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isPendingApproval(result: unknown): boolean {
  if (!isRecord(result) || !isRecord(result.result)) return false;
  return result.result.status === "pendingApproval";
}

function pendingRequestId(trace: ToolCallTrace, actionResults: ActionResult[]): string | undefined {
  if (trace.toolName !== "simulation.request_action" || !isPendingApproval(trace.result)) return undefined;
  return actionResults.some((result) => result.request.requestId === trace.callId) ? undefined : trace.callId;
}

function buildFeed(model: SimulationModel): FeedItem[] {
  const items: FeedItem[] = [
    ...model.events.map((event) => ({ kind: "event" as const, tick: event.tick, sequence: event.sequence, data: event })),
    ...model.toolCalls.map((trace, index) => ({ kind: "toolCall" as const, tick: trace.tick, sequence: -index, data: trace })),
    ...model.humanTurns.map((turn, index) => ({
      kind: "humanTurn" as const,
      tick: turn.tick,
      sequence: -index,
      data: turn
    })),
    ...model.actionResults.map((result, index) => ({
      kind: "actionResult" as const,
      tick: result.tick,
      sequence: -index,
      data: result
    }))
  ];
  // Keep the newest step at the top. Tool/action arrays already arrive
  // newest-first, so the synthetic sequence preserves their local order.
  return items.sort((a, b) => (b.tick - a.tick) || (b.sequence - a.sequence));
}

interface StoryStep {
  tick: number;
  events: SimulationEvent[];
  humanTurns: HumanTurnTrace[];
  actionResults: ActionResult[];
  toolCalls: ToolCallTrace[];
}

function isBackgroundEvent(event: SimulationEvent): boolean {
  return [
    "HumanStateDeltaApplied",
    "HumanPhysiologyUpdated",
    "CabinAirQualityUpdated",
    "CabinPressureUpdated",
    "CabinTemperatureChanged",
    "StateDiffApplied",
    "InfluenceApplied"
  ].includes(event.eventType);
}

function buildStorySteps(feed: FeedItem[]): StoryStep[] {
  const steps = new Map<number, StoryStep>();
  for (const item of feed) {
    const step = steps.get(item.tick) ?? { tick: item.tick, events: [], humanTurns: [], actionResults: [], toolCalls: [] };
    if (item.kind === "event" && !isBackgroundEvent(item.data)) step.events.push(item.data);
    else if (item.kind === "humanTurn" && item.data.evidence.decision.actions.length > 0) step.humanTurns.push(item.data);
    else if (item.kind === "actionResult") step.actionResults.push(item.data);
    else if (item.kind === "toolCall") step.toolCalls.push(item.data);
    steps.set(item.tick, step);
  }
  return [...steps.values()]
    .filter((step) => step.events.length > 0 || step.humanTurns.length > 0 || step.actionResults.length > 0 || step.toolCalls.some((trace) => trace.toolName === "simulation.request_action" && isPendingApproval(trace.result)))
    .sort((left, right) => right.tick - left.tick);
}

function compactChange(event: SimulationEvent, model: SimulationModel, locale: ReturnType<typeof useI18n>["locale"]): string {
  if (isScenarioInteractionEvent(event.eventType)) {
    return event.payload.message;
  }
  const target = event.payload.target;
  if (!target) return eventLabel(event.eventType, locale);
  const human = model.snapshot?.humans.find((candidate) => candidate.id === target);
  return `${human?.persona.name ?? target}：${eventLabel(event.eventType, locale)}`;
}

export function SimulationActivityFeed({ model, dispatch }: Props) {
  const { locale, t } = useI18n();
  const { syncEvents } = useSimulator(model, dispatch);
  const [showExportMenu, setShowExportMenu] = useState(false);
  const [page, setPage] = useState(0);
  const [loadingEarlierEvidence, setLoadingEarlierEvidence] = useState(false);

  const feed = useMemo(
    () => buildFeed(model),
    [model.events, model.toolCalls, model.humanTurns, model.actionResults]
  );
  const storySteps = useMemo(() => buildStorySteps(feed), [feed]);
  const pendingCount = model.toolCalls.filter((trace) => pendingRequestId(trace, model.actionResults)).length;

  const totalPages = Math.max(1, Math.ceil(storySteps.length / APP_CONFIG.EVENTS_PER_PAGE));
  const startIndex = page * APP_CONFIG.EVENTS_PER_PAGE;
  const displayed = storySteps.slice(startIndex, startIndex + APP_CONFIG.EVENTS_PER_PAGE);

  async function resolve(requestId: string, decision: "approve" | "reject") {
    try {
      if (decision === "approve") await simulatorClient.approveAction(requestId);
      else await simulatorClient.rejectAction(requestId, t("operatorRejectedReason"));
      await syncEvents();
    } catch (error) {
      dispatch({
        type: "commandRejected",
        error: {
          code: "SIMULATOR_COMMAND_FAILED",
          message: describeError(error, t("approvalCommandFailed")),
          correlationId: "desktop-approval",
          runId: model.runId,
          tick: model.tick
        }
      });
    }
  }

  async function cancelPending() {
    try {
      await simulatorClient.cancelAgentTurn();
      await syncEvents();
    } catch (error) {
      dispatch({
        type: "commandRejected",
        error: {
          code: "SIMULATOR_COMMAND_FAILED",
          message: describeError(error, t("cancelCommandFailed")),
          correlationId: "desktop-cancel",
          runId: model.runId,
          tick: model.tick
        }
      });
    }
  }

  async function loadEarlierEvidence() {
    if (!model.runId || !model.auditRecovery || model.auditRecovery.earliestOffset === 0) return;
    setLoadingEarlierEvidence(true);
    try {
      const offset = Math.max(0, model.auditRecovery.earliestOffset - 256);
      const audit = await simulatorClient.recordedAuditPage({
        runId: model.runId,
        startTick: 0,
        endTick: model.tick,
        offset
      });
      dispatch({
        type: "recordedAuditPage",
        events: audit.events.map((item) => item.event),
        totalEvents: audit.totalEvents,
        earliestOffset: audit.offset
      });
    } catch (error) {
      dispatch({
        type: "commandRejected",
        error: {
          code: "RECORDED_AUDIT_LOAD_FAILED",
          message: describeError(error, t("auditLoadFailed")),
          correlationId: "desktop-recorded-audit",
          runId: model.runId,
          tick: model.tick
        }
      });
    } finally {
      setLoadingEarlierEvidence(false);
    }
  }

  return (
    <section className="flex h-full min-w-0 flex-col overflow-hidden rounded-xl border border-zinc-800/90 bg-zinc-900/60 backdrop-blur-md shadow-sm">
      <div className="flex shrink-0 items-center justify-between border-b border-zinc-800/80 bg-zinc-900/80 px-3.5 py-2 text-xs font-semibold text-zinc-100">
        <div className="flex items-center gap-2">
          <div>
            <div className="tracking-wide">{t("activity")}</div>
            <div className="mt-0.5 text-[10px] font-normal text-zinc-500">{t("activitySubtitle")}</div>
          </div>
          {pendingCount > 0 && (
            <span className="flex items-center gap-1 rounded-full bg-amber-500/90 px-2 py-0.5 text-[10px] font-semibold text-zinc-950">
              {pendingCount} {t("awaitingApproval")}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {totalPages > 1 && (
            <div className="flex items-center gap-1 text-xs text-zinc-400">
              <button
                aria-label={t("previousPage")}
                className="control-button h-[26px] w-[26px] disabled:opacity-30"
                disabled={page === 0}
                onClick={() => setPage(page - 1)}
              >
                <ChevronLeft className="h-3 w-3" />
              </button>
              <span>
                {page + 1} / {totalPages}
              </span>
              <button
                aria-label={t("nextPage")}
                className="control-button h-[26px] w-[26px] disabled:opacity-30"
                disabled={page >= totalPages - 1}
                onClick={() => setPage(page + 1)}
              >
                <ChevronRight className="h-3 w-3" />
              </button>
            </div>
          )}
          {storySteps.length > 0 && (
            <div className="relative">
              <button
                aria-label={t("exportActivity")}
                className="control-button h-[26px] w-[26px]"
                onClick={() => setShowExportMenu(!showExportMenu)}
              >
                <Download className="h-3 w-3" />
              </button>
              {showExportMenu && (
                <div className="absolute right-0 top-8 z-10 flex flex-col border border-zinc-700 bg-zinc-900 text-xs">
                  <button
                    className="px-3 py-2 text-left hover:bg-zinc-800"
                    onClick={() => {
                      exportEventsAsJSON(model.events);
                      setShowExportMenu(false);
                    }}
                  >
                    {t("exportEventsJson")}
                  </button>
                  <button
                    className="px-3 py-2 text-left hover:bg-zinc-800"
                    onClick={() => {
                      exportEventsAsCSV(model.events);
                      setShowExportMenu(false);
                    }}
                  >
                    {t("exportEventsCsv")}
                  </button>
                  <button
                    className="px-3 py-2 text-left hover:bg-zinc-800"
                    onClick={() => {
                      exportTracesAsJSON(model.toolCalls);
                      setShowExportMenu(false);
                    }}
                  >
                    {t("exportToolsJson")}
                  </button>
                  <button
                    className="px-3 py-2 text-left hover:bg-zinc-800"
                    onClick={() => {
                      exportTracesAsCSV(model.toolCalls);
                      setShowExportMenu(false);
                    }}
                  >
                    {t("exportToolsCsv")}
                  </button>
                  <button
                    className="px-3 py-2 text-left hover:bg-zinc-800"
                    onClick={() => {
                      exportActionResultsAsJSON(model.actionResults);
                      setShowExportMenu(false);
                    }}
                  >
                    {t("exportActionsJson")}
                  </button>
                </div>
              )}
            </div>
          )}
        </div>
      </div>
      {model.auditRecovery?.earliestOffset && model.auditRecovery.earliestOffset > 0 ? (
        <div className="flex shrink-0 items-center justify-between gap-3 border-b border-amber-900/60 bg-amber-950/20 px-3.5 py-2 text-[11px] text-amber-100">
          <span>
            {t("auditTruncated")
              .replace("{count}", String(model.auditRecovery.earliestOffset))
              .replace("{total}", String(model.auditRecovery.totalEvents))}
          </span>
          <button
            className="control-button h-[26px] shrink-0 px-2 text-[11px]"
            disabled={loadingEarlierEvidence}
            onClick={() => void loadEarlierEvidence()}
          >
            {t("loadEarlierEvidence")}
          </button>
        </div>
      ) : null}
      <div className="min-h-0 flex-1 overflow-auto">
        {storySteps.length === 0 ? (
          <div className="p-3 text-sm text-zinc-500">
            {t("emptyActivity")}
          </div>
        ) : (
          displayed.map((step) => {
            const pending = step.toolCalls
              .map((trace) => ({ trace, requestId: pendingRequestId(trace, model.actionResults) }))
              .filter((item): item is { trace: ToolCallTrace; requestId: string } => Boolean(item.requestId));
            return (
              <article key={`step-${step.tick}`} className="border-b border-zinc-800/60 px-3.5 py-3 text-xs">
                <div className="mb-2 flex items-center gap-2">
                  <span className="font-mono font-semibold text-cyan-300">t{step.tick}</span>
                  <span className="text-[10px] text-zinc-500">{t("stepStoryLabel")}</span>
                </div>
                <div className="space-y-1.5 leading-relaxed">
                  {step.humanTurns.map((turn) => {
                    const name = model.snapshot?.humans.find((human) => human.id === turn.evidence.humanId)?.persona.name ?? turn.evidence.humanId;
                    const actions = turn.evidence.decision.actions;
                    const actionSummary = actions.map((action) => `${commandLabel(action.command, locale)} (${action.target})`).join(", ");
                    const story = actions.length > 0
                      ? t("personDecisionStory").replace("{name}", name).replace("{actions}", actionSummary)
                      : t("noActionStory").replace("{name}", name);
                    return (
                      <div key={`human-${turn.evidence.humanId}`} className="flex gap-2 text-violet-100">
                        <Bot className="mt-0.5 h-3.5 w-3.5 shrink-0 text-violet-300" />
                        <span>{story}</span>
                      </div>
                    );
                  })}
                  {step.actionResults.map((result) => (
                    <div key={result.request.requestId} className="flex gap-2 text-emerald-100">
                      <Zap className="mt-0.5 h-3.5 w-3.5 shrink-0 text-emerald-300" />
                      <span>{t("systemActionStory")
                        .replace("{status}", actionStatusLabel(result.status, locale))
                        .replace("{action}", capabilityLabel(result.request.capabilityId, locale))
                        .replace("{target}", result.request.target)}</span>
                    </div>
                  ))}
                  {step.events.map((event) => (
                    <div key={event.eventId} className="flex gap-2 text-cyan-100">
                      <Zap className="mt-0.5 h-3.5 w-3.5 shrink-0 text-cyan-300" />
                      <span>{t("eventChangeStory").replace("{change}", compactChange(event, model, locale))}</span>
                    </div>
                  ))}
                  {pending.map(({ requestId }) => <div key={requestId} className="flex items-center gap-2 bg-amber-950/30 px-2 py-1.5 text-amber-200"><span>{t("pendingApproval")}</span><button aria-label={t("approveAction")} className="control-button h-[26px] w-[26px]" title={t("approveAction")} onClick={() => void resolve(requestId, "approve")}><Check className="h-3 w-3" /></button><button aria-label={t("rejectAction")} className="control-button h-[26px] w-[26px]" title={t("rejectAction")} onClick={() => void resolve(requestId, "reject")}><X className="h-3 w-3" /></button><button aria-label={t("cancelPending")} className="control-button h-[26px] w-[26px]" title={t("cancelPending")} onClick={() => void cancelPending()}><Ban className="h-3 w-3" /></button></div>)}
                </div>
                {(step.events.length > 0 || step.toolCalls.length > 0) && <details className="mt-2 text-[10px] text-zinc-600"><summary className="cursor-pointer select-none hover:text-zinc-400">{t("technicalDetails")}</summary><div className="mt-1 space-y-0.5 font-mono">{step.events.map((event) => <div key={`detail-${event.eventId}`}>{event.eventType} · {event.source}</div>)}{step.toolCalls.map((trace) => <div key={`detail-${trace.callId}`}>{trace.toolName} · {trace.allowed ? t("allowed") : t("denied")}</div>)}</div></details>}
              </article>
            );
          })
        )}
      </div>
    </section>
  );
}
