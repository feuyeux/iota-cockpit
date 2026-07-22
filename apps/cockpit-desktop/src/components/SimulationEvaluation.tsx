import type * as React from "react";
import { AlertCircle, Bot, CheckCircle2, CircleDot, Clock3, XCircle, Zap } from "lucide-react";
import type { EvaluationReportRecord, SimulationModel } from "../types/simulation";
import { useI18n } from "../i18n";
import { evaluationExplanation, commandLabel, eventLabel, alertLabel, actionStatusLabel } from "../utils/domainPresentation";
import { BENCHMARK_SCENARIOS, localize } from "../config/scenarioCatalog";
import { IndependentEvaluationPanel } from "./IndependentEvaluationPanel";

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

export function SimulationEvaluation({
  model,
  completedReport,
}: {
  model: SimulationModel;
  completedReport?: EvaluationReportRecord;
}) {
  const { locale, t } = useI18n();
  const report = completedReport && completedReport.runId === model.runId
    ? completedReport.report
    : undefined;
  const progress = model.evaluationProgress?.runId === model.runId ? model.evaluationProgress : undefined;
  const scenario = BENCHMARK_SCENARIOS.find((item) => item.path === model.scenario?.path);
  const text = locale === "zh-CN"
    ? {
        process: "仿真过程与评测", risk: "风险感知", decision: "模型决策", action: "系统动作", proof: "评测证据",
        waitingRisk: "等待场景触发风险", waitingDecision: "等待人物后端完成决策", waitingAction: "等待已授权动作通过系统网关",
        waitingProof: "尚未捕获能证明通过的事件", expected: "通过所需证据", deadline: "截止进度", guide: "如何观察",
        before: "先从左侧完成“选择 → 加载 → 一键运行”。", ready: "场景已就绪：推荐点击“一键运行”，再在此查看全过程。",
        running: "按顺序查看：风险出现 → 模型提出动作 → 系统执行 → 证据通过。", stopped: "本次运行已停止；重新加载后可再次运行。",
        evidence: "已捕获证据", detail: "原始证据 ID", trajectory: "轨迹指标", actionRequests: "动作请求", rejectedActions: "拒绝动作", riskExposure: "风险暴露", firstAction: "首次动作",
        executionFailure: "执行失败", safetyFailure: "安全门槛未通过", failedRules: "未通过规则", trajectoryFailure: "轨迹门槛未通过",
        finalReport: "最终独立报告", releaseGate: "发布门禁", passedGate: "通过", blockedGate: "阻断",
        recording: "记录中", recordedTicks: "已记录", waitingFinal: "等待运行完成后生成最终报告"
      }
    : {
        process: "Simulation process & evaluation", risk: "Risk sensing", decision: "Model decision", action: "System action", proof: "Evaluation evidence",
        waitingRisk: "Waiting for the scenario risk", waitingDecision: "Waiting for a human backend decision", waitingAction: "Waiting for an authorized action through the gateway",
        waitingProof: "No passing evidence has been captured", expected: "Evidence required to pass", deadline: "Deadline progress", guide: "How to observe",
        before: "Complete Select → Load → Run scenario on the left.", ready: "The scenario is ready. Use Run scenario, then follow this process.",
        running: "Follow the order: risk → model decision → system action → passing evidence.", stopped: "This run stopped. Reload the scenario to run it again.",
        evidence: "Captured evidence", detail: "Raw evidence ID", trajectory: "Trajectory metrics", actionRequests: "Action requests", rejectedActions: "Rejected actions", riskExposure: "Risk exposure", firstAction: "First action",
        executionFailure: "Execution failure", safetyFailure: "Safety gate failed", failedRules: "Failed rules", trajectoryFailure: "Trajectory gate failed",
        finalReport: "Final independent report", releaseGate: "Release gate", passedGate: "Passed", blockedGate: "Blocked",
        recording: "Recording", recordedTicks: "Recorded", waitingFinal: "Waiting for completion before final evaluation"
      };
  const alerts = model.observations[0]?.alerts ?? [];
  const riskEvent = model.events.find((event) => event.eventType === "SmokeDetected" || event.eventType === "EngineFire");
  const decision = model.humanTurns.find((turn) => turn.evidence.decision.actions.length > 0);
  const action = model.actionResults[0];
  const evidenceEvents = (report?.evidence ?? [])
    .filter((reference) => reference.eventId)
    .map((reference) => ({ id: reference.eventId!, event: model.events.find((item) => item.eventId === reference.eventId) }));
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
  const failedRules = report?.deterministicResults.filter((rule) => rule.verdict !== "pass") ?? [];

  return (
    <section className="flex h-full min-h-0 min-w-0 flex-col overflow-hidden border-r border-zinc-800/80 bg-zinc-900/60 backdrop-blur-sm">
      <div className="shrink-0 border-b border-zinc-800/80 bg-zinc-900/80 px-3.5 py-2 text-xs font-semibold text-zinc-100">
        <span className="tracking-wide">{text.process}</span>
      </div>
      <div className="min-h-0 flex-1 space-y-3 overflow-y-auto p-3">
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
          <ProcessRow icon={<CheckCircle2 className="h-4 w-4" />} title={text.proof} detail={evidenceDetail} complete={report?.verdict === "pass"} />
        </div>

        <div className="border-t border-zinc-800 pt-3">
          {report ? (
            <>
              <div className="flex items-center gap-2 text-sm">
                {report.verdict === "pass" ? <CheckCircle2 className="h-5 w-5 text-emerald-300" /> : <XCircle className="h-5 w-5 text-rose-300" />}
                <span className={report.verdict === "pass" ? "font-medium text-emerald-200" : "font-medium text-rose-200"}>{text.finalReport} · {report.verdict}</span>
                <span className="ml-auto text-xs text-zinc-400">{text.releaseGate} <span className={report.releaseGatePassed ? "text-emerald-200" : "text-rose-200"}>{report.releaseGatePassed ? text.passedGate : text.blockedGate}</span></span>
              </div>
              <p className="mt-2 text-xs leading-5 text-zinc-300">{evaluationExplanation(report.explanation, locale)}</p>
              {failedRules.length > 0 ? <div className="mt-2 text-xs text-rose-200"><span className="font-medium">{text.failedRules}:</span> {failedRules.map((rule) => `${rule.ruleId} (t${rule.deadlineTick})`).join("；")}</div> : null}
              {evidenceEvents.length > 0 ? <div className="mt-2 space-y-1 border-t border-zinc-800 pt-2 text-[11px]">{evidenceEvents.map(({ id, event }) => <div key={id} className="flex items-center gap-2 text-emerald-200"><CircleDot className="h-3 w-3 shrink-0" /><span>{event ? `${eventLabel(event.eventType, locale)} · t${event.tick}` : id}</span><code className="ml-auto max-w-24 truncate text-[9px] text-zinc-600" title={id}>{id}</code></div>)}</div> : null}
            </>
          ) : progress ? (
            <div className="flex items-center gap-2 text-xs text-zinc-400"><Clock3 className="h-4 w-4 text-cyan-300" />{text.recording} · {text.recordedTicks} {progress.recordedTicks} tick · {progress.status === "failed" ? progress.executionError : text.waitingFinal}</div>
          ) : <div className="text-xs text-zinc-500">{t("noEvaluation")}</div>}
        </div>

        <IndependentEvaluationPanel model={model} completedReport={completedReport} />

        <div className="rounded border border-zinc-800 bg-zinc-950/50 p-2.5 text-[11px] leading-5 text-zinc-400"><span className="mr-1 font-medium text-zinc-200">{text.guide}：</span>{guide}</div>
      </div>
    </section>
  );
}
