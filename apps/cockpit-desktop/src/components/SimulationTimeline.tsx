import type { SimulationModel } from "../types/simulation";

export function SimulationTimeline({ model }: { model: SimulationModel }) {
  return (
    <section className="min-h-[260px] border border-zinc-800 bg-zinc-900/70">
      <div className="border-b border-zinc-800 px-3 py-2 text-sm font-medium">Timeline</div>
      <div className="max-h-[340px] overflow-auto">
        {model.events.length === 0 ? (
          <div className="p-3 text-sm text-zinc-500">No events</div>
        ) : (
          model.events.map((event) => (
            <div key={event.eventId} className="grid grid-cols-[70px_160px_1fr] gap-3 border-b border-zinc-800 px-3 py-2 text-sm">
              <span className="text-zinc-400">t{event.tick}</span>
              <span className="text-cyan-200">{event.eventType}</span>
              <span className="text-zinc-300">{event.payload.message}</span>
            </div>
          ))
        )}
      </div>
    </section>
  );
}
