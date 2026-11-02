---
waive:
  - rule: "prose.length"
    section: "acceptance"
    reason: "다섯 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "이름과 철자는 사용자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "이름과 철자는 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "이름과 철자는 결정 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.length"
    section: "brief"
    reason: "한 변경에서 정해야 하는 사실이 많아 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "intent"
    reason: "한 변경에서 정해야 하는 사실이 많아 줄이면 결정이 빠진다"
---

# canon의 파괴적 계획 digest가 sha256:과 JCS를 쓰고 입력까지 덮는다

Source: https://github.com/journeyWorker/atip tag v1.2.0, spec 12.4-12.8, approaches/migration.md §3.1 5번과 §7, 계약 저자의 답(2026-11-02), issue 134

## Intent

canon의 파괴적 명령 6곳(`purge`, `artifacts move`, `migrate`, `migrate --revert`, `ledger compact`, `tier age`)은 미리보기가 `planDigest`를 내고 확정이 같은 digest를 요구한다. 그런데 digest는 효과 목록만 해시한다. 인수와 계획이 읽은 파일의 내용은 해시하지 않고, 직렬화는 serde의 필드 순서이고, `sha256:` 접두사가 없다. 계약 12.6은 digest가 효과와 그것을 정하는 모든 입력을 덮고 RFC 8785(JCS)의 SHA-256이기를 권한다. 계약 저자는 효과만 해시하는 것이 규칙에 못 미치고 이 이동은 canon에서 반드시 해야 한다고 답했다. `purge`의 확정은 감사 줄 하나를 쓰는데 미리보기의 효과 목록에 이 줄이 없다.

## Desired outcome

- canon의 6곳이 `atip::plan`을 쓴다. digest는 `sha256:`와 소문자 16진이고 `Plan { effects, inputs }`의 JCS의 SHA-256이다.
- `inputs`가 정규화한 인수 객체와 계획이 읽은 상태나 파일 내용의 digest를 담는다. 계획이 읽은 입력이 바뀌면 확정이 `PLAN_STALE`이다.
- ==**`purge` 미리보기의 효과가 확정이 쓰는 감사 줄을 싣는다.**==
- `canon/src/safety.rs`의 digest 직렬화와 접두사 없는 형식이 없어지고 호출 지점이 모두 옮겨진다.
- 확정은 저장소 쓰기 잠금을 확정 구간 동안 쥐어 계획이 확인과 효과 사이에 바뀌지 않는다.
- 이동 전에 낸 미리보기의 digest는 확정에서 `PLAN_STALE`이다. 업그레이드 노트가 이를 말한다.

## User flow

1. F1 에이전트는 파괴적 명령의 미리보기를 읽는다 → `planDigest`가 `sha256:`로 시작하고 효과와 입력을 덮는다 → 확정 호출을 그대로 실행한다.
2. F2 에이전트는 미리보기 뒤에 계획이 읽은 파일이나 상태가 바뀐다 → 확정이 `PLAN_STALE`로 거절된다 → 미리보기를 다시 한다.
3. F3 에이전트는 `purge` 미리보기의 효과를 읽는다 → 삭제할 기록과 감사 줄이 모두 있다 → 확정이 하는 일을 미리 안다.
4. F4 에이전트는 0.34가 낸 미리보기의 확정 호출을 실행한다 → `PLAN_STALE`이 온다 → ==**미리보기를 다시 한다.**==
5. F5 사람은 CHANGELOG의 업그레이드 노트를 읽는다 → digest 형식과 이동 전 미리보기의 운명이 적혀 있다 → 스크립트를 고친다.

## Step detail

### F1 digest

- ==**digest는 `sha256:` 뒤에 `{"effects":[…],"inputs":{…}}`를 JCS로 쓴 JSON의 SHA-256 소문자 16진 문자열이다.**== 효과의 순서는 도구가 정한 그대로이다.
- 6곳 모두 `atip::plan`의 `Plan`을 만들고 확정 관문(`gate`)과 미리보기의 `next` 항목, 확정 argv도 crate의 것을 쓴다. canon의 `safety.rs`는 canon의 효과 종류(`delete`, `skip` 등)와 `command()` 같은 canon 전용 부분만 남긴다.
- 미리보기 보고서의 `data`는 `planDigest`, `effects`, `committed`이다. 보고서 스키마가 crate의 `Effect`와 미리보기 스키마를 참조하고 `schemaVersion`의 minor가 오른다.

### F2 입력

- `inputs`는 호출을 정규화한 인수 객체(spec 11.7 6단계의 모양)와, 계획이 읽은 상태나 파일 내용의 digest이다. 명령마다 무엇을 읽는지가 다르다. 구현자가 명령별 표를 변경 기록에 싣는다.
  - `purge`: 정규화한 인수와, 삭제 후보를 정하는 데 쓴 기록 집합. 효과 목록이 이미 후보를 싣는다.
  - `artifacts move`: 인수와 옮길 파일의 내용 digest.
  - `migrate`와 `migrate --revert`: 인수와 옮길 파일과 설정의 내용 digest.
  - `ledger compact`, `tier age`: 인수와 계획이 읽은 기록 집합.
- 계획이 읽은 파일의 내용이 미리보기 뒤에 바뀌면 같은 확정이 `PLAN_STALE`이다(spec 12.5). exit 2이고 아무것도 바꾸지 않으며 `meta.next[0]`이 미리보기이다.
- 확정은 저장소 쓰기 잠금을 먼저 잡고, 잠금 아래서 계획을 다시 계산해 관문에 넘기고, 마지막 효과가 끝난 뒤에 잠금을 푼다. 계획이 확인과 효과 사이에 바뀔 수 없게 하는 것이 이 잠금의 목적이다(spec 12.7, MUST). 확정이 잠금 때문에 기다리다 시간이 다 되면 일을 시작하지 않고 거절한다.
- 아래는 저장소 쓰기 잠금이다.
- canon에는 저장소 전체의 쓰기 잠금이 없다. 있는 것은 시나리오 색인 잠금과 산출물 저장소 잠금이고 둘 다 한 저장소 부분만 보호한다. 둘은 같은 기반(`canon_store::file_lock::FileLock`, OS의 권고 잠금, 대기 시간 제한, 프로세스가 죽으면 풀린다)을 쓴다. 이 변경이 같은 기반으로 저장소 쓰기 잠금을 더한다. 잠금 파일 경로는 기존 두 잠금처럼 `canon_model::paths`의 상수이다.
- 여섯 파괴적 명령의 확정은 계획 재계산부터 마지막 효과까지 이 잠금을 쥔다. 미리보기는 잠금을 쥐지 않는다.
- 계획의 입력을 바꿀 수 있는 canon의 다른 쓰기도 쓰는 동안 같은 잠금을 쥔다. 대상은 원장 추가, 산출물 저장소 쓰기, 계층 이동, 이름 바꾸기와 설정 쓰기를 하는 명령이다. 구현자가 canon의 쓰기 명령을 전수 조사해 목록을 변경 기록에 싣는다. 조사 결과 계획 입력과 무관한 쓰기는 잠금 없이 둔다.
- 잠금을 못 얻고 기다림이 끝나면 일을 시작하지 않고 거절한다. 거절은 등록된 exit 2 코드이고 힌트가 잠시 뒤 다시 하라고 말한다. 기존 두 잠금의 busy 코드 방식을 따라 새 코드 이름을 구현자가 정하고 레지스트리에 `retryable`로 등록한다.
- 확정 중 프로세스가 죽어도 잠금은 풀린다. 잠금 파일은 지우지 않는다.

### F3 purge의 감사 줄

- `purge`의 확정은 삭제가 하나라도 있으면 접근 감사 줄 하나를 쓴다. 미리보기의 `effects`가 그 줄을 `append` 종류로 싣고 경로를 가진다. 삭제할 것이 없으면 감사 줄도 효과도 없다.
- 미리보기는 아무것도 쓰지 않는다. 확정이 쓰는 파일과 줄이 효과 목록과 같은지 시험한다.

### F4 이동 전 미리보기

- 0.34가 낸 접두사 없는 digest는 새 형식과 다르므로 `PLAN_STALE`이다. 이전 형식을 받아들이는 호환 경로를 두지 않는다.
- 호출자는 미리보기를 다시 해서 새 digest로 확정한다. 이 경우는 hint가 미리보기를 다시 실행하라고 말한다.

### F5 업그레이드 노트

- CHANGELOG의 이 버전 항목이 새 형식(`sha256:` 접두사, JCS, 입력 포함)과 이동 전 미리보기가 `PLAN_STALE`이 된다는 사실을 싣는다.
- 미리보기와 확정을 하나의 스크립트에서 곧바로 이어 쓰던 호출자는 영향이 없다. 미리보기와 확정 사이에 canon을 올린 호출자만 영향을 받는다.
- digest를 레코드나 백업에 저장하는 코드는 canon에 없다(0.34 조사). 이전 digest가 저장되어 무효가 되는 곳이 없다.

## Acceptance

- F1:
  - 6곳 모두의 미리보기가 `planDigest`를 `sha256:`와 64자 소문자 16진으로 낸다. 같은 계획은 같은 digest이다. 효과 순서나 값이 바뀌면 달라진다.
  - ==**미리보기의 digest로 만든 확정이 효과를 내고 `committed: true`이다.**==
  - 실패 경로: 접두사 없는 digest와 한 글자 다른 digest는 `PLAN_STALE`, exit 2이고 아무것도 바뀌지 않는다. `meta.next[0]`이 미리보기이다.
- F2:
  - 동시 쓰기 시험에서 한 프로세스가 확정으로 잠금을 쥔 동안 다른 canon 프로세스의 쓰기가 기다린다.
  - 그 쓰기는 확정이 끝난 뒤에 진행한다. 확정 도중 계획의 입력이 바뀌는 일이 없다.
  - 실패 경로: 기다림 시간이 끝나면 확정이 일을 시작하지 않고 등록된 exit 2 코드로 거절한다. 프로세스를 죽이면 잠금이 풀리고 파일이 남아도 다음 확정이 진행된다.
  - 미리보기 뒤에 계획이 읽은 파일(`artifacts move`의 대상 파일, `migrate`의 원본)을 바꾸면 확정이 `PLAN_STALE`이다.
  - 실패 경로: 인수만 같고 입력이 같은 확정은 통과한다. 인수 하나를 바꾸면 digest가 달라진다. 서로 다른 인수로 만든 digest를 다른 호출에 쓰면 `PLAN_STALE`이다.
- F3:
  - `purge` 미리보기의 `effects`에 감사 줄의 `append` 효과가 있고, 확정이 쓴 파일과 줄이 효과 목록과 일치한다. 삭제할 것이 없는 호출은 효과에 감사 줄이 없다.
  - 실패 경로: 미리보기와 거절이 아무것도 쓰지 않는다. 스냅샷이 같다.
- F4:
  - 0.34 바이너리로 만든 digest로 확정을 부르면 `PLAN_STALE`이다.
- F5:
  - CHANGELOG에 업그레이드 노트가 있고 digest 형식이 새로 적혀 있다. 소스에 접두사 없는 digest를 만드는 코드가 남아 있지 않다.
- `atip check`의 `L2.destructive-preview`, `L2.destructive-preview-stdin`, `L2.destructive-stale`, `L2.destructive-half`, `L2.destructive-commit`이 canon에서 통과한다. `cargo test --workspace`, conformance, 생성 문서 drift 시험이 통과한다.

## Questions and assumptions

- 계약 저자가 정했다. digest는 `sha256:`와 `{"effects","inputs"}`의 JCS의 SHA-256이고 이 이동은 canon에서 필수이다. `inputs`는 정규화한 인수 객체와 계획이 읽은 상태나 파일 내용의 digest이다.
- 계약 저자가 정했다. 계약 12.7은 MUST이다. 계획이 확인과 효과 사이에 바뀔 수 있으면 효과마다 확인하거나 잠금을 쥐어야 한다. canon의 프로세스 둘이 한 저장소에서 동시에 돌 수 있으므로 확정은 저장소 쓰기 잠금을 확정 구간 동안 쥔다. 이미 있는 쓰기 잠금이 있으면 재사용한다. 동시에 쓰는 프로세스의 시험이 필요하다.
- 가정: 보고서 스키마가 `Effect`를 crate에서 가져오면 보고서 `schemaVersion`의 minor가 오른다. 효과의 `append` 종류 추가는 데이터 변경이므로 `purge` 보고서의 minor도 오른다.
- 정책 질문은 없다. 6곳의 확정에 필요한 승인과 gate는 바뀌지 않는다.

## Out of scope

- crate의 plan 모듈(`canon-0-35-crate-plan`).
- 효과마다 계획을 다시 확인하는 방식. 이 변경은 잠금 방식을 택한다.
- 파괴적 명령의 효과를 바꾸는 일. 읽는 입력을 효과 목록에 더하는 것 말고는 하지 않는다.
- `STOPPED_MIDWAY`의 `PLAN_STALE` 이유(`canon-0-35-canon-stopped-midway`). canon에는 그 경로가 없다.

Approved: Journey Hwang 2026-11-02T06:53:19Z — "휴가 중 승인 위임(2026-11-01 대화); 0.35 즉시 시작은 사용자 결정(ATIP 세션 경유 2026-11-02)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:f8422b84f4164201579e87b4ce002f5b0c4efca315ae68578947009d7d30a085)
