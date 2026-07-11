import type { SimulationModel } from "../types/simulation";

export function SimulationTrace({ model }: { model: SimulationModel }) {
  return (
    <section className="min-h-[260px] border border-zinc-800 bg-zinc-900/70">
      <div className="border-b border-zinc-800 px-3 py-2 text-sm font-medium">Agent Trace</div>
      <div className="max-h-[340px] overflow-auto">
        {model.actionResults.length === 0 ? (
          <div className="p-3 text-sm text-zinc-500">No tool calls</div>
        ) : (
          model.actionResults.map((result) => (
            <div key={result.request.requestId} className="border-b border-zinc-800 px-3 py-2 text-sm">
              <div className="flex justify-between">
                <span className="text-cyan-200">{result.request.command}</span>
                <span className="text-zinc-400">t{result.tick}</span>
              </div>
              <div className="mt-1 text-zinc-300">
                {result.status}
                {result.errorCode ? ` / ${result.errorCode}` : ""}
              </div>
            </div>
          ))
        )}
      </div>
    </section>
  );
}
