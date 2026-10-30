---
kind: chore
---

# crates/atip 추출: 오류·봉투 출력·argv 판정을 공유 crate로

Source: https://github.com/journeyWorker/atip (atip/1 1.0), approaches/rust.md §3, approaches/migration.md §3.1 1번

## Intent

canon의 오류 값과 오류 코드 행 타입, JSON 모드 판정, 한 줄 JSON 출력, 다음 단계 항목의 셸 인용은 모두 `canon-cli` 안에만 있다. `canon-cli`는 clap 트리와 canon의 모든 crate에 의존하므로, 같은 규칙을 쓰려는 다른 도구(lute)는 이 코드를 가져올 수 없다. atip/1 계약을 구현하는 공유 crate `atip`은 canon 워크스페이스 안에서 먼저 만들고 나중에 atip 레포로 옮긴다. 같은 릴리스의 `canon-0-33-atip-l0`가 이 코드를 바꾸므로, 그 전에 자리만 옮겨 둔다.

## Category

refactor

## Invariants

- ==**사용자가 보는 출력은 0.32.0과 바이트 단위로 같다.**== 대상은 모든 명령의 텍스트와 `--json` 출력, 종료 코드, 명령 명세 파일, 오류 레지스트리 출력, conformance 케이스 출력이다. 레저 레코드의 바이트와 이름도 같다.
- 새 crate `atip`은 워크스페이스 멤버이고 `canon-*` crate에 의존하지 않는다. atip 레포로 옮길 때 소스 수정이 필요 없다.
- crate로 옮기는 것은 네 가지다.
  - 오류 개체와 그 빌더, 비밀 값을 가리는 `redact`.
  - 오류 코드 행의 타입과 종료 값 상수.
  - JSON 모드 판정과 봉투를 한 줄로 쓰는 출력 함수.
  - 다음 단계 항목과 `command` 문자열의 셸 인용·분해 규칙.
- `canon-cli`에 남는 것은 봉투의 `Meta` 구조체, canon의 코드 행 176개, clap 오류 변환, hint 문구다. 이들은 `canon-0-33-atip-l0`가 바꾼다.
- 옮긴 코드의 단위 테스트는 같이 옮기고 개수가 줄지 않는다. `canon-cli`에 같은 기능의 복제가 남지 않는다.
- 새 의존성은 워크스페이스에 이미 있는 crate로 한정한다. 워크스페이스 lint와 0.30 Rust 규칙을 따른다.

## Checks

- ==**각 단계 커밋마다 0.32.0 바이너리로 만든 기준과 바이트 비교한다.**== 기준은 canon 코퍼스 사본에 대한 `canon status --json`, `canon gate check --json`, `canon schema --errors --json`, conformance 케이스 출력, 명령 명세 파일이다. 버전 값은 비교에서 뺀다.
- `cargo build -p atip`이 `canon-*` crate 없이 단독으로 빌드되고, `cargo tree -p atip`에 `canon-*`이 없다.
- `cargo test --workspace`, `just lint`의 clippy 세 줄, 생성 문서 drift 테스트(`just docs` 후 diff 없음), canon 자기 `canon gate check`가 같은 커밋에서 통과한다.
- evidence: 마지막 코드 커밋 뒤에 context-capability의 시나리오를 다시 돌려 `canon evidence add --all-scenarios`로 묶는다. 변경 단위 리뷰는 다른 세션의 reviewer가 한다.

## Out of scope

- 봉투 모양, 오류 코드 이름, 종료 코드 규칙의 변경(`canon-0-33-atip-l0`).
- 예약 코드 표, 계약 스키마 파일의 반입, clap 오류 매핑 모듈. L0 변경이 필요한 만큼 만든다.
- `spec`, `schema`, `input`, `safety`, `stream`, `skills` 모듈의 추출. L1(0.34)과 L2(0.35)에서 한다.
- `atip check` 검사기(`canon-0-33-atip-check`).
- crates.io 게시, atip 레포로의 이동, lute 전환.
