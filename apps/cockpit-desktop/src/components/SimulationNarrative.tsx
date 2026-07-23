import { MessageSquare } from "lucide-react";
import type { SimulationModel } from "../types/simulation";
import { useI18n } from "../i18n";
import { eventLabel, isScenarioInteractionEvent } from "../utils/domainPresentation";

/// Shows the public causal situation injected by a scenario. Sensor noise
/// remains available in technical traces, but does not masquerade as dialogue.
export function SimulationNarrative({ model }: { model: SimulationModel }) {
  const { locale, t } = useI18n();
  const interactions = model.events
    .filter((event) => isScenarioInteractionEvent(event.eventType))
    .sort((left, right) => right.tick - left.tick || right.sequence - left.sequence);

  return (
    <section className="flex h-full min-h-0 flex-1 flex-col overflow-hidden bg-zinc-900/60 backdrop-blur-sm">
      <div className="flex shrink-0 items-center justify-between border-b border-zinc-800/80 bg-zinc-900/80 px-3.5 py-2 text-xs font-semibold text-zinc-100">
        <span className="tracking-wide">{t("scenarioInteraction")}</span>
        <span className="text-[11px] font-normal text-zinc-400">{t("newestFirst")}</span>
      </div>
      {interactions.length === 0 ? (
        <div className="p-4 text-xs text-zinc-500 leading-relaxed">{t("noScenarioInteraction")}</div>
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto">
          {interactions.map((event) => (
            <div key={event.eventId} className="flex items-start gap-2.5 border-b border-zinc-800/60 px-3.5 py-3 text-xs">
              <MessageSquare className="mt-0.5 h-4 w-4 shrink-0 text-sky-300" />
              <div className="min-w-0">
                <div className="text-[10px] text-zinc-500">t{event.tick} · {eventLabel(event.eventType, locale)} · {event.source}{event.payload.target ? ` → ${event.payload.target}` : ""}</div>
                <div className="mt-1 leading-relaxed text-sky-100">{event.payload.message}</div>
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
