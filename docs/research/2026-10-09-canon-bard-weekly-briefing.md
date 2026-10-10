# Canon·Bard 주간 리서치 브리핑 — 2026년 10월 9일

## 출처와 검증 (2026-10-11)

원문은 ChatGPT 주간 리서치 대화다([공유 링크](https://chatgpt.com/share/6aca5a43-c520-83ec-a2cf-98210a167cf4)). 공유 페이지에 인용된 링크에서 브리핑 8건의 원문 URL을 찾았다. arXiv 초록과 GitHub 게시물을 직접 열어 핵심 수치를 대조했고, 각 절 제목 아래에 원문 링크를 달았다.

| # | 원문 | 초록·본문에서 확인한 수치 | 원문에서 확인하지 못한 내용 |
|---|---|---|---|
| 1 | [arXiv:2610.10619](https://arxiv.org/abs/2610.10619) | 6개 backend, 5개 benchmark. 성공 판정의 약 34.4%가 요구사항 위반. resolve rate 50.6% → 33.2% | 없음 |
| 2 | [arXiv:2610.11647](https://arxiv.org/abs/2610.11647) | 저장소 20,947개, 312쌍, 모델 3개, 실행 6,368회. 최종 답변이 사용한 스킬을 밝힌 비율 0.9%. 초록에는 두 가지가 더 있다: "설치된 스킬의 거의 4분의 1이 같은 일을 하는 스킬과 함께 설치돼 있다", "어느 스킬이 실행될지는 설치 위치가 결정하고 목록 순서는 거의 영향이 없다". | "5회 중 1회 대체", "핵심 기능의 3분의 1 이상 상실", pre-tool hook 효과는 초록에서 찾지 못했다. 본문 확인이 필요하다. |
| 3 | [arXiv:2610.12269](https://arxiv.org/abs/2610.12269) | mini-swe-agent +25.33%(76개 추가 해결), Moatless +15.67% | 검사 빈도를 조절한다는 설명은 초록에서 찾지 못했다. |
| 4 | [arXiv:2610.11725](https://arxiv.org/abs/2610.11725) | 독립 스위치 세 개(draft gate, breaking-change delegation, run mode)와 project floor 두 방식을 조합해 18가지 운영 방식 | 없음 |
| 5 | [arXiv:2610.11169](https://arxiv.org/abs/2610.11169) | 스킬 채택 2,193,119건. 복사 네트워크 기준 상위 100개 저장소를 검토하면 이후 고위험 스킬 채택의 14.9%를 막는다. 별점 상위 100개는 0.5%. | 없음 |
| 6 | [GitHub Blog](https://github.blog/ai-and-ml/github-copilot/reviewbench-an-open-benchmark-for-ai-code-review/) | (대조 중) | |
| 7 | [GitHub Changelog](https://github.blog/changelog/2026-10-07-local-sandboxing-for-github-copilot-now-generally-available/) | (대조 중) | |
| 8 | [arXiv:2610.10961](https://arxiv.org/abs/2610.10961) | paired development turn 20건 가운데 8건에서 material finding (95% 정확 신뢰구간 19.1–63.9%) | 없음 |

같은 대화에서 인용됐지만 브리핑 본문에는 들어가지 않은 관련 원문:

- [arXiv:2610.10978](https://arxiv.org/abs/2610.10978) *Probabilistic Sensing, Deterministic Authority: Admitting Model-Produced Observations into Sufficiency-Checked Governance Contracts*
- [arXiv:2610.11514](https://arxiv.org/abs/2610.11514) *SSCBench: Evaluating the Evidential Validity of Fault-Injection Tests for Tool-Using LLM Agents*
- [arXiv:2610.11179](https://arxiv.org/abs/2610.11179) *Who Pays the Review Cost? Triage, Fairness, and Accountability in AI-authored Pull Requests*
- [arXiv:2610.10659](https://arxiv.org/abs/2610.10659) *Applying Security by Design at the Point of Execution*
- [arXiv:2610.11559](https://arxiv.org/abs/2610.11559) *SWE-Journey: Long-Horizon, Multi-Turn Interaction*
- [arXiv:2610.10610](https://arxiv.org/abs/2610.10610) *Code Understanding is a Bottleneck for Coding Agents*
- [arXiv:2610.11858](https://arxiv.org/abs/2610.11858) *Trajectory-Guided Fault Localization for Agent Skill Evolution*

> 사용자가 2026-10-11에 전달한 자료다. 0.14.0을 배포한 뒤 이 자료로 다음 할 일을 찾고, 다음 릴리스 계획을 세우고, 구현까지 진행한다.

이번 주에는 바로 설계에 반영할 만한 변화가 8건 있었다. 가장 큰 흐름은 다음 네 가지다.

1. "테스트 통과"를 Outcome으로 간주하면 안 된다. 고정 테스트가 성공으로 판정한 실행 가운데 상당수가 실제 요구사항을 위반했다.
2. ContextPack에는 어떤 스킬을 제공했는지만 기록해서는 부족하다. 실제로 어떤 스킬이 선택·열람·적용됐는지, 그 출처와 버전까지 추적해야 한다.
3. 승인과 감시는 정적인 체크포인트가 아니라 실행 상태에 따라 강도가 달라지는 런타임 정책이어야 한다.
4. 에이전트의 모델 선택과 실행 권한은 분리해야 한다. 모델과 상관없이 도구 실행을 capability policy와 샌드박스로 제어하는 방식이 실전 표준으로 굳어지고 있다.

---

## 1. TestJack: 고정 테스트가 coding-agent 성공률을 크게 부풀린다

- 10월 7일, 연구 논문. *TestJack: Should you trust the results in coding benchmarks? Agentic Coding Benchmarks Auditing via Evaluator Evolution* — [arXiv:2610.10619](https://arxiv.org/abs/2610.10619)
- 관련성: 최상. Canon의 Evidence → Decision → Outcome 사슬, eval corpus, regression scoreboard와 닿아 있다.
- **근거.** TestJack은 에이전트가 제출한 패치마다 요구사항 위반을 겨냥한 추가 테스트를 만들고, 정답 패치도 통과하는 테스트만 증거로 채택한다. 이 방식으로 6개 frontier model backend와 5개 coding benchmark를 감사했다. 기존 평가에서 성공으로 판정된 실행의 약 **34.4%**가 실제로는 요구사항을 위반했고, 전체 resolve rate는 **50.6%에서 33.2%**로 떨어졌다.
- **왜 중요한가.** `tests_passed=true`를 Outcome으로 저장하면, 에이전트가 evaluator의 빈틈을 통과한 경우와 요구사항을 실제로 충족한 경우를 구별할 수 없다.
- **적용안.** Outcome을 pass/fail 하나가 아니라 다음 필드로 나눈다. `declared_test_result`, `requirement_coverage`, `adaptive_audit_result`, `counterexample_tests`, `evaluator_version`, `evaluator_evidence_digest`, `false_success_detected`. Regression scoreboard에는 기존 점수와 별도로 audited success rate를 두고, 성공한 실행 일부를 표본으로 뽑아 요구사항 기반 evaluator-evolution을 돌린다.

## 2. One Skill Too Many: 비슷한 스킬이 함께 있으면 조용히 엉뚱한 스킬이 실행된다

- 10월 8일, 대규모 실증 연구. *One Skill Too Many: How Co-Installed Skills Conflict in Coding Agents* — [arXiv:2610.11647](https://arxiv.org/abs/2610.11647)
- 관련성: 최상. ContextPack, skill registry, 실행 lineage와 닿아 있다.
- **근거.** 20,947개 저장소에서 유사 스킬 쌍을 조사해 312쌍을 확인하고, 3개 모델로 6,368회 실행했다.
  - 비슷한 스킬이 함께 설치돼 있으면 약 5회 중 1회는 의도한 스킬 대신 유사 스킬이 선택됐다.
  - 유사 스킬을 먼저 읽은 실행은 원래 스킬에만 있는 핵심 기능의 3분의 1 이상을 잃었다. 일반적인 task-completion 평가는 이것을 잡지 못했다.
  - 대체 실행에서 최종 답변이 실제로 사용한 스킬을 밝힌 비율은 **0.9%**였다.
  - 충돌은 대개 첫 스킬을 읽는 순간 결정됐고, 그 시점의 pre-tool hook이 충실도를 회복시켰다.
- **적용안.**
  - RunManifest에 다음 필드를 추가한다. `available_skills[]`, `candidate_skill_matches[]`, `selected_skill`, `first_skill_read`, `skill_source`, `skill_version` 또는 content hash, `skill_resolution_reason`, `conflicting_skills[]`, `exclusive_obligations[]`, `obligations_satisfied[]`.
  - 이름·description·capability·금지사항이 겹치는 스킬은 실행 전에 탐지한다. 충돌하면 자동으로 고르지 말고 deterministic resolver나 승인 단계로 넘긴다.
  - "git 조작 금지", "배포 금지" 같은 배타적 의무사항은 task success와 별개의 평가축으로 검사한다.
- **Canon 메모.** dogfood eval 1에서 실제로 같은 일이 일어났다. Codex가 canon 스킬을 읽고도 description이 "game"을 내세운 `sites` 스킬을 골랐다(FINDINGS F2).

## 3. Cadence: 고정 간격 감시 대신 실행 상태에 따라 개입 강도를 바꾼다

- 10월 8일, 연구 논문. *Cadence: Strategic Guidance for Coding Agents* — [arXiv:2610.12269](https://arxiv.org/abs/2610.12269)
- 관련성: 매우 높음. Bard 오케스트레이션, risk-tier approval, runtime monitor와 닿아 있다.
- **근거.** Cadence는 trajectory를 검사하다가 정상이거나 경미하게 이탈한 실행에는 advisory guidance를, 심각한 오작동에는 replacement-level guidance를 준다. 심각한 문제를 찾으면 검사 빈도를 높이고, 안정적인 실행에서는 감시를 늦춘다. SWE-bench Lite 300개 작업에서 vanilla agent 대비 mini-swe-agent는 **+25.33%p**, Moatless는 **+15.67%p** 향상됐다.
- **적용안.** Healthy → Advisory → Corrective → Approval-required 4단계로 운영한다. Evidence에는 `risk_score`, `monitor_reason`, `intervention_level`, `inspection_interval`, `pre_intervention_state`, `post_intervention_outcome`을 남긴다.

## 4. Spec Growth Engine: 스펙 그래프와 코드·테스트 증거를 deterministic하게 결합한다

- 10월 8일, 구현 보고서. *Implementing the Spec Growth Engine: Preventing Spec-Code Divergence, and Growing the Spec with Agents* — [arXiv:2610.11725](https://arxiv.org/abs/2610.11725)
- 관련성: 매우 높음. knowledge map, 문서 freshness, spec-code lineage, 승인 정책과 닿아 있다.
- **근거.** 두 층을 나눈다.
  - deterministic layer가 스펙 그래프를 검증하고 코드 import graph와 비교한다. spec node는 테스트 증거가 있을 때만 verified로 승격된다.
  - 에이전트는 스펙을 확장하지만, 다음 라운드로 넘어갈지는 deterministic rule이 정한다.
  - draft gate, breaking-change delegation, run mode를 독립적인 축으로 둬서 18가지 운영 조합을 만든다.
  - 저자의 핵심 주장: 자율 실행의 신뢰도는 그것을 측정하는 deterministic instance의 품질을 넘을 수 없다.
- **적용안.** spec node의 상태를 draft / implemented / evidence-attached / verified / stale / diverged / waived로 둔다. 코드 변경이 영향을 주는 spec node를 dependency graph에서 계산하고, stale node가 있으면 자동 병합을 막거나 risk tier를 올린다. 대규모 비교실험이 아니라 구현 보고서이므로 효과는 Canon 자체 eval로 검증해야 한다.

## 5. Skill Constellations: 스킬은 버전과 출처를 관리해야 하는 공급망이다

- 10월 8일, 공급망 실증 연구. *Skill Constellations: Tracing the Supply Chain of Agent Skills on GitHub* — [arXiv:2610.11169](https://arxiv.org/abs/2610.11169)
- 관련성: 매우 높음. skill provenance, ContextPack 재현성, 보안과 닿아 있다.
- **근거.** GitHub에서 스킬 채택 2,193,119건을 Git 이력으로 추적했다.
  - 원본이 수정돼도 복사본은 거의 업데이트되지 않았다.
  - 별점 상위 100개 저장소를 감사하면 이후 고위험 스킬 채택의 **0.5%**만 잡혔다. 복사 네트워크로 고른 100개를 감사하면 **14.9%**가 잡혔다.
  - 연구진은 복사 배포 대신 versioned reference를 쓰자고 제안한다.
- **적용안.** skill registry에 immutable `skill_id`, semantic version, source repository와 source commit, content digest, parent/fork provenance, capability declaration, policy and permission digest, revoked/superseded 상태, known consumers와 실행 lineage를 둔다. ContextPack에는 스킬 본문 대신 `skill_id@version + digest`를 넣고, 실행 전에 lockfile과 registry 상태를 대조한다.
- **Canon 메모.** `canon skills install`이 이미 install lock(path→hash)을 쓴다. 출처 commit, 버전, revoked 상태를 추가로 담는 방향으로 확장할 수 있다.

## 6. ReviewBench: 오프라인 평가를 실제 프로덕션 결과와 연결한 공개 사례

- 10월 5일, GitHub 공식 벤치마크·운영 사례. *ReviewBench: An open benchmark for AI code review* — [GitHub Blog](https://github.blog/ai-and-ml/github-copilot/reviewbench-an-open-benchmark-for-ai-code-review/)
- 관련성: 높음. eval corpus, regression scoreboard, 오프라인 결과와 온라인 결과의 연결과 닿아 있다.
- **근거.**
  - PR 1억 390만 개의 분포를 분석해 19개 언어, 187개 저장소에서 PR 219개로 corpus를 만들었다.
  - 사람 리뷰, 여러 모델, 정적 분석 결과로 golden set을 만들었고, 독립 시니어 엔지니어 재검증과의 일치율은 **96.6%**였다.
  - grounded precision/recall(golden set 안의 문제만 계산)과 augmented precision/recall(새로 찾은 유효 문제도 인정)을 나눠 본다.
  - 데이터셋, rubric, judge 설정, runner를 모두 공개했다.
  - Copilot code review 온라인 A/B 결과의 방향이 오프라인 예측과 일치했다. addressed rate **+8.0%**, recall **+13.6%**, cost per review **−8.0%**.
- **적용안.** Canon eval schema에 다음 축을 따로 둔다. severity, defect category, grounded vs novel finding, precision/recall operating point, human addressed rate, additional-review-needed rate, cost per accepted finding, offline/online directional agreement. evaluator, matcher, dataset은 모두 버전을 붙여 RunManifest에 남긴다.

## 7. GitHub Copilot 로컬 샌드박스 GA: 모델과 실행 권한을 분리한다

- 10월 7일, 공식 제품·운영 기술. *Local sandboxing for GitHub Copilot now generally available* — [GitHub Changelog](https://github.blog/changelog/2026-10-07-local-sandboxing-for-github-copilot-now-generally-available/)
- 관련성: 높음. Bard capability policy, 승인 경계, vendor-neutral runtime과 닿아 있다.
- **근거.** Copilot CLI·앱·VS Code Agent Host에 로컬 샌드박스가 GA로 나왔다.
  - 하나의 정책을 Windows·macOS·Linux 네이티브 통제로 변환해 파일시스템, 인터넷, 로컬 네트워크, Git 자격증명, GitHub CLI 자격증명 접근을 제한한다.
  - 로컬 MCP와 language server에도 적용할 수 있다.
  - 조직 정책으로 강제하면 사용자가 약화할 수 없다.
  - GitHub는 모델 실행과 도구 격리를 별개의 문제로 명시한다.
- **적용안.** runtime adapter contract에 공통 CapabilityPolicy를 둔다. 항목은 filesystem read/write roots, network allow/deny targets, credential visibility, shell and subprocess permissions, MCP server permissions, Git operation scope, maximum runtime·cost·process count다. Decision에는 "에이전트가 실행을 요청했다"를, Outcome에는 "정책 엔진이 실제로 허용·차단한 operation의 digest"를 기록한다.
- **Canon 메모.** canon은 실행을 소유하지 않는다는 방향(2026-10-10 대화)과 맞는다. canon은 정책을 강제하는 쪽이 아니라 정책과 그 판정 결과를 기록하는 쪽이다.

## 8. Cross-Provider Review Contract: 독립 리뷰도 입력·종료 상태·불변 증거가 없으면 무의미하다

- 10월 7일, 소규모 pilot·fault-injection 연구. *Cross-Provider Review as a Runtime Contract for Coding Agents: A Controlled Pilot and Fault-Injection Study* — [arXiv:2610.10961](https://arxiv.org/abs/2610.10961)
- 관련성: 높음. vendor-neutral review, independent critic, audit evidence와 닿아 있다.
- **근거.** 다른 provider의 두 번째 에이전트가 결과를 검토하는 계약을 제안한다. 요건은 별도 resource pool, 제한된 capability, 완전한 입력 전달, bounded execution, 명시적 실패 상태, 실행별 영속 증거다. paired development turn 20건 가운데 8건에서 유의미한 finding이 나왔다. fault injection으로 하니스 결함도 드러났다.
  - 입력이 일부만 전달됐는데 review 성공으로 처리됐다.
  - process reaping 중 cancellation이 누락됐다.
  - reviewer가 non-zero로 종료했는데 verdict 형식이 정상이라 complete로 처리됐다.
  - 종료 상태가 실행별로 기록되지 않아 사후에 복구할 수 없었다.

  소규모 pilot이며 field reliability를 주장하지 않는다.
- **적용안.** ReviewEvidence에 다음을 필수로 둔다. reviewer provider/model/runtime, input manifest와 content digest, input completeness check, allowed capabilities, attempted policy violations, exit code·signal·timeout, structured verdict schema version, material findings, reviewer cost·duration, 원래 결과와 리뷰 결과의 binding. reviewer 실패는 `approved=false`로 처리하지 말고 `review_unavailable` 또는 `indeterminate`로 분리한다.
- **Canon 메모.** 0.13 self-review와 dogfood에서 리뷰어 신원은 구현자가 정해 준 문자열이었다(F11). 0.14 K4에서 session id를 붙인다. 입력 manifest와 종료 상태는 그다음 단계다.

---

## 우선순위 (브리핑 원문)

**P0: 바로 스키마에 반영**
1. Outcome에서 고정 테스트 결과와 adaptive audit 결과를 분리한다.
2. RunManifest에 실제로 선택·열람된 스킬과 충돌 후보를 기록한다.
3. 스킬 source·version·digest·provenance를 담은 lockfile을 도입한다.
4. evaluator·dataset·judge·matcher의 버전과 digest를 저장한다.
5. tool execution 결과에 정책 판정·exit code·signal·timeout을 저장한다.

**P1: 작은 프로토타입 권장**
1. execution-health 기반 monitor (Healthy → Advisory → Corrective → Approval)
2. spec graph와 코드 dependency graph 사이 divergence 검사
3. 성공 실행 일부를 표본으로 뽑아 TestJack식 요구사항 기반 재감사
4. cross-provider read-only reviewer contract와 fault-injection test
5. severity·category·precision/recall을 분리한 regression scoreboard

## 한 줄 결론

어떤 컨텍스트와 스킬이 실제로 선택됐는지, 어떤 권한 아래 무엇이 실행됐는지, 어느 버전의 evaluator가 어떤 반증 가능한 증거로 성공을 판정했는지가 하나로 묶이지 않으면, 성공 점수도 승인 기록도 독립 리뷰도 조용히 거짓이 될 수 있다.
