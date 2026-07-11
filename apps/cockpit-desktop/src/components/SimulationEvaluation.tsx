import { CheckCircle2, XCircle } from "lucide-react";
import type { SimulationModel } from "../types/simulation";

export function SimulationEvaluation({ model }: { model: SimulationModel }) {
  const evaluation = model.evaluation;

  return (
    <section className="border border-zinc-800 bg-zinc-900/70">
      <div className="border-b border-zinc-800 px-3 py-2 text-sm font-medium">Evaluation</div>
      <div className="space-y-3 p-3 text-sm">
        {evaluation ? (
          <>
            <div className="flex items-center gap-2">
              {evaluation.passed ? (
                <CheckCircle2 className="h-5 w-5 text-emerald-300" />
              ) : (
                <XCircle className="h-5 w-5 text-red-300" />
              )}
              <span>{evaluation.passed ? "passed" : "failed"}</span>
            </div>
            <div className="h-2 overflow-hidden bg-zinc-800">
              <div className="h-full bg-emerald-400" style={{ width: `${evaluation.score * 100}%` }} />
            </div>
            <p className="text-zinc-300">{evaluation.explanation}</p>
          </>
        ) : (
          <div className="text-zinc-500">No evaluation</div>
        )}
      </div>
    </section>
  );
}
