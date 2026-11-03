---
waive:
  - rule: "prose.code-citation"
    section: "intent"
    reason: "코드 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "코드 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "코드 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "코드 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "코드 이름은 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "코드 이름은 결정 대상의 철자 자체다"
  - rule: "prose.length"
    section: "detail"
    reason: "코드 표가 결정의 본체라 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "코드마다 확인할 사실이 많아 줄이면 확인 항목이 빠진다"
  - rule: "prose.length"
    section: "brief"
    reason: "코드 표가 결정의 본체라 줄이면 결정이 빠진다"
---

# 읽지 못한 파일과 깨진 저장 상태를 DENIED·INVALID·CORRUPT·UNAVAILABLE로 가른다

Source: https://github.com/journeyWorker/atip tag v1.4.2 spec 7.2.2, 7.2.3, 8.3, 계약 저자의 1.3.0 최종 규칙 댓글, canon issue #135의 (b)와 (c) 항목

## Intent

`BRIEF_UNREADABLE`, `CONFIG_UNREADABLE`, `REPO_UNREADABLE`처럼 "읽을 수 없다"는 이름의 코드 8개는 파일이 없는 경우, 권한이 없는 경우, 내용이 틀린 경우, 도구가 쓴 상태가 깨진 경우를 한 이름으로 낸다. 모두 재시도 불가로 등록되어 있어 일시적인 실패도 영구적으로 보인다. `WRITE_UNAVAILABLE`는 이름이 `UNAVAILABLE`(다시 하면 된다)인데 기다려도 풀리지 않는다. 반대로 `EVIDENCE_STORE_UNAVAILABLE`은 설정을 고쳐야 하는 경우까지 재시도 가능으로 낸다. 계약 1.3은 원인으로 조건을 정하고 재시도 값은 조건과 레지스트리가 정한다.

## Desired outcome

- 아래 표의 15개 코드와 `CONTEXT_PACK_CORRUPT`는 원인별 코드로만 나온다. 한 호출 지점은 원인 하나의 조건을 낸다.
- ==**파일이나 상태를 읽지 못한 실패는 원인이 `NOT_FOUND`, `DENIED`, `INVALID`, `CORRUPT`, `UNAVAILABLE` 중 하나로 가려진다.**==
- `UNAVAILABLE`은 다시 하면 풀릴 수 있는 경우에만 쓰고 재시도 값이 예이다. 설정을 고치거나 만들어야 하는 경우는 재시도 값이 아니오이다.
- `EVIDENCE_STORE_*`와 `ARTIFACTS_STORE_*`는 한 대상 `ARTIFACTS`로 모인다.
- 종료 값이 바뀌는 것은 `POLICY_APPROVAL_UNREADABLE`뿐이다. 1에서 2가 된다.

## User flow

1. F1 에이전트는 사람이 쓰거나 고치는 파일(canon.yaml, 정책, 브리프, 플랜, 스펙 루트, 전략 서명)을 읽는 명령을 한다 → 없으면 `NOT_FOUND`, 권한이 없으면 `DENIED`, 내용이 틀리면 `INVALID`가 온다 → ==**코드의 조건대로 파일을 찾거나 권한을 받거나 고친다.**==
2. F2 에이전트는 도구가 쓴 상태(매니페스트, 컨텍스트 팩, 정책 승인 파일, 장부, 블롭)를 읽는 명령을 한다 → 깨졌으면 `CORRUPT`가 온다 → 힌트대로 고치거나 다시 만든다.
3. F3 에이전트는 잠금이나 원격 저장소가 막은 명령을 한다 → `UNAVAILABLE`과 재시도 값 예가 온다 → 잠시 뒤 같은 호출을 한다.
4. F4 에이전트는 쓸 수 없는 대상에 쓰는 명령을 한다 → 읽기 전용이면 `WRITE_DENIED`, 디렉터리가 없거나 디렉터리가 아니면 `WRITE_INVALID`가 온다 → 대상을 고친다.
5. F5 사람은 안내와 CHANGELOG를 읽는다 → 옛 `*_UNREADABLE`이 어느 코드로 갈라졌는지 보인다 → 스크립트를 고친다.

## Step detail

### F1 원인과 조건

- ==**파일을 읽는 실패는 이 순서로 가른다.**==
  - 호출자가 이름을 준 경로가 없으면 `<대상>_NOT_FOUND`이다.
  - 도구가 있어야 한다고 보는 설정이 없으면 `<대상>_REQUIRED`이다.
  - 시스템이 접근을 거부하면 `<대상>_DENIED`이다.
  - 사람이 쓰거나 고치는 파일의 내용이 틀리면 `<대상>_INVALID`이다.
  - 도구가 쓴 상태가 해석되지 않거나 서로 맞지 않으면 `<대상>_CORRUPT`이다.
- 사람이 쓰거나 고치는 파일은 canon.yaml, 정책, 브리프, 플랜 문서, 스펙 파일, 전략 평가와 서명 파일이다. 도구가 쓰는 상태는 dispatch 매니페스트, 컨텍스트 팩, 정책 승인 파일, 장부 기록, 블롭, 색인이다.
- `<대상>_UNAVAILABLE`은 다시 하면 풀릴 수 있는 운영체제 오류(EIO 등)에만 쓰고 재시도 값이 예이다. 호출자가 준 경로가 틀린 경우(디렉터리 자리의 파일, 파일 자리의 디렉터리: EISDIR, ENOTDIR)는 `<대상>_INVALID`이다. 작업이 시작된 뒤의 실패는 효과가 있으므로 종료 값 1의 `FAILED`나 `INCOMPLETE`이다. 계약 저자의 규칙이다.
- 표의 이름 하나는 한 대상의 여러 원인을 이렇게 가른다. 예를 들어 `BRIEF_UNREADABLE`은 `BRIEF_NOT_FOUND`(이미 있다), `BRIEF_DENIED`, `BRIEF_INVALID`가 된다. `BRIEF_INVALID`의 이유는 `canon-0-36-code-reasons`가 정한다.
- 경고 코드(`CONFIG_UNREADABLE`, `DISPATCH_MANIFEST_MISMATCH`, `DISPATCH_MANIFEST_UNREADABLE`, `POLICY_APPROVAL_UNREADABLE`)는 이름이 그대로이다. 오류만 새 코드가 된다.

### F2 도구가 쓴 상태

- 깨진 도구 상태는 `<대상>_CORRUPT`이고 종료 값 2, 재시도 불가이다. `hint`가 고치거나 다시 만드는 명령을 말한다.
- 정책 승인 파일이 읽히지 않으면 지금은 종료 값 1로 "닫힌 게이트"처럼 출고를 막는다. 이제 `POLICY_APPROVAL_CORRUPT`(해석 불가)나 `POLICY_APPROVAL_DENIED`(권한)로 종료 값 2에 거절한다. `--override-reason`으로 넘어가지 않는 성질은 그대로이다.
- 컨텍스트 팩을 읽다가 깨졌음을 알면 `CONTEXT_PACK_CORRUPT`이다. `context-pack verify`의 답은 `canon-0-36-codes-exit`가 정한다.
- 저장소 읽기 실패는 대상으로 가른다. 저장소 디렉터리 접근이 거부되면 `REPO_DENIED`, 저장소가 아니면 `REPO_INVALID`, 장부 기록이 깨졌으면 `LEDGER_CORRUPT`이다.

### F3 잠금과 원격

- 저장소 쓰기 잠금을 기다리다 끝나면 `REPO_LOCK_UNAVAILABLE`이고 재시도 값 예이다. 이름이 조건으로 끝나지 않던 `REPO_WRITE_BUSY`가 바뀐 것이다.
- 인공물 저장소를 열 수 없거나 잠금이 잡혀 있으면 `ARTIFACTS_STORE_UNAVAILABLE`이고 재시도 값 예이다. 단계가 붙지 않았으면 `ARTIFACTS_STORE_REQUIRED`이고 재시도 불가이다. `artifact_store:` 절이 틀리면 `CONFIG_INVALID`와 이유 `ARTIFACT_STORE`, 위험한 저장 디렉터리는 `ARTIFACTS_STORE_INVALID`, 형식이 틀린 digest는 `ARTIFACTS_DIGEST_INVALID`이다.
- 처음 쓰기 전의 원인 여럿이 한 코드로 합쳐지고 재시도 값이 섞여 있던 `EVIDENCE_STORE_UNAVAILABLE`과 `ARTIFACTS_STORE_UNREACHABLE`은 이렇게 갈라진다. 재시도 값이 다른 상황은 코드로 가르고 이유로 가르지 않는다.

### F4 쓸 수 없는 대상

- 대상이 읽기 전용이면 `WRITE_DENIED`, 디렉터리가 없거나 디렉터리가 아니면 `WRITE_INVALID`이다. 둘 다 아무것도 바꾸기 전에 안다. 기다려서 풀리지 않으므로 재시도 불가이다.
- `ledger compact`가 장부 루트가 `ledger`라는 이름의 디렉터리가 아니어서 보관소를 못 찾으면 `LEDGER_ARCHIVE_INVALID`이다.
- 계획 문서에 작업의 행이 없으면 `PLAN_DOCUMENT_INVALID`이다. 계획 문서는 사람이 고치는 파일이다.

### F5 표와 소비자

| 지금 | 지금 exit | 새 코드 | 이유 | 새 exit | 재시도 |
|---|---|---|---|---|---|
| `ARTIFACTS_STORE_UNREACHABLE` | 2 | `ARTIFACTS_STORE_UNAVAILABLE` · `ARTIFACTS_STORE_REQUIRED` · `ARTIFACTS_STORE_INVALID` · `CONFIG_INVALID` · `ARTIFACTS_DIGEST_INVALID` | ARTIFACT_STORE | 2 | yes for UNAVAILABLE only |
| `BRIEF_UNREADABLE` | 2 | `BRIEF_INVALID` · `BRIEF_DENIED` · `BRIEF_NOT_FOUND` |  | 2 | 아니오 |
| `CONFIG_UNREADABLE` | 2 | `CONFIG_INVALID` · `CONFIG_DENIED` · `CONFIG_REQUIRED` |  | 2 | 아니오 |
| `DISPATCH_MANIFEST_MISMATCH` | 2 | `DISPATCH_MANIFEST_CORRUPT` |  | 2 | 아니오 |
| `DISPATCH_MANIFEST_UNREADABLE` | 2 | `DISPATCH_MANIFEST_CORRUPT` · `DISPATCH_MANIFEST_DENIED` |  | 2 | 아니오 |
| `EVIDENCE_STORE_UNAVAILABLE` | 2 | `ARTIFACTS_STORE_UNAVAILABLE` · `ARTIFACTS_STORE_REQUIRED` · `CONFIG_INVALID` | ARTIFACT_STORE | 2 | yes for UNAVAILABLE only |
| `LEARN_SIGNATURE_UNREADABLE` | 2 | `STRATEGY_SIGNATURE_INVALID` · `STRATEGY_SIGNATURE_NOT_FOUND` · `STRATEGY_SIGNATURE_DENIED` |  | 2 | 아니오 |
| `LEDGER_ARCHIVE_UNAVAILABLE` | 2 | `LEDGER_ARCHIVE_INVALID` |  | 2 | 아니오 |
| `PLAN_CORPUS_UNREADABLE` | 2 | `PLAN_CORPUS_INVALID` · `PLAN_CORPUS_DENIED` |  | 2 | 아니오 |
| `POLICY_APPROVAL_UNREADABLE` | 1 | `POLICY_APPROVAL_CORRUPT` · `POLICY_APPROVAL_DENIED` |  | 2 | 아니오 |
| `REPO_UNREADABLE` | 2 | `REPO_DENIED` · `REPO_INVALID` · `LEDGER_CORRUPT` |  | 2 | 아니오 |
| `REPO_WRITE_BUSY` | 2 | `REPO_LOCK_UNAVAILABLE` |  | 2 | 예 |
| `SPEC_ROOTS_UNREADABLE` | 2 | `SPEC_ROOT_INVALID` · `SPEC_ROOT_DENIED` |  | 2 | 아니오 |
| `TASK_ROW_NOT_FOUND` | 2 | `PLAN_DOCUMENT_INVALID` |  | 2 | 아니오 |
| `WRITE_UNAVAILABLE` | 2 | `WRITE_DENIED` · `WRITE_INVALID` |  | 2 | 아니오 |

- 소비자 이동, 시나리오 문장(6개), conformance, CHANGELOG 행은 `canon-0-36-codes-rename`과 같은 규칙으로 한다. 옛 이름 모음과 아직 옛 이름인 코드의 목록도 같다.

## Acceptance

- F1:
  - 읽기 권한을 뺀 브리프는 `BRIEF_DENIED`, 내용이 깨진 브리프는 `BRIEF_INVALID`, 이름을 준 변경에 브리프가 없으면 `BRIEF_NOT_FOUND`이다. 모두 종료 값 2, 재시도 불가이고 아무것도 쓰지 않는다.
  - 실패 경로: 권한 시험은 비루트로 돌고 루트면 건너뛰며 그 사실을 말한다. 디렉터리를 파일 자리에 둔 호출은 `_INVALID`이고 재시도 값 아니오이다.
- F2:
  - ==**깨진 dispatch 매니페스트는 `DISPATCH_MANIFEST_CORRUPT`이고 종료 값 2이다.**== 깨진 정책 승인 파일은 출고와 재개를 종료 값 2의 `POLICY_APPROVAL_CORRUPT`로 거절하고 `--override-reason`이 넘기지 못한다.
  - 실패 경로: 같은 매니페스트를 고치면 다음 호출이 성공한다. 경고로 같은 상황을 알리는 호출은 지금 이름의 경고를 그대로 낸다.
- F3:
  - 잠금을 쥔 프로세스가 있을 때 쓰기 명령이 `REPO_LOCK_UNAVAILABLE`, 종료 값 2, 재시도 값 예이다. 잠금이 풀리면 같은 호출이 성공한다.
  - 실패 경로: 단계가 붙지 않은 인공물 저장소 호출은 `ARTIFACTS_STORE_REQUIRED`이고 재시도 값이 아니오이다. `artifact_store:` 절이 틀리면 `CONFIG_INVALID`이다.
- F4:
  - 읽기 전용 대상은 `WRITE_DENIED`, 없는 디렉터리는 `WRITE_INVALID`이고 둘 다 아무것도 바꾸지 않는다. `ledger compact`의 틀린 루트는 `LEDGER_ARCHIVE_INVALID`이다.
- F5:
  - 표의 옛 이름은 레지스트리와 모든 `errors`와 봉투에 없고 `schema --error`가 거절한다. 새 코드의 종료 값과 재시도 값이 표와 같다.
  - 실패 경로: 옛 이름을 안내에 되살리면 옛 이름 시험이 실패한다. 재시도 값 예인 코드가 `UNAVAILABLE`이 아니면 레지스트리 시험이 실패한다.
- `cargo test --workspace`, conformance, 생성 문서 drift 시험이 통과한다.

## Questions and assumptions

- 계약 저자의 답: 다시 하면 풀릴 수 있는 운영체제 오류만 `_UNAVAILABLE`이고, 호출자의 잘못된 경로는 `_INVALID`, 작업이 시작된 뒤의 실패는 `_FAILED`나 `_INCOMPLETE`이다.
- 가정: 사람이 고치는 파일과 도구가 쓰는 상태의 구분은 F1의 두 목록이다. 구현자가 다른 파일을 만나면 목록에 더하고 감사표에 적는다.
- 가정: `CONFIG_INVALID`의 이유 이름은 `canon-0-36-code-reasons`가 정한다. 이 변경은 `ARTIFACT_STORE`를 필요한 이유로 등록한다.
- 가정: 시나리오 문장 6개를 고치면 그 리뷰가 stale이 된다. 리뷰어가 차이표를 보고 다시 리뷰한다. 이 가정이 맞지 않으면 사람이 정책을 정해야 한다.
- 정책 질문: `POLICY_APPROVAL_UNREADABLE`의 종료 값이 1에서 2로 바뀐다. 출고를 막는 성질은 같고 호출자가 보는 값만 다르다. 사람이 확인한다.

## Out of scope

- 이름만 바뀌는 코드(`canon-0-36-codes-rename`)와 여러 거절을 가리던 코드(`canon-0-36-codes-refusals`).
- 게이트 거절의 종료 값(`canon-0-36-codes-exit`)과 중간에 멈추는 명령(`canon-0-36-codes-incomplete`).
- 읽기 실패를 줄이거나 자동으로 고치는 일.
