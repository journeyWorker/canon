---
kind: chore
waive:
  - rule: "prose.length"
    section: "invariants"
    reason: "정리 항목 표가 확정된 항목 목록이라 문장을 줄이면 사실이 빠진다"
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "규칙 문서가 정하는 lint와 가시성 이름이 정리 대상 자체다"
    match: "unwrap()"
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "규칙 문서가 정하는 lint와 가시성 이름이 정리 대상 자체다"
    match: "for_each"
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "규칙 문서가 정하는 lint와 가시성 이름이 정리 대상 자체다"
    match: "pub(super)"
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "규칙 문서가 정하는 lint와 가시성 이름이 정리 대상 자체다"
    match: "pub(crate)"
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "규칙 문서가 정하는 lint 이름을 규칙 문서 변경 항목으로 그대로 가리킨다"
    match: "result_large_err"
  - rule: "prose.code-citation"
    section: "checks"
    reason: "사용자가 비교 대상에서 제외하는 JSON 필드 이름이다"
    match: "canonVersion"
---

# 0.30 Rust 코딩 규칙 정리

Source: docs/design/rust-coding-rules.md (Accepted, §9)

## Intent

canon의 Rust 코드는 승인된 코딩 규칙을 아직 따르지 않는다. 0.29.0에서 규칙의 lint를 켜면 약 1,300곳이 걸리고, 기본 clippy만으로도 114곳(그중 1곳은 오류)이 걸린다. 0.29가 구조 개편을 끝냈으므로 규칙을 코드 전체에 한 번에 적용한다. 경고를 끄지 않고 원인을 고친다.

0.30 순서: 계획 이전 → 스펙 보강 → 이 변경 → clippy CI → 레저 이름 변경 → 레코드 연결. 여섯 brief는 함께 승인받는다.

## Category

refactor

## Invariants

- ==**사용자가 보는 출력은 0.29.0과 바이트 단위로 같다.**== 대상은 모든 명령의 텍스트와 `--json` 출력, 종료 코드, 명령 명세 파일이다. 오류 봉투는 코드, 메시지, hint, 필드 모두 같다.
- 레저에 쓰는 레코드의 바이트와 이름은 바뀌지 않는다.
- 0.29의 비용 증가 모양 검사는 계속 통과하고, 아래 성능 정리 3건은 측정값이 나빠지지 않는다.
- 규칙이 정한 범위 밖의 이름, 모듈 배치, 공개 범위는 바꾸지 않는다.

정리 항목:

| 항목 | 방식 |
|---|---|
| lint 설정 | 규칙 §1의 lint 블록과 clippy 설정을 워크스페이스에 한 번 선언하고 모든 crate가 따른다 |
| 기본 clippy 경고 114곳 | 끄지 않고 고친다. 큰 오류 값은 CLI 오류의 선택 필드를 상자 하나로 묶어 해결한다. 시그니처가 외부(clap, trait)에 묶인 곳만 이유를 적은 `expect`로 남긴다 |
| 종료 코드 | CLI 오류와 그 출력 함수가 0–255 값을 다뤄 좁히는 캐스트 약 120곳을 없앤다. 나머지 캐스트는 검사하거나 범위 근거를 적는다 |
| panic | 운영 코드의 `unwrap()`은 전부(clippy 기준 13곳) 불변식을 적은 `expect`나 타입 있는 오류로, poisoned lock은 전파한다. `unsafe` 10곳에 이유와 SAFETY 설명을 단다 |
| `Debug`, `allow` | `Debug`가 없는 타입 267개에 붙이고 비밀 값은 직접 구현하며, `allow`는 이유를 적은 `expect`로 바꾼다 |
| import, 반복 | glob import와 `for_each`를 명시 import와 `for`로 바꾼다 |
| 공개 범위 | `pub(super)` 19곳을 `pub(crate)`로 바꾼다 |
| 오류 타입 | 문자열 오류를 돌려주는 공개 함수 52개에 실패 영역별 타입 오류를 준다. `Display`/`Error`가 없는 오류 타입 8개를 채운다. crate 사이 같은 이름 오류 4쌍을 crate 안에서 유일한 이름으로 바꾼다. 원인을 문자열로 뭉갠 곳은 원인을 잇되 사용자 메시지는 그대로 둔다. 쓰지 않는 `anyhow` 선언을 지운다 |
| 성능 3건 | evidence를 한 번만 역직렬화한다. 바인딩 계산이 evidence 전체를 복제하지 않는다. 페이지 조회가 페이지 밖 레코드의 커서 키를 만들지 않는다 |
| 규칙 문서 | 결정이 바뀐 곳을 같은 변경에서 고친다: 기본 경고 수, `result_large_err`를 상자로 해결한 결정, clippy 잡을 별도 CI 변경이 정리 뒤에 붙인다는 순서 |

출력에 닿는 곳이라 그대로 두는 것:

- 명령 명세의 오류 표 종료 코드는 32비트 정수로 남는다. 명세 파일의 형식이 바뀌지 않게 하기 위해서다.
- 자식 프로세스 종료 값은 음수를 담을 수 있어 32비트 정수로 남는다.
- 어휘 스냅샷 버전과 정책 바인딩 버전의 해시 계산에 쓰이는 `Debug` 출력이 있는 타입의 `Debug`는 바꾸지 않는다. 진단 메시지에 쓰이는 `{:?}` 출력도 그대로다.
- 대시보드 승인 잠금은 규칙대로 poisoned 상태를 전파한다. 차이는 앞서 내부 panic이 난 뒤에만 생긴다.

## Checks

- 스펙 보강이 끝난 뒤 0.29.0 바이너리로 기준을 저장한다. 기준은 canon 코퍼스의 고정 사본과 0.29 벤치 합성 코퍼스에 대한 `canon gate check --json`, `canon status --json`, `canon schema --errors --json`, conformance 케이스 출력이다. ==**각 단계 커밋마다 같은 입력으로 바이트 비교한다.**== `canonVersion` 값만 제외한다.
- 타입 오류로 바꾼 함수마다 오류 문자열이 이전 문자열과 같은지 단위 테스트로 확인한다.
- 각 단계 커밋에서 `cargo test --workspace`, conformance 케이스, 명령 명세 drift 테스트, canon 자기 `canon gate check`가 통과한다.
- lint 집계는 live tier feature를 켠 대상까지 포함하고, 집계 명령을 레포에 스크립트로 커밋한다. 끝났을 때 규칙 §1, §3, §4, §5의 집계가 0이고 전후 표를 남긴다.
- 0.29 벤치 증가 모양 검사가 통과하고, 성능 3건은 같은 벤치 지점에서 전후 CPU와 peak RSS를 기록한다.
- evidence: 0.30의 마지막 코드 커밋 뒤에 모든 subject의 시나리오를 다시 돌린다. 결과는 `canon evidence add --all-scenarios`로 한 번에 묶는다. 뒤 변경들이 코드를 다시 바꾸므로 이 변경 직후 묶은 evidence는 오래된다. 변경 단위 리뷰는 다른 세션의 reviewer가 한다.

## Out of scope

- 레저 이름과 종류 이름 변경, 레코드 연결(각각 별도 변경).
- CI의 clippy 잡과 공개 범위 검사 스크립트(clippy CI 변경).
- 다른 crate에서 쓰지 않는 공개 항목 약 1,000개를 좁히는 일, rustfmt 폭, edition 2024, 코드 전반의 newtype 도입.
- ingest 재해시와 행 보관, `report check`의 duckdb 메모리.
- id 플래그의 `-id` 접미사 제거.

Approved: Journey Hwang 2026-10-26T12:59:28Z — "레츠고" (brief sha256:6365ae61dd28f489302e94fbaa29213b737c5ac64826c3c715a6b1f89e75bf6c)
