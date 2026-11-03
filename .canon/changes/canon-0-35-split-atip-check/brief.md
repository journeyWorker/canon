---
kind: chore
---

# atip check를 canon 테스트에서 분리하고 canon 준수는 CI 한 번으로

## Intent

canon 전체 테스트는 실행 시간 합계가 846분이고, 그중 약 76%를 테스트 100개가 차지한다. 그 100개는 대부분 `atip check`를 처음부터 끝까지 돌리는 테스트다. 이런 테스트는 두 종류다.

- 체커를 증명하는 테스트: 가짜 도구에 위반을 하나씩 심고, 위반마다 체커 전체를 다시 돌린다. 이 체커 크레이트가 canon 워크스페이스 안에 있어서, canon 테스트를 돌릴 때마다 함께 돈다.
- canon의 L1/L2 준수를 확인하는 테스트: 7~8개가 각자 같은 검사를 처음부터 다시 돌린다. CI의 `atip-l0`/`atip-l2` job도 같은 검사를 한다.

canon과 atip은 별개다. canon이 계약을 지키는지는 CI에서 `atip check`를 한 번 돌리면 확인된다. 체커 자체의 테스트는 체커 크레이트에서만 돌면 된다.

## Category

build

## Invariants

- ==**canon의 기본 테스트 실행에는 체커 크레이트의 테스트가 들어가지 않는다.**== 기본 실행은 `cargo test --workspace`, nextest 기본 프로필, `just test`를 말한다. 체커 크레이트 테스트는 별도 레시피 `just test-atip`과 별도 CI job에서만 돈다.
- ==**canon-cli 테스트 안에서 `atip check` 전체 실행은 테스트 바이너리마다 많아야 한 번이다.**== L1/L2 준수를 보는 테스트들은 그 한 번의 보고서를 나눠 읽는다. 시나리오 id와 그 의미는 그대로 둔다.
- CI는 canon에 대해 `atip check`를 L0과 L2로 한 번씩만 돌린다. 판정 결과는 지금과 같다.
- canon과 체커의 동작, 출력, 레지스트리는 바뀌지 않는다. 바뀌는 것은 테스트 구성, CI, justfile, 문서뿐이다.
- 체커의 무거운 검사(선언과 동작의 일치)를 `--deep` 같은 옵션 뒤로 옮기는 일은 이 변경에 들어가지 않는다.

## Checks

- ==**`just test`가 체커 크레이트 테스트 없이 돌고, 실행 시간 합계가 지금보다 크게 줄어든다.**== 전후 수치를 evidence에 남긴다.
- `just test-atip`과 해당 CI job에서 체커 크레이트 테스트가 그대로 통과한다.
- canon-cli의 atip 준수 테스트가 바이너리마다 `atip check` 실행 한 번으로 통과한다. 실행 횟수는 evidence에 남긴다.
- CI 워크플로가 모든 테스트를 어느 한 job에서는 돌린다. 빠지는 테스트가 없다.

## Out of scope

- 체커 크레이트를 atip 레포로 옮기는 일.
- `atip check`의 기본 검사를 가볍게 줄이고 `--deep`을 도입하는 일. 체커 동작 변경이라 별도 변경으로 한다.
- 체커 테스트 개수를 줄이는 일.

## Questions and assumptions

- 사용자 결정(2026-11-03 대화): canon 준수는 `atip check` 한 번으로 확인하고, 체커와 canon 테스트는 분리한다.

Approved: Journey Hwang 2026-11-03T10:11:37Z — "분리좀 하자 (2026-11-03 대화)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:987a4cb77aed953377499ca479a430e8c3181b7b426e2a95bd848ac5892dee91)
