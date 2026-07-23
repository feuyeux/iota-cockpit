import { useI18n } from "../i18n";
import type { SimulationModel } from "../types/simulation";

interface Props {
  tick: number;
  maxTicks?: number;
  state: SimulationModel["state"];
}

export function SimulationProgress({ tick, maxTicks, state }: Props) {
  const { t } = useI18n();
  const hasHorizon = Number.isFinite(maxTicks) && (maxTicks ?? 0) > 0;

  if (!hasHorizon) {
    return (
      <div className="flex min-w-0 items-center gap-2 text-xs text-zinc-500" data-testid="simulation-progress-pending">
        <span className="font-medium text-zinc-400">{t("simulationProgress")}:</span>
        <span>{t("progressPending")}</span>
      </div>
    );
  }

  const horizon = maxTicks!;
  const completedTicks = Math.min(Math.max(tick, 0), horizon);
  const remainingTicks = Math.max(horizon - completedTicks, 0);
  const percent = Math.round((completedTicks / horizon) * 100);
  const horizonReached = remainingTicks === 0 || state === "completed";
  const status = horizonReached
    ? t("runHorizonReached")
    : `${t("remainingSteps")} ${remainingTicks} ${t("ticksUnit")}`;

  return (
    <section className="flex w-full min-w-0 items-center gap-3 text-xs" aria-label={t("simulationProgress")} data-testid="simulation-progress">
      <span className="shrink-0 font-medium text-zinc-400">{t("simulationProgress")}</span>
      <div className="flex flex-1 items-center min-w-[80px]">
        <div
          aria-label={t("simulationProgress")}
          aria-valuemax={horizon}
          aria-valuemin={0}
          aria-valuenow={completedTicks}
          aria-valuetext={`${percent}% · ${status}`}
          className="h-2 w-full overflow-hidden rounded-full bg-zinc-900 border border-zinc-800"
          role="progressbar"
        >
          <div
            className={horizonReached ? "h-full bg-emerald-400 transition-[width] duration-300" : "h-full bg-cyan-400 transition-[width] duration-300"}
            style={{ width: `${percent}%` }}
          />
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-1.5 font-mono text-zinc-300">
        <span className="text-cyan-300 font-semibold">t{completedTicks} / t{horizon}</span>
        <span className={horizonReached ? "text-emerald-400 font-medium" : "text-zinc-400"}>
          ({status})
        </span>
      </div>
    </section>
  );
}
