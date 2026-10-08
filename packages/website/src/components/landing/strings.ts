// Landing copy. EN is the approved comp's wording plus existing docs and README
// content; KO reuses the existing KO docs where a counterpart exists and is
// otherwise a faithful translation of the EN text.

export type Lang = "en" | "ko";

type Station = { name: string; lines: string[]; href: string };
type Source = { label: string; href: string };
type Row = { tool: string; owns: string; checks: string; note?: string; sources: Source[] };

const en = {
  htmlLang: "en",
  base: "/",
  docsBase: "/",
  title: "canon",
  description: "A verification gate for agent-written work.",
  nav: {
    label: "Primary",
    docs: { text: "Docs", href: "/getting-started/" },
    cli: { text: "CLI", href: "/cli/" },
    concepts: { text: "Concepts", href: "/concepts/canon/" },
    github: { text: "GitHub", href: "https://github.com/journeyWorker/canon" },
  },
  headline: ["Agents say done.", "canon makes them", "prove it."],
  subline: "A verification gate for agent-written work.",
  command: "bunx @journeykit/canon init",
  copy: "Copy",
  copied: "Copied",
  copyLabel: "Copy the install command",
  guide: { text: "Read the getting started guide", href: "/getting-started/" },
  fig1: {
    number: "Fig. 1",
    title: "canon's own corpus",
    scenarios: (n: number) => `${n} scenarios`,
    violations: (n: number) => (n === 1 ? "1 violation" : `${n} violations`),
    unknown: "gate result unavailable",
    data: {
      caption: (n: number, lagged: number) =>
        `Fig. 1 data: ${n} scenarios, each with the day it was authored and the day of its first evidence; ${lagged} got evidence after authoring.`,
      id: "Scenario",
      authored: "Authored",
      evidence: "First evidence",
      lagged: "Evidence after authoring",
      none: "none",
      yes: "yes",
      no: "no",
    },
  },
  stations: ["intent", "run", "evidence", "gate", "decision", "learning"],

  loop: {
    heading: "Every run feeds the next",
    number: "Fig. 2",
    title: "intent → run → evidence → gate → decision → learning",
    stations: [
      {
        name: "intent",
        lines: [
          "An agent's first act on substantial work is authoring or updating the Subject.",
        ],
        href: "/concepts/canon/",
      },
      {
        name: "run",
        lines: [
          "Session adapters write one root run per session plus one child run per subagent.",
        ],
        href: "/architecture/",
      },
      {
        name: "evidence",
        lines: [
          "Coverage — does the required evidence exist for this artifact, per policy?",
          "Verdict ledger — did that evidence pass, by whom, and how stale is it now?",
        ],
        href: "/concepts/trust-spine/",
      },
      {
        name: "gate",
        lines: [
          "Fails closed — the row stays unflipped — on missing, malformed, or fabricated evidence.",
          "Exit 0 clean, 1 gate-red, 2 usage/load failure.",
        ],
        href: "/cli/",
      },
      {
        name: "decision",
        lines: [
          "Risk tiers require current effect/path binding and policy-pinned SSH approval.",
        ],
        href: "/concepts/strategy-memory/",
      },
      {
        name: "learning",
        lines: [
          "Newly distilled strategies are quarantined by default.",
          "canon retrieve surfaces them before the next run starts.",
        ],
        href: "/concepts/strategy-memory/",
      },
    ] as Station[],
  },

  compare: {
    heading: "What each tool owns",
    lead: "Canon records and gates evidence; it is not a general agent runtime and does not schedule or execute declared plans.",
    number: "Fig. 3",
    title: "What each owns · what canon owns",
    cols: { tool: "Tool", owns: "Owns", checks: "Its own checks", sources: "Sources" },
    rows: [
      {
        tool: "GitHub Spec Kit",
        owns: "A spec-driven process run by your coding agent: specify → plan → tasks → implement → converge.",
        checks: "Agent-run quality gates; converge assesses the code against the spec, plan, and tasks.",
        sources: [
          { label: "README", href: "https://github.com/github/spec-kit" },
          { label: "Agentic SDD", href: "https://github.github.io/spec-kit/reference/agentic-sdd.html" },
        ],
      },
      {
        tool: "AWS AI-DLC",
        owns: "An adaptive lifecycle workflow: AI plans and asks, humans approve before each stage moves on.",
        checks: "Human approval gates, source-bound review evidence, an audit trail, and learned rules.",
        sources: [
          { label: "README", href: "https://github.com/awslabs/aidlc-workflows" },
          { label: "AWS blog", href: "https://aws.amazon.com/blogs/devops/ai-driven-development-life-cycle/" },
        ],
      },
      {
        tool: "OpenHands",
        owns: "Runs coding agents and automations, locally or in Docker or Kubernetes workspaces.",
        checks: "An experimental critic scores agent work; confirmation mode holds risky actions for approval.",
        sources: [
          { label: "README", href: "https://github.com/OpenHands/OpenHands" },
          { label: "Agent SDK", href: "https://github.com/OpenHands/software-agent-sdk" },
          { label: "Critic", href: "https://docs.openhands.dev/sdk/guides/critic" },
          { label: "Confirmation", href: "https://docs.openhands.dev/sdk/arch/agent" },
        ],
      },
      {
        tool: "SWE-agent",
        owns: "Takes a GitHub issue and tries to fix it, running commands in a Docker container, local or remote.",
        checks: "Retries across agent configurations, then chooses the best attempt.",
        note: "Its README now recommends mini-SWE-agent going forward.",
        sources: [
          { label: "README", href: "https://github.com/SWE-agent/SWE-agent" },
          { label: "Architecture", href: "https://swe-agent.com/latest/background/architecture/" },
          { label: "Changelog", href: "https://swe-agent.com/latest/installation/changelog/" },
        ],
      },
      {
        tool: "canon",
        owns: "Records and verifies intent, evidence, policy, approvals, and run lineage; it does not execute agent actions.",
        checks: "Completion is gated on evidence that exists, is attributed, and is fresh. A policy can also require an independent review per scenario (spec_coverage.require_review, 0.12.0).",
        note: "Experimental (0.11.0, off by default): evidence binding to artifacts and JUnit/Cucumber reports.",
        sources: [
          { label: "Trust spine", href: "/concepts/trust-spine/" },
          { label: "Architecture", href: "/architecture/" },
        ],
      },
    ] as Row[],
    footnote: "Claims about other tools were checked against each project's own README or docs on 2026-10-09.",
  },

  quick: {
    heading: "See it in 60 seconds",
    demo: [
      "mkdir canon-demo && cd canon-demo",
      "bunx @journeykit/canon demo init      # scaffolds a throwaway evidence loop",
      "bunx @journeykit/canon gate check     # RED",
      "# canon gate check: 1 violation(s)",
      "#",
      "# uncovered-cell (1):",
      "#   uncovered-cell auth.login.01 — policy-required cell 'reviewer' has no matching-role evidence record",
      "bunx @journeykit/canon demo attest    # records the reviewer's evidence",
      "bunx @journeykit/canon gate check     # GREEN",
      "# canon gate check: clean (0 violations)",
    ],
    after:
      "Watch the gate flip from red to green the instant reviewer evidence lands — no scenarios, review records, or regime keys required to see it happen.",
    anyRepo: "In any repo",
    repo: [
      "# one Rust core, distributed through npm",
      "bunx @journeykit/canon report          # where does this repo stand?",
      "bunx @journeykit/canon gate check      # is \"done\" actually done?",
      "bunx @journeykit/canon ingest sessions # fold agent transcripts into the ledger",
    ],
  },

  limits: {
    heading: "What canon proves — and what it doesn't",
    verifiesLead: "Canon strongly verifies:",
    verifies: [
      "that policy-required evidence exists",
      "that evidence is recorded as pass or fail",
      "who recorded it",
      "that the target code hasn't changed out from under the evidence (freshness)",
      "that a completion mark passed a defined procedure",
    ],
    notLead: "Canon does not prove:",
    not: [
      "whether a test actually verifies the requirement",
      "whether a reviewer judged correctly",
      "that a malicious user fabricated plausible-looking evidence",
      "mathematical or semantic correctness of the program",
    ],
    statement:
      "Canon verifies the structure, provenance, verdict, and freshness of evidence. It does not decide whether a test or reviewer is semantically correct.",
  },

  docs: {
    heading: "Docs",
  },

  footer: {
    status:
      "Public pre-alpha. Core workflows are implemented and dogfooded; interfaces and storage formats may change.",
    language: "Language",
  },
};

export type Strings = typeof en;

const ko: Strings = {
  htmlLang: "ko",
  base: "/ko/",
  docsBase: "/ko/",
  title: "canon",
  description: "에이전트가 작성한 작업을 위한 검증 게이트.",
  nav: {
    label: "주 메뉴",
    docs: { text: "문서", href: "/ko/getting-started/" },
    cli: { text: "CLI", href: "/ko/cli/" },
    concepts: { text: "개념", href: "/ko/concepts/canon/" },
    github: { text: "GitHub", href: "https://github.com/journeyWorker/canon" },
  },
  headline: ["에이전트는", "완료라고 말합니다.", "canon은 그것을", "증명하게 만듭니다."],
  subline: "에이전트가 작성한 작업을 위한 검증 게이트.",
  command: "bunx @journeykit/canon init",
  copy: "복사",
  copied: "복사됨",
  copyLabel: "설치 명령 복사",
  guide: { text: "시작하기 가이드 읽기", href: "/ko/getting-started/" },
  fig1: {
    number: "그림 1",
    title: "canon 자체 코퍼스",
    scenarios: (n: number) => `시나리오 ${n}개`,
    violations: (n: number) => `위반 ${n}건`,
    unknown: "gate 결과 없음",
    data: {
      caption: (n: number, lagged: number) =>
        `그림 1 데이터: 시나리오 ${n}개의 작성일과 첫 증거일. 이 중 ${lagged}개는 작성 후에 증거가 기록되었습니다.`,
      id: "시나리오",
      authored: "작성일",
      evidence: "첫 증거일",
      lagged: "작성 후 증거",
      none: "없음",
      yes: "예",
      no: "아니요",
    },
  },
  stations: ["의도", "실행", "증거", "게이트", "결정", "학습"],

  loop: {
    heading: "모든 실행이 다음 실행을 먹입니다",
    number: "그림 2",
    title: "의도 → 실행 → 증거 → 게이트 → 결정 → 학습",
    stations: [
      {
        name: "의도",
        lines: ["본격적인 작업에서 에이전트의 첫 행동은 서브젝트를 저작하거나 갱신하는 것입니다."],
        href: "/ko/concepts/canon/",
      },
      {
        name: "실행",
        lines: ["세션 어댑터는 세션마다 루트 run 하나와 서브에이전트마다 자식 run 하나를 씁니다."],
        href: "/ko/architecture/",
      },
      {
        name: "증거",
        lines: [
          "커버리지 — 정책상 요구되는 증거가 이 아티팩트에 대해 존재하는가?",
          "판정 레저 — 그 증거가 통과했는가, 누가 했는가, 얼마나 오래됐는가?",
        ],
        href: "/ko/concepts/trust-spine/",
      },
      {
        name: "게이트",
        lines: [
          "증거가 없거나 형식이 잘못되었거나 조작된 경우 fail closed — 행은 플립되지 않은 채로 남습니다.",
          "종료 코드: 0 클린, 1 gate-red, 2 사용/로드 실패.",
        ],
        href: "/ko/cli/",
      },
      {
        name: "결정",
        lines: ["위험 등급은 현재의 effect/path 바인딩과 policy에 pin된 SSH 승인을 요구합니다."],
        href: "/ko/concepts/strategy-memory/",
      },
      {
        name: "학습",
        lines: [
          "새로 증류된 전략은 기본적으로 quarantine됩니다.",
          "canon retrieve가 다음 실행 전에 꺼내줍니다.",
        ],
        href: "/ko/concepts/strategy-memory/",
      },
    ],
  },

  compare: {
    heading: "각 도구가 담당하는 것",
    lead: "Canon은 증거를 기록하고 게이트할 뿐, 일반 에이전트 런타임이 아니며 선언된 플랜을 스케줄링하거나 실행하지 않습니다.",
    number: "그림 3",
    title: "각 도구가 담당하는 것 · canon이 담당하는 것",
    cols: { tool: "도구", owns: "담당 영역", checks: "자체 검사", sources: "출처" },
    rows: [
      {
        tool: "GitHub Spec Kit",
        owns: "코딩 에이전트가 수행하는 스펙 주도 프로세스: specify → plan → tasks → implement → converge.",
        checks: "에이전트가 수행하는 품질 게이트. converge는 스펙, 플랜, 태스크에 비추어 코드를 평가합니다.",
        sources: [
          { label: "README", href: "https://github.com/github/spec-kit" },
          { label: "Agentic SDD", href: "https://github.github.io/spec-kit/reference/agentic-sdd.html" },
        ],
      },
      {
        tool: "AWS AI-DLC",
        owns: "적응형 라이프사이클 워크플로: AI가 계획하고 질문하며, 각 단계가 넘어가기 전에 사람이 승인합니다.",
        checks: "사람의 승인 게이트, 소스에 묶인 리뷰 증거, 감사 추적, 학습된 규칙.",
        sources: [
          { label: "README", href: "https://github.com/awslabs/aidlc-workflows" },
          { label: "AWS blog", href: "https://aws.amazon.com/blogs/devops/ai-driven-development-life-cycle/" },
        ],
      },
      {
        tool: "OpenHands",
        owns: "코딩 에이전트와 자동화를 로컬 또는 Docker·Kubernetes 워크스페이스에서 실행합니다.",
        checks: "실험적 critic이 에이전트 작업에 점수를 매기고, confirmation 모드는 위험한 행동을 승인 전까지 보류합니다.",
        sources: [
          { label: "README", href: "https://github.com/OpenHands/OpenHands" },
          { label: "Agent SDK", href: "https://github.com/OpenHands/software-agent-sdk" },
          { label: "Critic", href: "https://docs.openhands.dev/sdk/guides/critic" },
          { label: "Confirmation", href: "https://docs.openhands.dev/sdk/arch/agent" },
        ],
      },
      {
        tool: "SWE-agent",
        owns: "GitHub 이슈를 받아 수정을 시도하며, 명령은 로컬 또는 원격 Docker 컨테이너에서 실행합니다.",
        checks: "여러 에이전트 구성으로 재시도한 뒤 가장 좋은 시도를 고릅니다.",
        note: "README는 앞으로 mini-SWE-agent 사용을 권장합니다.",
        sources: [
          { label: "README", href: "https://github.com/SWE-agent/SWE-agent" },
          { label: "Architecture", href: "https://swe-agent.com/latest/background/architecture/" },
          { label: "Changelog", href: "https://swe-agent.com/latest/installation/changelog/" },
        ],
      },
      {
        tool: "canon",
        owns: "intent, 증거, policy, 승인, run lineage를 기록하고 검증합니다. 에이전트의 행동을 실행하지는 않습니다.",
        checks: "완료는 존재하고 귀속되며 최신인(fresh) evidence로 게이트됩니다. 정책으로 시나리오마다 독립 리뷰를 요구할 수도 있습니다(spec_coverage.require_review, 0.12.0).",
        note: "실험 기능(0.11.0, 기본값 off): 아티팩트와 JUnit/Cucumber 리포트에 대한 증거 바인딩.",
        sources: [
          { label: "트러스트 스파인", href: "/ko/concepts/trust-spine/" },
          { label: "아키텍처", href: "/ko/architecture/" },
        ],
      },
    ],
    footnote: "다른 도구에 대한 내용은 2026-10-09에 각 프로젝트의 README 또는 문서와 대조해 확인했습니다.",
  },

  quick: {
    heading: "60초 만에 확인하기",
    demo: en.quick.demo,
    after:
      "리뷰어 증거가 도착하는 순간 게이트가 red에서 green으로 바뀌는 것을 확인하세요 — 시나리오, 리뷰 레코드, regime key 없이도 이 흐름을 볼 수 있습니다.",
    anyRepo: "어느 레포에서든",
    repo: [
      "# 하나의 Rust 코어, npm으로 배포",
      "bunx @journeykit/canon report          # 이 레포는 지금 어디까지 왔나?",
      "bunx @journeykit/canon gate check      # \"done\"이 진짜 done인가?",
      "bunx @journeykit/canon ingest sessions # 에이전트 트랜스크립트를 레저로",
    ],
  },

  limits: {
    heading: "canon이 증명하는 것 — 그리고 증명하지 않는 것",
    verifiesLead: "canon이 강하게 검증하는 것:",
    verifies: [
      "정책상 요구되는 증거가 존재하는지",
      "증거가 통과 또는 실패로 기록되었는지",
      "누가 그것을 기록했는지",
      "대상 코드가 증거 아래에서 바뀌지 않았는지(freshness)",
      "완료 표시가 정의된 절차를 통과했는지",
    ],
    notLead: "canon이 증명하지 않는 것:",
    not: [
      "테스트가 요구사항을 실제로 검증하는지",
      "리뷰어가 올바르게 판단했는지",
      "악의적인 사용자가 그럴듯해 보이는 증거를 조작하지 않았는지",
      "프로그램의 수학적·의미적 정확성",
    ],
    statement:
      "canon은 증거의 구조, provenance, 판정, freshness를 검증합니다. 테스트나 리뷰어가 의미적으로 옳은지는 판단하지 않습니다.",
  },

  docs: {
    heading: "문서",
  },

  footer: {
    status:
      "공개 프리알파. 핵심 워크플로는 구현되어 도그푸딩 중이며, 인터페이스와 저장 형식은 바뀔 수 있습니다.",
    language: "언어",
  },
};

export const strings = { en, ko };
