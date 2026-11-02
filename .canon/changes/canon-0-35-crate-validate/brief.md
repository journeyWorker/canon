---
kind: chore
waive:
  - rule: "prose.code-citation"
    section: "intent"
    reason: "기능 이름과 의존성 이름은 읽는 쪽이 쓰는 철자 자체다"
---

# crates/atip: jsonschema를 validate 기능 뒤로 옮긴다

Source: https://github.com/journeyWorker/canon/issues/134 (첫 댓글 5번)

## Intent

`atip` crate는 `jsonschema`를 기본 의존성으로 끌어들이고, 그 의존성은 크레이트 24개를 더해 `atip`을 쓰는 모든 바이너리에 들어간다. 봉투와 필드 투영의 검증은 검사기와 시험의 일이다. 의존성을 `validate` 기능 뒤로 옮기고 검사기와 시험에서만 켠다. `atip`만 쓰는 도구는 링크하지 않게 된다. canon 자신은 예외이다. 레코드 검증(`canon-fmt`)이 `canon-gate`와 `canon-learn`과 `canon-vocab`을 거쳐 `jsonschema`를 일반 의존성으로 쓰므로 `canon-cli`는 계속 링크한다. 도메인 검사(`admits`)는 canon의 실행 경로가 쓰므로 `jsonschema` 없이 같은 답을 내야 한다.

## Category

dependency

## Invariants

- canon과 `atip check`의 출력은 0.34와 바이트까지 같다. 오류 코드, 종료 코드, 보고서, `schema` 답, 생성 문서가 모두 같다.
- 기능 이름은 `validate`이고 기본값은 꺼짐이다. `cargo tree -p atip`에 `jsonschema`가 없다.
- `validate`를 켜면 지금의 공개 함수가 모두 그대로 있다. 봉투와 레지스트리 항목과 스키마 문서의 검증 함수, 필드 투영의 검증, 도메인의 `Compiled`가 해당한다.
- `atip-check`와 `atip`의 시험은 `validate`를 켠다. `canon-cli`의 시험과 디버그 검증도 켠다.
- ==**`atip`에 기대는 도구가 `validate` 없이 `jsonschema`를 링크하지 않는다.**== 최소 소비자 시험이나 `atip`만 의존하는 시험용 크레이트의 `cargo tree`가 이를 증명한다. `canon-cli`는 `canon-fmt`를 거쳐 `jsonschema`를 계속 링크하고, 이 사실은 CHANGELOG에 크레이트 수와 함께 싣는다.
- 도메인 검사(`admits`)는 `jsonschema` 없이 자체 판정을 한다. 도메인 키워드는 모두 자체로 판정하고, `pattern`은 `regex` 크레이트로 한다. canon의 도메인 표 전체에서 `jsonschema`가 내는 답과 같다. 그렇게 할 수 없는 키워드가 있으면 구현자가 기록하고 그 키워드만 `validate` 뒤에 둔다.
- 새 의존성을 더하지 않는다. 호출 지점의 소스는 `use` 경로 말고 바뀌지 않는다.

## Checks

- ==**같은 입력으로 `canon`의 모든 명령과 `schema` 답을 0.34 바이너리와 바이트 비교한다.**== `atip check`의 보고도 같다.
- 차분 시험: canon의 도메인 표 전체에서 경계 안과 밖의 값으로 `admits`를 `validate` 없이 부른 답과 `jsonschema`가 낸 답이 같다. 이 시험은 `validate`를 켠 빌드에서만 돈다.
- `cargo tree -p atip`(기본 기능)에 `jsonschema`가 없고 `cargo tree -p atip --features validate`에는 있다. `atip`만 의존하는 최소 소비자(시험용 크레이트)의 `cargo tree`에도 `jsonschema`가 없다. `cargo tree -p canon-cli -e normal`에는 `canon-fmt`를 거쳐 `jsonschema`가 계속 있다.
- `cargo test --workspace`와 `just lint`의 clippy 세 줄이 같은 커밋에서 통과한다. 생성 문서 drift 시험과 canon 자신의 `canon gate check`가 통과한다.
- 크레이트 수의 변화를 변경 기록과 CHANGELOG에 싣는다. `atip` 기본 기능과 최소 소비자는 줄고, `canon-cli`는 `canon-fmt`를 거쳐 계속 링크하므로 줄지 않는다.
- evidence: 마지막 코드 커밋 뒤에 context-capability의 시나리오를 다시 돌려 묶는다. 변경 단위 리뷰는 다른 세션의 reviewer가 한다.

## Out of scope

- 새 기능이나 새 API, 검증 규칙의 변경.
- `jsonschema` 버전 올리기와 다른 검증 라이브러리로의 교체. `canon-fmt`와 `canon-gate`, `canon-learn`, `canon-vocab`의 레코드 검증기를 바꾸는 일도 포함한다.
- `schemars`와 `regex` 등 다른 의존성의 정리.
- 도메인 키워드 지원 범위를 넓히는 일.

Approved: Journey Hwang 2026-11-02T06:38:09Z — "휴가 중 승인 위임(2026-11-01 대화); 0.35 즉시 시작은 사용자 결정(ATIP 세션 경유 2026-11-02)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:7a23decaee66fa44ea0a8f755b854b11e8ea7f4c2db93fa9158eb7f0144e4982)
