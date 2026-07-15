import { useI18n } from "../i18n";
import type { SimulationModel } from "../types/simulation";

interface Props {
  tick: number;
  deadlineTick?: number;
  state: SimulationModel["state"];
}

export function SimulationProgress({ tick, deadlineTick, state }: Props) {
  const { t } = useI18n();
  const hasDeadline = Number.isFinite(deadlineTick) && (deadlineTick ?? 0) > 0;

  if (!hasDeadline) {
    return (
      <div className="flex min-w-0 flex-col items-center text-sm text-zinc-500" data-testid="simulation-progress-pending">
        <span className="font-medium text-zinc-300">{t("simulationProgress")}</span>
        <span>{t("progressPending")}</span>
      </div>
    );
  }

  const deadline = deadlineTick!;
  const completedTicks = Math.min(Math.max(tick, 0), deadline);
  const remainingTicks = Math.max(deadline - completedTicks, 0);
  const percent = Math.round((completedTicks / deadline) * 100);
  const deadlineReached = remainingTicks === 0 || state === "completed";
  const status = deadlineReached
    ? t("deadlineReached")
    : `${t("remainingSteps")} ${remainingTicks} ${t("ticksUnit")}`;

  return (
    <section className="w-full min-w-0" aria-label={t("simulationProgress")} data-testid="simulation-progress">
      <div className="mb-1 flex items-center justify-between gap-3 text-sm">
        <span className="font-medium text-zinc-100">{t("simulationProgress")}</span>
        <span className="font-mono text-cyan-200">t{completedTicks} / t{deadline}</span>
      </div>
      <div
        aria-label={t("simulationProgress")}
        aria-valuemax={deadline}
        aria-valuemin={0}
        aria-valuenow={completedTicks}
        aria-valuetext={`${percent}% · ${status}`}
        className="h-2.5 overflow-hidden rounded-full bg-zinc-800"
        role="progressbar"
      >
        <div
          className={deadlineReached ? "h-full bg-emerald-400 transition-[width] duration-300" : "h-full bg-cyan-400 transition-[width] duration-300"}
          style={{ width: `${percent}%` }}
        />
      </div>
      <div className={deadlineReached ? "mt-1 text-right text-sm font-medium text-emerald-300" : "mt-1 text-right text-sm text-zinc-300"}>
        {status}
      </div>
    </section>
  );
}
