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

`atip` crate는 `jsonschema`를 기본 의존성으로 끌어들이고, 그 의존성은 크레이트 24개를 더해 도구의 바이너리까지 들어간다. 계약 스키마로 봉투를 검증하는 일과 필드 투영의 검증은 검사기와 시험이 하는 일이고, 도구의 실행 경로는 쓰지 않는다. 의존성을 `validate` 기능 뒤로 옮기고, 기능은 검사기와 시험에서만 켠다. 한 가지 예외 후보가 있다. canon의 인수 도메인 검사(`atip::domain::admits`)가 실행 중에 쓴다.

## Category

dependency

## Invariants

- ==**canon과 `atip check`의 출력은 0.34와 바이트까지 같다.**== 오류 코드, 종료 코드, 보고서, `schema` 답, 생성 문서가 모두 같다.
- 기능 이름은 `validate`이고 기본값은 꺼짐이다. `cargo tree -p atip`에 `jsonschema`가 없다.
- `validate`를 켜면 지금의 공개 함수가 모두 그대로 있다. 봉투와 레지스트리 항목과 스키마 문서의 검증 함수, 필드 투영의 검증, 도메인의 `Compiled`가 해당한다.
- `atip-check`와 `atip`의 시험은 `validate`를 켠다. `canon-cli`의 시험과 디버그 검증도 켠다.
- `canon-cli`의 릴리스 빌드는 `jsonschema`를 링크하지 않는다. 도메인 검사 `admits`가 그 빌드에서 필요하면 기능 없이 같은 답을 내도록 한다. 그렇게 할 수 없는 도메인 키워드가 있으면 구현자가 기록하고 그 키워드만 기능 뒤에 둔다.
- 도메인의 `pattern`은 기능 없이도 `regex` 크레이트로 판정한다. 도메인 판정의 답은 `jsonschema`가 내는 답과 같다.
- 새 의존성을 더하지 않는다. 호출 지점의 소스는 `use` 경로 말고 바뀌지 않는다.

## Checks

- ==**같은 입력으로 `canon`의 모든 명령과 `schema` 답을 0.34 바이너리와 바이트 비교한다.**== `atip check`의 보고도 같다.
- 차분 시험: canon의 도메인 표 전체에서 경계 안과 밖의 값으로 `admits`를 기능 없이 부른 답과 `validate`를 켜고 부른 답이 같다. 이 시험은 `validate`를 켠 빌드에서만 돈다.
- `cargo tree -p atip`에 `jsonschema`가 없고 `cargo tree -p atip --features validate`에는 있다. `cargo tree -p canon-cli -e normal`의 릴리스 의존성에도 `jsonschema`가 없다.
- `cargo test --workspace`와 `just lint`의 clippy 세 줄이 같은 커밋에서 통과한다. 생성 문서 drift 시험과 canon 자신의 `canon gate check`가 통과한다.
- 의존성 크레이트 수의 변화(24개 감소)를 변경 기록에 싣는다.
- evidence: 마지막 코드 커밋 뒤에 context-capability의 시나리오를 다시 돌려 묶는다. 변경 단위 리뷰는 다른 세션의 reviewer가 한다.

## Out of scope

- 새 기능이나 새 API, 검증 규칙의 변경.
- `jsonschema` 버전 올리기와 다른 검증 라이브러리로의 교체.
- `schemars`와 `regex` 등 다른 의존성의 정리.
- 도메인 키워드 지원 범위를 넓히는 일.
