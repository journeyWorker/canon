---
waive:
  - rule: "prose.code-citation"
    section: "intent"
    reason: "코드와 이유 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "코드와 이유 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "코드와 이유 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "코드와 이유 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "코드와 이유 이름은 검증 대상의 철자 자체다"
  - rule: "prose.length"
    section: "detail"
    reason: "이유 표가 결정의 본체라 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "brief"
    reason: "이유 표가 결정의 본체라 줄이면 결정이 빠진다"
---

# 오류 코드의 이유(reason)를 코드별 이름공간으로 정리한다

Source: https://github.com/journeyWorker/atip tag v1.4.2 spec 7.4.2, 7.6, 15.3, 계약 저자의 1.3.0 최종 규칙 댓글, canon issue #135의 5절

## Intent

한 코드 아래에서 다음 행동이 다른 상황은 이유로 가른다(계약 7.6). 코드를 합치고 가르는 앞선 변경들이 끝나면 한 코드에 여러 변경이 이유를 보탠다. 예를 들어 `CONFIG_INVALID`에는 대시보드, 스펙, 스킬, 정책 설정이 모두 모인다. issue #135의 5절은 이유가 조건 단어와 겹치거나(`CURSOR_INVALID`의 `EXPIRED`, `*_CHECK_FAILED`의 `STALE`), 이유가 종료 값이나 재시도 값을 바꿔야 하는 것처럼 보이는 경우를 적었다. 계약의 최종 규칙은 이유가 코드마다 따로인 이름공간이고, 이유는 종료 값과 재시도 값을 바꾸지 않는다고 정한다. 이 변경은 모든 이유를 이 규칙으로 정리한다.

## Desired outcome

- 오류 봉투나 `details` 항목의 `reason`은 그 코드 아래 등록된 이유뿐이다. 이유는 `{reason, meaning, fix}`이고 `fix`가 이유마다 다르다.
- ==**이유는 종료 값과 재시도 값을 바꾸지 않는다. 재시도 값이 다른 상황은 코드가 다르다.**==
- 여러 변경이 보탠 코드의 이유 목록이 하나로 정리된다. 이유 이름은 코드 안에서 유일하고 대문자와 밑줄이다.
- 계약이 이름공간으로 풀어 준 겹침(`EXPIRED`, `STALE`, `UNREADABLE`)은 이름을 바꾸지 않는다.
- 없어지는 이유는 `STOPPED_MIDWAY`의 `SKILLS_INVALID_ARGUMENT` 하나이고 CHANGELOG에 적힌다.

## User flow

1. F1 에이전트는 오류의 `reason`을 읽는다 → 그 코드 아래 등록된 이유이다 → ==**이유의 `fix`대로 행동한다.**==
2. F2 에이전트는 레지스트리 항목을 읽는다 → 상황이 여럿인 코드가 이유 목록을 가졌다 → 이유별 해결을 본다.
3. F3 에이전트는 설정이 틀렸다는 오류를 만난다 → `CONFIG_INVALID`의 이유가 틀린 파일이나 절을 가리킨다 → 그 파일을 연다.
4. F4 에이전트는 게이트 거절의 `details`를 읽는다 → 항목마다 조건에 맞는 자기 코드가 있고 이유는 다음 행동이 다를 때만 있다 → 항목별로 고친다.
5. F5 canon 기여자는 새 상황을 더한다 → 규칙이 이유와 새 코드를 가른다 → 시험이 그 규칙을 지킨다.
6. F6 사람은 CHANGELOG를 읽는다 → 바뀐 이유와 없어진 이유가 있다 → 스크립트를 고친다.

## Step detail

### F1 등록과 이름

- ==**이유는 `{reason, meaning, fix}`로 코드 아래에 등록하고, 봉투나 `details`는 등록된 이유만 싣는다.**== 이유가 코드를 대신하지 않는다. 이 시험은 0.35에서 있고 이 변경은 새 코드 전체로 넓힌다.
- 이유 이름은 대문자와 밑줄이고 한 코드 안에서 유일하다. 다른 코드가 같은 이름을 써도 된다. 계약 7.6이 이름공간을 코드별로 정한다.
- 조건 단어와 같은 이유는 이름을 그대로 둔다. `CURSOR_INVALID`의 `EXPIRED`, `INPUT_INVALID`의 `UNREADABLE`, `REPORT_CHECK_FAILED`의 `STALE`이 그렇다. 이름을 바꾸면 호출자가 깨진다. 계약 15.3이 그렇게 정한다.
- `INIT_INCOMPLETE`의 이유는 그 거절의 코드 하나이다. 계약 8.3.2가 그렇게 정한다. 앞선 변경이 이름을 바꾼 코드를 가리키므로 새 이름을 쓴다.

### F2 합쳐진 코드의 이유

- 이 표는 시작점이다. 구현자가 호출 지점이 실제로 내는 상황과 맞추어 이유를 확정하고 감사표에 적는다. 쓰지 않는 이유는 등록하지 않는다.

| 코드 | 이유 | 이유를 보탠 변경 |
|---|---|---|
| `CONFIG_INVALID` | `CANON_YAML`, `POLICY`, `DASHBOARD`, `SPECS`, `SKILLS`, `ARTIFACT_STORE` | rename, refusals, stored |
| `SUBJECT_CONFLICT` | `TRANSITION`, 채택 거절의 원인, `UNADOPT_APPROVED_BRIEF`, `UNADOPT_FLOW_TAGGED` | rename, exit, refusals |
| `BRIEF_INVALID` | `UNAPPROVABLE`, `BLOCKERS`, 읽은 내용의 원인 | rename, refusals, stored |
| `BRIEF_CHECK_FAILED` | `BLOCKERS` | rename |
| `CONTEXT_INPUT_INVALID` | `SECRET` | rename |
| `SKILLS_CHECK_FAILED` | `DRIFT` | rename |
| `REPORT_CHECK_FAILED` | `STALE`, 보고서가 없는 경우 | exit |
| `DISPATCH_CONFLICT` | `ALREADY_ENDED`, `NOT_RUNNING` | refusals |
| `DIVERGENCE_DENIED` | `NOT_REVIEWER`, `SELF_ATTESTED` | refusals |
| `DIVERGENCE_CONFLICT` | `NOT_DIVERGENT` | refusals |
| `MIGRATION_INCOMPLETE` | `STOPPED`, `INDEX`, `PINS` | incomplete |
| `INVENTORY_INCOMPLETE` | `VIOLATIONS`, `DUPLICATE_SCENARIOS` | incomplete |
| `INIT_INCOMPLETE` | 스킬 단계가 만나는 거절의 코드 | incomplete |

- 그대로인 것은 `CURSOR_INVALID`의 세 이유, `INPUT_INVALID`의 아홉 이유, `INPUT_TOO_LARGE`의 두 이유, `EVIDENCE_APPROVAL_INVALID`의 두 이유이다.

### F3 어느 파일인가

- `CONFIG_INVALID`의 이유가 틀린 파일이나 절을 가리킨다: canon.yaml, 정책, 대시보드, `specs:`, `skills:`, `artifact_store:`. `.canon/policy.yaml`은 canon.yaml이 아니므로 `POLICY`이다. `hint`가 그 파일을 고치는 방법을 말한다.

### F4 게이트의 거절

- 게이트 거절은 조건마다 다른 코드이다. 승인이 없으면 `<대상>_APPROVAL_REQUIRED`, 근거가 낡았으면 `EVIDENCE_STALE`, 그 밖의 위반은 `<대상>_INVALID`이다. 이 코드들은 `canon-0-36-codes-exit`가 정한다. 계약 저자의 규칙이다.
- 게이트의 위반 종류는 같은 조건 안에서 다음 행동이 다를 때만 이유가 된다. 11개 종류(`uncovered-cell`, `unreviewed-promotion`, `trust-below-required`, `stale-evidence`, `malformed-evidence`, `flagged`, `unevidenced-flip`, `fabricated-evidence`, `open-blocker`, `false-success`, `old-layout`)를 모두 이유로 옮기지 않는다. 같은 `*_INVALID` 아래에서 고칠 일이 다른 종류만 대문자와 밑줄의 이유로 등록한다.
- `details`의 항목은 자기 코드를 갖는다. 이유는 그 코드 아래 등록된 것만 싣는다.

### F5 이유와 코드를 가르는 규칙

- 같은 대상과 조건에서 다음 행동만 다르면 이유이다. 조건이 다르거나 종료 값이나 재시도 값이 다르면 코드이다. 계약 저자의 규칙이다.
- 앞선 변경이 코드로 가른 상황은 이유로 되돌리지 않는다. `ARTIFACTS_STORE_UNAVAILABLE`은 잠금과 원격이고 재시도 값이 예이다. `ARTIFACTS_STORE_REQUIRED`는 단계가 붙지 않은 경우이고 아니오이다. 승격의 거절 `PROMOTION_INVALID`는 종료 값 2이고 쓰기 실패 `PROMOTION_FAILED`는 종료 값 1이다.
- 시험이 코드마다 이유가 둘 이상이면 모든 방출 지점이 레지스트리의 `retryable`과 종료 값을 쓰는지 확인한다.

### F6 변경 기록

- CHANGELOG에 이유가 바뀐 코드의 표를 싣는다. 새로 생긴 이유, 이름이 바뀐 이유, 없어진 이유(`STOPPED_MIDWAY`의 이유 하나)를 모두 적는다.

## Acceptance

- F1:
  - 모든 방출 지점이 낸 이유가 그 코드 아래 등록되어 있다. 등록되지 않은 이유를 내는 시험은 실패한다.
  - 실패 경로: 이유가 없는 코드가 상황 둘 이상을 낸다고 감사표가 적으면 시험이 실패한다. 한 코드 안의 이유 이름이 겹치면 실패한다.
- F2:
  - 표의 코드마다 `canon schema --error <CODE> --json`이 확정된 이유 목록을 낸다. `{reason, meaning, fix}`가 모두 있고 `fix`가 이유마다 다르다.
  - 실패 경로: 쓰지 않는 이유가 등록되어 있으면 시험이 실패한다.
- F3:
  - ==**틀린 `specs:` 절, 틀린 정책, 틀린 canon.yaml이 각각 `CONFIG_INVALID`의 서로 다른 이유로 온다.**==
  - 실패 경로: 같은 파일의 같은 오류는 매번 같은 이유이다. 이유가 종료 값이나 재시도 값을 바꾸는 방출은 시험이 거절한다.
- F4:
  - 게이트에 거절된 `gate promote`의 `details` 항목마다 조건에 맞는 코드가 있고, 이유가 있으면 그 코드 아래 등록되어 있다.
  - 실패 경로: 다음 행동이 같은 두 종류가 서로 다른 이유로 등록되어 있으면 감사표가 이를 설명하지 못해 시험이 실패한다. 게이트에 없는 종류를 이유로 내면 실패한다.
- F5:
  - `ARTIFACTS_STORE_UNAVAILABLE`과 `ARTIFACTS_STORE_REQUIRED`가 별개 코드이고 재시도 값이 다르다. 이유 하나가 두 값을 오가는 방출이 있으면 시험이 실패한다.
- F6:
  - CHANGELOG의 이유 표가 레지스트리와 같다. `STOPPED_MIDWAY`가 없다.
- `cargo test --workspace`, conformance, 생성 문서 drift 시험이 통과한다.

## Questions and assumptions

- 가정: issue #135 5절의 겹침은 계약 7.6의 이름공간 규칙으로 풀렸다. `EXPIRED`, `STALE`, `UNREADABLE`은 이름을 바꾸지 않는다. 계약 저자의 1.3.0 최종 규칙 댓글이 근거이다.
- 가정: 이유로 등록하는 위반 종류는 구현자가 호출 지점과 `hint`의 다음 행동으로 정한다. 이름은 게이트의 이름을 대문자와 밑줄로 옮긴 것이다.
- 가정: 표의 이유는 시작점이고 구현자가 호출 지점에 맞춰 확정한다. 확정 목록이 표와 다르면 감사표에 이유를 적는다.
- 정책 질문은 이 변경에 새로 없다.

## Out of scope

- 코드를 합치고 가르는 일(`canon-0-36-codes-rename`, `canon-0-36-codes-exit`, `canon-0-36-codes-refusals`, `canon-0-36-codes-stored`, `canon-0-36-codes-incomplete`).
- 전체 대체표와 업그레이드 노트(`canon-0-36-codes-table`).
- 경고 코드의 이유.
