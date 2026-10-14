# Eval 7 결과: canon 0.19 on omp (2026-10-15)

`PROTOCOL-7.md` 기준. 레포 `nyan-omp-020`, 로그 `run-k-omp-020-t{1,2,4,5}.log`. 바이너리는 feat/0.19 c8671f8b(평가 중 발견한 두 결함을 고치기 전).

## 인수 기준 (plan D10)

| 기준 | 결과 | 근거 |
|---|---|---|
| (a) 턴 1: 멈추기 전 브리프 차단 0, 한국어 보고, 멈추기 전 `.canon/` 밖 쓰기 0 | 조건부 합격 | 99초에 한국어 PRD 브리프(F1–F4, 섹션 표식, 질문·가정)로 멈췄다. trace_check: 멈추기 전 바깥 쓰기 0. 구조 차단 0이었지만 `brief.flow-uncovered` 4건이 떴다(결함 1). |
| (b) 턴 2: `canon brief approve`, 흐름 단계마다 시나리오와 실패 경로, shipped, gate clean | 합격 | 승인 줄에 `brief sha256:7f0c…` 다이제스트. status가 F1–F4별 시나리오 12개를 보여 주고 단계마다 `[failure]`가 있다. 72/72 evidenced·reviewed, gate clean. 리뷰어는 별도 세션. |
| (c) 승인 뒤 브리프 수정 시 거절 | 합격 | 한 단어 수정 후 `status`, `evidence add`(exit 1), shipped 재개(reopen) 모두 `BRIEF_CHANGED_SINCE_APPROVAL`과 재승인 명령을 냈다. 되돌리자 정상. |
| (d) 턴 4–5: `--kind chore`, 사람 멈춤 1회, 새 시나리오 0, `--all-scenarios` 재결합, 재리뷰, shipped | 합격 | `<!-- canon:kind chore -->` 브리프로 95초 만에 멈춤. 승인 후 리팩터 커밋에 스펙 변경 0, 시나리오 72 유지. `evidence add --all-scenarios`로 재결합, 다른 세션(`reviewer-split`)이 재리뷰, shipped, gate clean. 에이전트가 나누기 전후 18판 시뮬레이션 상태를 바이트 비교해 증거로 묶었다. |

## 평가 중 고친 것 (feat/0.19 7a15637e)

1. **새 범위의 연결 규칙이 승인 전에 걸림.** 주제가 이미 `shipped`라 "building 이후" 조건이 맞아, 아직 쓸 수 없는 시나리오를 차단으로 보고했다. 연결 규칙은 이제 브리프가 승인된 뒤에만 적용한다. `brief approve`는 연결 규칙을 보지 않는다(진행 중 범위 변경도 재승인 가능).
2. **흐름 단계 문장 길이 권고가 과다.** "본다 → 한다 → 된다"는 세 절이라 단계마다 걸렸다(15건 중 다수). 이제 `→`로 나눈 절마다 잰다.

## 그 밖에

- 턴 1 첫 시도는 프롬프트가 빈 채로 전달됐다(셸 치환이 안 됨). 그 세션은 따로 치우고 `@file`로 다시 돌렸다.
- 기계 부하가 커서(load average 550–670) 게임 테스트 시간 제한을 에이전트가 120초로 올렸고, finding으로 기록 후 닫았다.
- Playwright의 실제 Chrome 실행이 매번 약 1.4G의 code-sign 사본을 `/private/var/folders/…/X`에 남긴다. 평가가 끝날 때마다 지워야 한다.
- canon 호출: 턴 1 5회, 전체 54회(시나리오 72개, 주제 1개, 변경 2개).
