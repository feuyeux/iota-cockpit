import { useEffect, useReducer, useState } from "react";
import { Activity, AlertTriangle, Bot, Gauge, Link, Link2Off, HelpCircle } from "lucide-react";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { KeyboardShortcutsHelp } from "./components/KeyboardShortcutsHelp";
import { SimulationEvaluation } from "./components/SimulationEvaluation";
import { SimulationSourcePanel } from "./components/SimulationSourcePanel";
import { SimulationActivityFeed } from "./components/SimulationActivityFeed";
import { SimulationWorldView } from "./components/SimulationWorldView";
import { SimulationNarrative } from "./components/SimulationNarrative";
import { KEYBOARD_SHORTCUTS } from "./config/constants";
import { runnerClient } from "./runnerClient";
import { initialSimulationModel, simulationReducer } from "./state/simulationReducer";
import { exponentialBackoff } from "./utils/reconnect";
import { loadPersistedSession } from "./utils/storage";
import { useI18n, type MessageKey } from "./i18n";
import type { SimulationModel } from "./types/simulation";

const stateLabels: Partial<Record<SimulationModel["state"], MessageKey>> = {
  connectedIdle: "connectedIdle",
  disconnected: "disconnected",
  running: "running",
  paused: "paused",
  ready: "ready",
  completed: "completed",
  stopped: "stopped",
  failed: "failedState"
};

export function App() {
  const { locale, setLocale, t } = useI18n();
  const persisted = loadPersistedSession();
  const [model, dispatch] = useReducer(
    simulationReducer,
    persisted
      ? { ...initialSimulationModel, approvalRequired: persisted.approvalRequired }
      : initialSimulationModel
  );
  const [showHelp, setShowHelp] = useState(false);
  const stateLabel = stateLabels[model.state];

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.target instanceof HTMLElement && ["INPUT", "TEXTAREA", "SELECT"].includes(event.target.tagName)) {
        return;
      }
      if (event.key === KEYBOARD_SHORTCUTS.HELP) {
        event.preventDefault();
        setShowHelp(true);
      } else if (event.key === "Escape") {
        setShowHelp(false);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  useEffect(() => {
    let cancelled = false;
    dispatch({ type: "connectRequested" });
    runnerClient
      .connect()
      .then(() => {
        if (!cancelled) dispatch({ type: "connected" });
      })
      .catch((error: Error) => {
        if (!cancelled) {
          dispatch({
            type: "disconnected",
            error: {
              code: "RUNNER_CONNECT_FAILED",
              message: error.message,
              correlationId: "desktop-connect"
            }
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function reconnect() {
    dispatch({ type: "connectRequested" });
    const result = await exponentialBackoff(async () => {
      await runnerClient.connect();
      const batch = await runnerClient.snapshot(model.lastCursor);
      if (batch.resetRequired) {
        const snapshot = await runnerClient.simulationSnapshot();
        dispatch({ type: "snapshotReset", snapshot, cursor: batch.firstAvailableCursor - 1 });
      }
      if (batch.events.length > 0) dispatch({ type: "runnerEvents", events: batch.events });
    });

    if (result.success) {
      dispatch({ type: "connected" });
    } else {
      dispatch({
        type: "disconnected",
        error: {
          code: "RUNNER_CONNECT_FAILED",
          message: result.error?.message ?? `${t("reconnectFailed")}: ${result.attempts}`,
          correlationId: "desktop-reconnect"
        }
      });
    }
  }

  return (
    <main className="flex h-dvh flex-col overflow-hidden bg-zinc-950 text-zinc-100">
      <header className="flex shrink-0 flex-col gap-2 border-b border-zinc-800 px-3 py-2 sm:flex-row sm:items-center sm:justify-between sm:px-4">
        <div className="flex min-w-0 items-center gap-2 sm:gap-3">
          <Activity className="h-5 w-5 shrink-0 text-cyan-300" />
          <h1 className="min-w-0 truncate text-base font-semibold">{t("appName")}</h1>
          <span className="max-w-32 shrink-0 truncate rounded border border-zinc-700 px-2 py-1 text-xs text-zinc-300" title={model.scenario?.id ?? t("noScenario")}>
            {model.scenario?.id ?? t("noScenario")}
          </span>
        </div>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2 text-xs text-zinc-300 sm:justify-end sm:text-sm">
          <span className="flex items-center gap-2">
            {model.serviceConnected ? (
              <Link className="h-4 w-4 text-emerald-300" />
            ) : (
              <Link2Off className="h-4 w-4 text-amber-300" />
            )}
            {stateLabel ? t(stateLabel) : model.state}
          </span>
          {!model.serviceConnected ? (
            <button
              aria-label={t("reconnect")}
              className="border border-zinc-700 px-2 py-1 text-xs hover:bg-zinc-800"
              onClick={() => void reconnect()}
            >
              {t("reconnect")}
            </button>
          ) : null}
          <span className="flex items-center gap-2">
            <Gauge className="h-4 w-4 text-cyan-300" />
            {t("tick")} {model.tick} / {model.simTimeMs}ms
          </span>
          <span className="flex max-w-48 items-center gap-1.5 truncate text-xs" title={model.backend}>
            <Bot className="h-4 w-4 text-violet-300" />
            {t("modelDrive")}
            {model.backend ? ` · ${model.backend}` : ""}
          </span>
          <div className="flex border border-zinc-700" aria-label={t("language")}>
            <button
              className={`h-7 px-2 text-xs ${locale === "zh-CN" ? "bg-cyan-900 text-cyan-100" : "text-zinc-400"}`}
              onClick={() => setLocale("zh-CN")}
            >
              中文
            </button>
            <button
              className={`h-7 px-2 text-xs ${locale === "en-US" ? "bg-cyan-900 text-cyan-100" : "text-zinc-400"}`}
              onClick={() => setLocale("en-US")}
            >
              EN
            </button>
          </div>
          <button
            aria-label={t("keyboardShortcuts")}
            className="control-button h-7 w-7"
            onClick={() => setShowHelp(true)}
          >
            <HelpCircle className="h-4 w-4" />
          </button>
        </div>
      </header>

      {model.error ? (
        <section className="mx-3 mt-2 flex shrink-0 items-start gap-3 border border-red-500/40 bg-red-950/30 p-2.5 text-sm sm:mx-4">
          <AlertTriangle className="h-5 w-5 shrink-0 text-red-300" />
          <div className="min-w-0">
            <div className="font-medium">{model.error.code}</div>
            <div className="truncate text-red-100" title={model.error.message}>{model.error.message}</div>
          </div>
        </section>
      ) : null}

      <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden p-3 sm:gap-4 sm:p-4">
        <div className="grid min-h-0 flex-1 grid-rows-3 gap-3 overflow-hidden sm:gap-4 lg:grid-cols-[260px_minmax(0,1fr)_300px] lg:grid-rows-1">
          <ErrorBoundary>
            <SimulationSourcePanel model={model} dispatch={dispatch} />
          </ErrorBoundary>
          <ErrorBoundary>
            <SimulationWorldView model={model} />
          </ErrorBoundary>
          <div className="flex min-h-0 min-w-0 flex-col gap-3 overflow-hidden sm:gap-4">
            <ErrorBoundary>
              <SimulationEvaluation model={model} />
            </ErrorBoundary>
            <ErrorBoundary>
              <SimulationNarrative model={model} />
            </ErrorBoundary>
          </div>
        </div>
        <div className="h-48 shrink-0 sm:h-56">
          <ErrorBoundary>
            <SimulationActivityFeed model={model} dispatch={dispatch} />
          </ErrorBoundary>
        </div>
      </div>
      <KeyboardShortcutsHelp visible={showHelp} onClose={() => setShowHelp(false)} />
    </main>
  );
}
