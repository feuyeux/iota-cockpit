import type * as React from "react";
import { AlertCircle, Bot, CheckCircle2, CircleDot, XCircle, Zap } from "lucide-react";
import type { SimulationModel } from "../types/simulation";
import { useI18n } from "../i18n";
import { evaluationExplanation, commandLabel, eventLabel, alertLabel, actionStatusLabel } from "../utils/domainPresentation";
import { BENCHMARK_SCENARIOS, localize } from "../config/scenarioCatalog";

interface ProcessRowProps {
  icon: React.ReactNode;
  title: string;
  detail: string;
  complete: boolean;
}

function ProcessRow({ icon, title, detail, complete }: ProcessRowProps) {
  return (
    <div className={`grid grid-cols-[26px_minmax(0,1fr)] gap-2 border-l-2 pl-2.5 ${complete ? "border-emerald-400" : "border-zinc-700"}`}>
      <span className={complete ? "text-emerald-300" : "text-zinc-500"}>{icon}</span>
      <div className="min-w-0">
        <div className={complete ? "text-xs font-medium text-emerald-100" : "text-xs font-medium text-zinc-300"}>{title}</div>
        <p className="mt-0.5 text-[11px] leading-4 text-zinc-500">{detail}</p>
      </div>
    </div>
  );
}

export function SimulationEvaluation({ model }: { model: SimulationModel }) {
  const { locale, t } = useI18n();
  const evaluation = model.evaluation;
  const scenario = BENCHMARK_SCENARIOS.find((item) => item.path === model.scenario?.path);
  const text = locale === "zh-CN"
    ? {
        process: "仿真过程与评测", risk: "风险感知", decision: "模型决策", action: "系统动作", proof: "评测证据",
        waitingRisk: "等待场景触发风险", waitingDecision: "等待人物后端完成决策", waitingAction: "等待已授权动作通过系统网关",
        waitingProof: "尚未捕获能证明通过的事件", expected: "通过所需证据", deadline: "截止进度", guide: "如何观察",
        before: "先从左侧完成“选择 → 加载 → 一键运行”。", ready: "场景已就绪：推荐点击“一键运行”，再在此查看全过程。",
        running: "按顺序查看：风险出现 → 模型提出动作 → 系统执行 → 证据通过。", stopped: "本次运行已停止；重新加载后可再次运行。",
        evidence: "已捕获证据", detail: "原始证据 ID"
      }
    : {
        process: "Simulation process & evaluation", risk: "Risk sensing", decision: "Model decision", action: "System action", proof: "Evaluation evidence",
        waitingRisk: "Waiting for the scenario risk", waitingDecision: "Waiting for a human backend decision", waitingAction: "Waiting for an authorized action through the gateway",
        waitingProof: "No passing evidence has been captured", expected: "Evidence required to pass", deadline: "Deadline progress", guide: "How to observe",
        before: "Complete Select → Load → Run scenario on the left.", ready: "The scenario is ready. Use Run scenario, then follow this process.",
        running: "Follow the order: risk → model decision → system action → passing evidence.", stopped: "This run stopped. Reload the scenario to run it again.",
        evidence: "Captured evidence", detail: "Raw evidence ID"
      };
  const alerts = model.observations[0]?.alerts ?? [];
  const riskEvent = model.events.find((event) => event.eventType === "SmokeDetected" || event.eventType === "EngineFire");
  const decision = model.humanTurns.find((turn) => turn.evidence.decision.actions.length > 0);
  const action = model.actionResults[0];
  const evidenceEvents = (evaluation?.evidenceEventIds ?? []).map((id) => ({ id, event: model.events.find((item) => item.eventId === id) }));
  const riskDetail = alerts.length > 0
    ? alerts.map((alert) => alertLabel(alert, locale)).join("、")
    : riskEvent ? eventLabel(riskEvent.eventType, locale) : text.waitingRisk;
  const decisionDetail = decision
    ? decision.evidence.decision.actions.map((item) => `${commandLabel(item.command, locale)} → ${item.target}`).join("；")
    : text.waitingDecision;
  const actionDetail = action
    ? `${commandLabel(action.request.command, locale)} · ${actionStatusLabel(action.status, locale)} · ${action.request.target}`
    : text.waitingAction;
  const evidenceDetail = evidenceEvents.length > 0
    ? evidenceEvents.map(({ event, id }) => event ? `${eventLabel(event.eventType, locale)} · t${event.tick}` : id).join("；")
    : text.waitingProof;
  const guide = model.state === "ready" ? text.ready : model.state === "running" ? text.running : ["stopped", "failed"].includes(model.state) ? text.stopped : text.before;

  return (
    <section className="min-w-0 overflow-hidden border border-zinc-800 bg-zinc-900/70">
      <div className="border-b border-zinc-800 px-3 py-2 text-sm font-medium">{text.process}</div>
      <div className="space-y-3 p-3">
        {scenario ? (
          <div className="border border-cyan-800/60 bg-cyan-950/20 p-2.5 text-xs">
            <div className="font-medium text-cyan-100">{localize(scenario.title, locale)}</div>
            <div className="mt-1 text-cyan-100/70">{localize(scenario.objective, locale)}</div>
            <div className="mt-2 grid grid-cols-2 gap-2 text-[10px]">
              <div><span className="text-zinc-500">{text.expected}</span><div className="mt-0.5 text-violet-200">{eventLabel(scenario.evidenceEvent, locale)}</div></div>
              <div><span className="text-zinc-500">{text.deadline}</span><div className="mt-0.5 text-amber-200">t{model.tick} / t{scenario.deadlineTick}</div></div>
            </div>
          </div>
        ) : null}

        <div className="space-y-3">
          <ProcessRow icon={<AlertCircle className="h-4 w-4" />} title={text.risk} detail={riskDetail} complete={alerts.length > 0 || Boolean(riskEvent)} />
          <ProcessRow icon={<Bot className="h-4 w-4" />} title={text.decision} detail={decisionDetail} complete={Boolean(decision)} />
          <ProcessRow icon={<Zap className="h-4 w-4" />} title={text.action} detail={actionDetail} complete={action?.status === "applied"} />
          <ProcessRow icon={<CheckCircle2 className="h-4 w-4" />} title={text.proof} detail={evidenceDetail} complete={Boolean(evaluation?.passed)} />
        </div>

        <div className="border-t border-zinc-800 pt-3">
          {evaluation ? (
            <>
              <div className="flex items-center gap-2 text-sm">
                {evaluation.passed ? <CheckCircle2 className="h-5 w-5 text-emerald-300" /> : <XCircle className="h-5 w-5 text-rose-300" />}
                <span className={evaluation.passed ? "font-medium text-emerald-200" : "font-medium text-rose-200"}>{evaluation.passed ? t("passed") : t("failed")}</span>
                <span className="ml-auto font-mono text-xs text-zinc-400">{t("score")} {(evaluation.score * 100).toFixed(0)}%</span>
              </div>
              <div className="mt-2 h-1.5 overflow-hidden rounded bg-zinc-800"><div className={evaluation.passed ? "h-full bg-emerald-400" : "h-full bg-rose-400"} style={{ width: `${evaluation.score * 100}%` }} /></div>
              <p className="mt-2 text-xs leading-5 text-zinc-300">{evaluationExplanation(evaluation.explanation, locale)}</p>
              {evaluation.firstFailureTick != null ? <div className="mt-1 text-xs text-rose-300">{t("firstFailureTick")}: t{evaluation.firstFailureTick}</div> : null}
              {evidenceEvents.length > 0 ? <div className="mt-2 space-y-1 border-t border-zinc-800 pt-2 text-[11px]">{evidenceEvents.map(({ id, event }) => <div key={id} className="flex items-center gap-2 text-emerald-200"><CircleDot className="h-3 w-3 shrink-0" /><span>{event ? `${eventLabel(event.eventType, locale)} · t${event.tick}` : id}</span><code className="ml-auto max-w-24 truncate text-[9px] text-zinc-600" title={id}>{id}</code></div>)}</div> : null}
            </>
          ) : <div className="text-xs text-zinc-500">{t("noEvaluation")}</div>}
        </div>

        <div className="rounded border border-zinc-800 bg-zinc-950/50 p-2.5 text-[11px] leading-5 text-zinc-400"><span className="mr-1 font-medium text-zinc-200">{text.guide}：</span>{guide}</div>
      </div>
    </section>
  );
}
