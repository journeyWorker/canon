---
kind: chore
---

# atip 크레이트와 검사기를 0.2.0 태그로 올리고 선언은 1.4로 둔다

Source: https://github.com/journeyWorker/atip 태그 v1.5.0(atip-crate-v0.2.0, atip-check-v0.2.0, 커밋 c6c1a1d)의 CHANGELOG 1.5.0 Crates 항목과 approaches/migration.md §10, canon-0-36-atip-dep

## Intent

canon은 atip 크레이트를 태그 `atip-crate-v0.1.3`으로, 검사기를 태그 `atip-check-v0.1.4`로 받는다. 1.5를 선언하려면 1.5를 아는 크레이트와 검사기가 먼저 있어야 한다. 새 크레이트에서는 패턴과 문서를 검사하는 함수가 선언한 minor를 받는다. 새 검사기는 1.5 주장을 판정한다. 이 변경은 두 태그만 올리고 선언은 1.4로 둔다. 선언을 바꾸는 일은 canon-0-37-claim-1-5가 한다.

## Category

dependency

## Invariants

- ==**canon이 1.4를 선언하는 동안 겉으로 보이는 동작은 0.36.0과 한 바이트도 다르지 않다.**== 모든 명령의 stdout과 종료 값, `canon schema`의 모든 답, 체크인된 생성 명세, conformance 기대 출력이 같다.
- canon-cli는 태그 `atip-crate-v0.2.0`의 `atip`를 git 의존으로 받고 잠금 파일이 커밋 c6c1a1d를 고정한다. `cargo build --locked`가 통과하고 오프라인 테스트 job의 순서(먼저 `cargo fetch --locked`, 그 뒤 네트워크 차단)가 그대로다.
- 패턴과 문서를 검사하는 크레이트 함수는 선언 minor를 받고 canon은 1.4를 넘긴다. 1.4 문서의 패턴에 공백 속성 이스케이프가 있으면 문제로 보고된다(계약 15.7).
- CI는 검사기를 태그 `atip-check-v0.2.0`에서 설치하고 `atip check --level L2 --contract 1.4 --deep`을 돌린다. `just atip-install`, `just atip`, `just atip-deep`이 같은 태그와 선언을 쓴다.
- 문서와 설정 주석이 새 태그 이름을 말하고 옛 이름이 남지 않는다.
- 시나리오를 더하지 않는다. 시험은 새 함수 모양으로 옮기고, 1.4 선언에서 공백 속성 패턴이 문제로 나온다는 단언을 기존 시험 하나에 더한다.

## Checks

- ==**`cargo build --locked -p canon-cli`와 `just test`가 통과하고, `canon schema --json`과 `--errors --json`과 `--warnings --json`의 출력 해시, `just conformance`의 차이, 체크인된 생성 명세의 차이가 이 변경 전후로 없다.**== 전의 값은 0.36.0 바이너리에서 잰다. 해시와 차이 없음을 evidence에 남긴다.
- `just atip-deep`이 검사기 0.2.0과 선언 1.4로 통과한다. 보고서 요약과 실행 시간을 evidence에 남긴다.
- 워크스페이스, justfile, 워크플로, 문서 어디에도 옛 태그 이름 `atip-crate-v0.1.3`과 `atip-check-v0.1.4`가 남지 않는다. 검색 결과를 evidence에 남긴다.
- `just lint`가 통과하고 `canon gate check`가 깨끗하다.

## Questions and assumptions

- 선행 조건 충족: 태그 `atip-crate-v0.2.0`, `atip-check-v0.2.0`, `v1.5.0`이 atip 레포의 같은 커밋 c6c1a1d를 가리킨다.
- 가정: 크레이트 0.2.0은 1.4를 선언한 문서를 0.1.3과 같게 쓴다. 계약 1.5.0의 변경 기록이 어떤 문서도 모양이 바뀌지 않는다고 적는다. 이 가정이 틀려 출력이 바뀌면 이 변경은 chore가 아니다. 그 차이를 evidence에 적고 사람에게 알린다.
- 가정: 검사기 0.2.0은 1.4 주장을 0.1.4와 같은 행으로 판정한다. 판정이 달라지면 사람에게 알린다.
- 정책 질문 없음. 이 변경은 canon이 계약에 대해 하는 주장을 바꾸지 않는다.

## Out of scope

- 1.5 선언, 선언 패턴 바꾸기(canon-0-37-claim-1-5).
- 옵션 값 규칙과 시각, 제공자 규칙(canon-0-37-option-values, canon-0-37-option-forms).
- 크레이트 API를 canon에 맞춰 바꾸는 일. 태그의 API에 canon을 맞춘다.
