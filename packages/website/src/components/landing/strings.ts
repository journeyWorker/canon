// Landing copy. EN is the approved comp's wording plus existing docs and README
// content; KO is a faithful translation of the EN text.

export type Lang = "en" | "ko";

type Station = { name: string; lines: string[]; href: string };

// The hero's pull request, in the approved comp's words. The log is the comp's
// line verbatim (user decision), not translated in KO.
type CheckState = "pass" | "fail" | "wait";
type Check = { name: string; state: CheckState; summary: string; log?: string[] };

// Comparison matrix. Every cell links the primary source it was checked
// against; "nd" means the tool's own docs say nothing either way.
export type CellState = "yes" | "partial" | "no" | "nd";
type Cell = { state: CellState; source: string; href: string; note: string };
type Row = { tool: string; href: string; cells: Cell[] };
type MatrixCell = { state: CellState; source: string; href: string; note: { en: string; ko: string } };
type MatrixRow = { tool: string; href: string; cells: MatrixCell[] };

const GATE_LOG = ["uncovered-cell cart.add.04 — spec scenario has no evidence record"];

const SPECKIT = "https://github.com/github/spec-kit";
const SPECKIT_DOCS = "https://github.github.io/spec-kit";
const AIDLC = "https://github.com/awslabs/aidlc-workflows";
const AIDLC_DOCS = `${AIDLC}/blob/main/docs`;
const OPENHANDS = "https://github.com/OpenHands/OpenHands";
const OH_DOCS = "https://docs.openhands.dev";
const SWE = "https://github.com/SWE-agent/SWE-agent";
const SWE_DOCS = "https://swe-agent.com/latest";
const CANON = "https://github.com/journeyWorker/canon";

// canon's own cells link its docs; a leading "/" is resolved per locale.
const matrix: MatrixRow[] = [
  {
    tool: "GitHub Spec Kit",
    href: SPECKIT,
    cells: [
      {
        state: "yes",
        source: "README",
        href: SPECKIT,
        note: {
          en: "specify → plan → tasks → implement → converge, per feature",
          ko: "기능마다 specify → plan → tasks → implement → converge",
        },
      },
      {
        state: "no",
        source: "README",
        href: SPECKIT,
        note: {
          en: "Commands and templates that run inside your coding agent",
          ko: "코딩 에이전트 안에서 실행되는 명령과 템플릿",
        },
      },
      {
        state: "no",
        source: "Workflows reference",
        href: `${SPECKIT_DOCS}/reference/workflows.html`,
        note: {
          en: "Workflow shell steps run with your privileges; no sandbox",
          ko: "워크플로 shell 단계는 사용자 권한으로 실행되며 샌드박스가 없음",
        },
      },
      {
        state: "partial",
        source: "Bug fixing guide",
        href: `${SPECKIT_DOCS}/guides/bugfix.html`,
        note: {
          en: "The bug-fix extension records pass/fail per check, not per requirement",
          ko: "버그 수정 확장이 요구사항이 아닌 검사 단위로 pass/fail을 기록",
        },
      },
      {
        state: "partial",
        source: "README",
        href: SPECKIT,
        note: {
          en: "The agent runs converge; nothing enforces it mechanically",
          ko: "에이전트가 converge를 실행할 뿐 기계적으로 강제하지 않음",
        },
      },
      {
        state: "yes",
        source: "Workflows reference",
        href: `${SPECKIT_DOCS}/reference/workflows.html`,
        note: {
          en: "Workflow gate steps pause for human approval",
          ko: "워크플로 gate 단계가 사람의 승인을 기다림",
        },
      },
      {
        state: "partial",
        source: "Agentic SDD reference",
        href: `${SPECKIT_DOCS}/reference/agentic-sdd.html`,
        note: {
          en: "Reviewer-owned checklists; no built-in reviewer agent",
          ko: "리뷰어 소유 체크리스트가 있으나 내장 리뷰어 에이전트는 없음",
        },
      },
      {
        state: "nd",
        source: "Community extensions",
        href: `${SPECKIT_DOCS}/community/extensions.html`,
        note: {
          en: "Not in the core docs; only community extensions add retrospectives",
          ko: "코어 문서에는 없고 커뮤니티 확장만 회고를 추가",
        },
      },
    ],
  },
  {
    tool: "AWS AI-DLC",
    href: AIDLC,
    cells: [
      {
        state: "yes",
        source: "README",
        href: AIDLC,
        note: {
          en: "Requirements, stories, design, and a per-Unit plan come before code",
          ko: "코드 전에 요구사항, 스토리, 설계, Unit별 플랜을 작성",
        },
      },
      {
        state: "no",
        source: "README",
        href: AIDLC,
        note: {
          en: "Runs inside Claude Code, Kiro, Codex, and other agents",
          ko: "Claude Code, Kiro, Codex 등 다른 에이전트 안에서 실행",
        },
      },
      {
        state: "no",
        source: "Interaction modes",
        href: `${AIDLC_DOCS}/guide/07-interaction-modes.md`,
        note: {
          en: "Relies on the host agent's permission model",
          ko: "호스트 에이전트의 권한 모델에 의존",
        },
      },
      {
        state: "partial",
        source: "Construction stages",
        href: `${AIDLC_DOCS}/reference/04-stages/construction.md`,
        note: {
          en: "traceability.json maps requirement IDs to code and test targets",
          ko: "traceability.json이 요구사항 ID를 코드·테스트 대상에 연결",
        },
      },
      {
        state: "yes",
        source: "Phases and stages",
        href: `${AIDLC_DOCS}/guide/04-phases-and-stages.md`,
        note: {
          en: "Unit approval requires a tool-recorded verification receipt",
          ko: "Unit 승인에는 도구가 기록한 검증 영수증이 필요",
        },
      },
      {
        state: "yes",
        source: "Interaction modes",
        href: `${AIDLC_DOCS}/guide/07-interaction-modes.md`,
        note: {
          en: "Every stage after initialization ends at an approval gate",
          ko: "초기화 이후 모든 단계가 승인 게이트로 끝남",
        },
      },
      {
        state: "yes",
        source: "Agents",
        href: `${AIDLC_DOCS}/guide/06-agents.md`,
        note: {
          en: "A separate reviewer sub-agent on 11 stages; open findings go to the human gate",
          ko: "11개 단계에서 별도 리뷰어 서브에이전트가 실행되고, 남은 지적은 사람의 게이트로 감",
        },
      },
      {
        state: "yes",
        source: "Rules and the learning loop",
        href: `${AIDLC_DOCS}/guide/09-rules-and-the-learning-loop.md`,
        note: {
          en: "Corrections confirmed at a gate are saved as rules",
          ko: "게이트에서 확인한 교정을 규칙으로 저장",
        },
      },
    ],
  },
  {
    tool: "OpenHands",
    href: OPENHANDS,
    cells: [
      {
        state: "partial",
        source: "Custom agents",
        href: `${OH_DOCS}/sdk/guides/agent-custom`,
        note: {
          en: "A planning agent writes a plan; no spec-to-task stages",
          ko: "플래닝 에이전트가 플랜을 쓰지만 스펙→태스크 단계는 없음",
        },
      },
      {
        state: "yes",
        source: "README",
        href: OPENHANDS,
        note: {
          en: "Runs its own agent, or third-party agents such as Claude Code",
          ko: "자체 에이전트 또는 Claude Code 같은 외부 에이전트를 실행",
        },
      },
      {
        state: "yes",
        source: "Sandboxes",
        href: `${OH_DOCS}/openhands/usage/sandboxes/overview`,
        note: {
          en: "Docker sandbox recommended; process mode has no isolation",
          ko: "Docker 샌드박스 권장, process 모드는 격리 없음",
        },
      },
      {
        state: "partial",
        source: "QA changes",
        href: `${OH_DOCS}/openhands/usage/use-cases/qa-changes`,
        note: {
          en: "An optional QA plugin posts a per-PR report with a PASS/FAIL verdict",
          ko: "선택형 QA 플러그인이 PR마다 PASS/FAIL 판정 리포트를 게시",
        },
      },
      {
        state: "partial",
        source: "Goal completion loop",
        href: `${OH_DOCS}/sdk/guides/convo-goal`,
        note: {
          en: "Opt-in /goal: a judge model must see the evidence",
          ko: "옵트인 /goal: 판정 모델이 증거를 봐야 함",
        },
      },
      {
        state: "yes",
        source: "Confirmation mode",
        href: `${OH_DOCS}/openhands/usage/confirmation-mode`,
        note: {
          en: "Confirmation mode holds high-risk actions for approval",
          ko: "confirmation 모드가 고위험 행동을 승인 전까지 보류",
        },
      },
      {
        state: "partial",
        source: "SDK hooks",
        href: `${OH_DOCS}/sdk/guides/hooks`,
        note: {
          en: "Opt-in reviewer hooks and an experimental critic",
          ko: "옵트인 리뷰어 훅과 실험적 critic",
        },
      },
      {
        state: "partial",
        source: "Persistent memory",
        href: `${OH_DOCS}/sdk/guides/persistent-memory`,
        note: {
          en: "Opt-in MEMORY.md the agent rereads in later sessions",
          ko: "에이전트가 이후 세션에서 다시 읽는 옵트인 MEMORY.md",
        },
      },
    ],
  },
  {
    tool: "SWE-agent",
    href: SWE,
    cells: [
      {
        state: "no",
        source: "README",
        href: SWE,
        note: {
          en: "Takes an issue and leaves the approach to the model",
          ko: "이슈를 받아 접근 방식을 모델에 맡김",
        },
      },
      {
        state: "yes",
        source: "README",
        href: SWE,
        note: {
          en: "Runs the LM agent loop; its README now recommends mini-SWE-agent",
          ko: "LM 에이전트 루프를 실행하며, README는 이제 mini-SWE-agent를 권장",
        },
      },
      {
        state: "yes",
        source: "Architecture",
        href: `${SWE_DOCS}/background/architecture/`,
        note: {
          en: "SWE-ReX runs commands in a local or remote Docker container",
          ko: "SWE-ReX가 로컬 또는 원격 Docker 컨테이너에서 명령을 실행",
        },
      },
      {
        state: "nd",
        source: "Trajectories",
        href: `${SWE_DOCS}/usage/trajectories/`,
        note: {
          en: "Saves run trajectories; no per-scenario record found",
          ko: "실행 trajectory는 저장하지만 시나리오별 레코드는 찾지 못함",
        },
      },
      {
        state: "partial",
        source: "Default config",
        href: `${SWE}/blob/main/config/default.yaml`,
        note: {
          en: "The first submit returns a self-review checklist; the agent can submit anyway",
          ko: "첫 제출 시 자체 점검 체크리스트를 돌려주지만 에이전트는 그대로 제출할 수 있음",
        },
      },
      {
        state: "partial",
        source: "ShellAgent source",
        href: `${SWE}/blob/main/sweagent/agent/extra/shell_agent.py`,
        note: {
          en: "Experimental shell mode: a human submits the solution",
          ko: "실험적 shell 모드: 사람이 해결안을 제출",
        },
      },
      {
        state: "partial",
        source: "Agent config",
        href: `${SWE_DOCS}/reference/agent_config/`,
        note: {
          en: "An optional retry config picks the best of several attempts",
          ko: "선택형 retry 설정이 여러 시도 중 가장 나은 것을 고름",
        },
      },
      {
        state: "nd",
        source: "Demonstrations",
        href: `${SWE_DOCS}/config/demonstrations/`,
        note: {
          en: "Demonstrations are curated by hand",
          ko: "데모는 사람이 직접 고름",
        },
      },
    ],
  },
  {
    tool: "canon",
    href: CANON,
    cells: [
      {
        state: "partial",
        source: "Trust spine",
        href: "/concepts/trust-spine/",
        note: {
          en: "Imports openspec and superpowers plans and gates each task row; it does not write them",
          ko: "openspec·superpowers 플랜을 가져와 태스크 행마다 게이트하며, 직접 작성하지는 않음",
        },
      },
      {
        state: "no",
        source: "README",
        href: CANON,
        note: {
          en: "Records runs; it does not execute agent actions",
          ko: "run을 기록할 뿐 에이전트의 행동을 실행하지 않음",
        },
      },
      {
        state: "no",
        source: "README",
        href: CANON,
        note: {
          en: "Sandboxing is left to the external provider",
          ko: "샌드박스는 외부 provider의 몫",
        },
      },
      {
        state: "yes",
        source: "Trust spine",
        href: "/concepts/trust-spine/",
        note: {
          en: "An attributed evidence record per scenario, in the ledger",
          ko: "레저에 시나리오별로 귀속된 증거 레코드",
        },
      },
      {
        state: "yes",
        source: "CLI",
        href: "/cli/",
        note: {
          en: "canon gate check exits 1 on missing, stale, or fabricated evidence",
          ko: "증거가 없거나 오래됐거나 조작되면 canon gate check가 1로 종료",
        },
      },
      {
        state: "yes",
        source: "README",
        href: CANON,
        note: {
          en: "Risk tiers require policy-pinned SSH approval",
          ko: "위험 등급은 policy에 pin된 SSH 승인을 요구",
        },
      },
      {
        state: "yes",
        source: "Trust spine",
        href: "/concepts/trust-spine/",
        note: {
          en: "Opt-in: spec_coverage.require_review (0.12.0)",
          ko: "옵트인: spec_coverage.require_review (0.12.0)",
        },
      },
      {
        state: "yes",
        source: "Strategy memory",
        href: "/concepts/strategy-memory/",
        note: {
          en: "Distilled strategies stay quarantined until evaluated and signed",
          ko: "증류된 전략은 평가와 서명 전까지 quarantine 상태로 남음",
        },
      },
    ],
  },
];

const rowsFor = (lang: Lang, docsBase: string): Row[] =>
  matrix.map((row) => ({
    tool: row.tool,
    href: row.href,
    cells: row.cells.map((c) => ({
      state: c.state,
      source: c.source,
      href: c.href.startsWith("/") ? `${docsBase}${c.href.slice(1)}` : c.href,
      note: c.note[lang],
    })),
  }));

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
  headline: ["Agents say done.", "canon makes", "them prove it."],
  command: "bunx @journeykit/canon init",
  copy: "Copy",
  copied: "Copied",
  copyLabel: "Copy the install command",

  pr: {
    head: "agent/feat-cart",
    base: "main",
    blocked: "Merge blocked",
    checks: [
      { name: "cargo test", state: "pass", summary: "passed" },
      { name: "lint", state: "pass", summary: "passed" },
      { name: "canon gate check", state: "fail", summary: "2 scenarios lack evidence", log: GATE_LOG },
      { name: "deploy", state: "wait", summary: "waiting" },
    ] as Check[],
    exit: "exit 1",
  },

  loop: {
    heading: "Every run feeds the next",
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
    tool: "Tool",
    cols: [
      "Spec → plan → tasks workflow",
      "Runs the coding agent",
      "Sandboxed execution",
      "Per-scenario evidence record",
      "Completion gate on evidence",
      "Human approval gate",
      "Required independent review",
      "Learns from outcomes",
    ],
    states: { yes: "Yes", partial: "Partial", no: "No", nd: "Not documented" } as Record<CellState, string>,
    source: "Source",
    rows: rowsFor("en", "/"),
    canonNote: "Experimental (0.11.0, off by default): evidence binding to artifacts and JUnit/Cucumber reports.",
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
  command: "bunx @journeykit/canon init",
  copy: "복사",
  copied: "복사됨",
  copyLabel: "설치 명령 복사",

  pr: {
    head: "agent/feat-cart",
    base: "main",
    blocked: "병합 차단됨",
    checks: [
      { name: "cargo test", state: "pass", summary: "통과" },
      { name: "lint", state: "pass", summary: "통과" },
      { name: "canon gate check", state: "fail", summary: "시나리오 2개에 증거 없음", log: GATE_LOG },
      { name: "deploy", state: "wait", summary: "대기 중" },
    ],
    exit: "exit 1",
  },

  loop: {
    heading: "모든 실행이 다음 실행을 먹입니다",
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
    tool: "도구",
    cols: [
      "스펙 → 플랜 → 태스크 워크플로",
      "코딩 에이전트 실행",
      "샌드박스 실행",
      "시나리오별 증거 레코드",
      "증거 기반 완료 게이트",
      "사람 승인 게이트",
      "필수 독립 리뷰",
      "결과로부터 학습",
    ],
    states: { yes: "예", partial: "부분", no: "아니요", nd: "문서화되지 않음" },
    source: "출처",
    rows: rowsFor("ko", "/ko/"),
    canonNote: "실험 기능(0.11.0, 기본값 off): 아티팩트와 JUnit/Cucumber 리포트에 대한 증거 바인딩.",
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
