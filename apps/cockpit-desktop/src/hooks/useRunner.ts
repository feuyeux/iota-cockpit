import { useCallback } from "react";
import { runnerClient } from "../runnerClient";
import type { SimulationAction } from "../state/simulationReducer";
import type { SimulationModel } from "../types/simulation";
import { useI18n } from "../i18n";
import { describeError } from "../utils/describeError";

export function useRunner(model: SimulationModel, dispatch: React.Dispatch<SimulationAction>) {
  const { t } = useI18n();
  const syncEvents = useCallback(async () => {
    const batch = await runnerClient.snapshot(model.lastCursor);
    if (batch.resetRequired) {
      const snapshot = await runnerClient.simulationSnapshot();
      dispatch({ type: "snapshotReset", snapshot, cursor: batch.firstAvailableCursor - 1 });
    }
    if (batch.events.length > 0) dispatch({ type: "runnerEvents", events: batch.events });
  }, [model.lastCursor, dispatch]);

  const runCommand = useCallback(
    async (command: () => Promise<unknown>): Promise<boolean> => {
      try {
        await command();
        await syncEvents();
        return true;
      } catch (error) {
        dispatch({
          type: "commandRejected",
          error: {
            code: "RUNNER_COMMAND_FAILED",
            message: describeError(error, t("commandFailed")),
            runId: model.runId,
            tick: model.tick,
            correlationId: "desktop-command",
          },
        });
        return false;
      }
    },
    [syncEvents, dispatch, model.runId, model.tick, t]
  );

  return { syncEvents, runCommand };
}
