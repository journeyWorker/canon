---
kind: chore
waive:
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "크레이트, 태그, 명령 이름은 이 변경이 다루는 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "checks"
    reason: "명령과 태그 이름은 검증 대상의 철자 자체다"
  - rule: "prose.length"
    section: "invariants"
    reason: "의존 전환은 워크스페이스, CI, 테스트, 시나리오, 문서를 함께 정해야 해서 줄이면 결정이 빠진다"
---

# atip 크레이트와 검사기를 atip 레포의 의존으로 바꾸고 CI는 atip check --deep을 돌린다

Source: https://github.com/journeyWorker/atip PR #1(이동), PR #2(`--deep`), PR #3(1.3과 1.4), canon issue #134의 이동 결정과 atip 레포 규칙 댓글

## Intent

canon 워크스페이스는 `atip` 크레이트, 검사기 `atip-check`(가짜 도구와 함께), `atip-probe`를 직접 갖고 있다. 세 크레이트는 처음부터 canon에 의존하지 않게 써 두었고, 같은 코드가 atip 레포에 이미 올라가 있다. 두 곳에 같은 코드가 있으면 계약이 바뀔 때마다 한 곳이 뒤처진다. atip 레포의 PR #3은 1.3과 1.4를 크레이트와 검사기에 더한다. canon은 그 버전을 태그로 받아 쓰고, 검사기의 테스트는 canon 테스트 실행에서 빠져야 한다.

## Category

dependency

## Invariants

- ==**1.2를 선언하는 canon의 겉으로 보이는 동작은 한 바이트도 바뀌지 않는다.**== 모든 명령의 stdout과 종료 값이 0.35.0과 같다. `canon schema`의 모든 답, 체크인된 생성 명세, conformance 기대 출력도 같다. 1.4 선언은 이 변경이 아니다.
- 워크스페이스에 `atip`, `atip-check`, `atip-probe` 크레이트와 가짜 도구가 없다. canon-cli는 `atip`를 태그 `atip-crate-v0.1.0`의 git 의존으로 받고, `validate` 기능은 지금과 같게 켠다.
- 잠금 파일이 태그가 가리키는 커밋을 고정한다. `cargo build --locked`가 통과한다. 오프라인 테스트 job은 먼저 `cargo fetch --locked`를 하고 그 뒤 네트워크를 막는 순서로 그대로 동작한다.
- CI는 검사기를 태그 `atip-check-v0.1.0`에서 설치해 canon 바이너리에 `atip check --deep`을 돌린다. 기본 실행의 통과는 수준 주장이 아니다. 수준은 L2이고, 계약 선언은 1.2이다. L2 보고서가 L0과 L1 행을 모두 담으므로 job은 하나다.
- 검사기를 증명하던 설정이 canon에서 없어진다: `just test-atip`, CI의 `atip-check-tests` job, nextest의 `atip` 프로필과 기본 프로필의 제외 필터, `just test`의 `--exclude atip-check`.
- canon-cli 테스트 중 검사기 자체를 시험하는 것은 없어진다. canon의 코드를 시험하는 것은 남는다. 구현자가 테스트마다 감사표에 처분을 적는다. 처분은 남김, 검사기를 시험해서 없앰, canon에 검사기를 돌려 보는 것이라 CI job에 넘기고 없앰 중 하나이다.
- canon 자신의 준수 확인은 남는다: 픽스처 `canon.fixture.json`의 레지스트리와 경고 목록이 `canon schema`가 내는 것과 같다는 시험과 CI job.
- 시나리오: 검사기와 크레이트의 동작만 말하는 시나리오는 canon의 스펙에서 없어진다. canon의 동작을 말하는 시나리오는 남고 증거가 남은 테스트에 묶인다. 감사표가 시나리오마다 처분을 적는다. 처분은 남김, 없앰, 다른 테스트로 다시 묶음 중 하나이다.
- 새 시나리오를 더하지 않는다. 문서(README, ARCHITECTURE, CHANGELOG, 설계 문서)가 크레이트의 새 위치를 말한다.

## Checks

- ==**`cargo build --locked -p canon-cli`와 `just test`가 통과하고, `canon schema --json`과 `--errors --json`과 `--warnings --json`의 출력 해시, `just conformance`의 차이, `docs/cli/canon.cli.json`의 차이가 이 변경 전후로 없다.**== 해시와 차이 없음을 evidence에 남긴다.
- `just test`의 실행 시간 합계와 테스트 수를 전후로 남긴다. 없어진 테스트의 수가 감사표와 맞는다.
- CI의 canon 준수 job이 태그의 검사기로 `atip check --level L2 --contract 1.2 --deep`을 통과한다. 결과 요약을 evidence에 남긴다.
- 워크스페이스, justfile, 워크플로, nextest 설정, 문서 어디에도 옮겨진 크레이트의 경로와 `test-atip`, `atip-check-tests`가 남지 않는다. 검색 결과를 evidence에 남긴다.
- 감사표의 모든 시나리오 처분이 실제 스펙과 맞고 `canon inventory sync` 뒤 `canon gate check`가 깨끗하다.
- `just lint`가 통과한다.

## Questions and assumptions

- 선행 조건: atip PR #3이 병합되고 태그 `atip-crate-v0.1.0`과 `atip-check-v0.1.0`이 있어야 시작한다. 태그가 PR #3의 병합 커밋을 가리키는지는 계약 저자에게 물었다. 이 변경은 태그가 없으면 시작하지 않는다.
- 가정: `atip-probe`(기본 기능의 도구가 `jsonschema`를 링크하지 않음을 `cargo tree`로 증명)는 크레이트의 속성을 시험하므로 atip 레포로 간다. atip 레포에는 아직 없어서 저자에게 물었다. 그때까지 canon에서는 없앤다.
- 가정: 검사기는 `cargo install --git … --tag atip-check-v0.1.0 atip-check --locked`로 설치하고 캐시한다. 미리 빌드된 바이너리가 있으면 그쪽으로 바꾼다. 저자에게 물었다.
- 가정: L0 job과 L2 job을 L2 job 하나로 합친다. L2 보고서가 L0과 L1 행을 모두 담기 때문이다. 판정은 지금과 같다.
- 가정: 태그의 크레이트는 1.3 이하를 선언하는 문서를 이전 버전과 같게 쓴다(PR #3의 보장). 이 가정이 틀려 출력이 바뀌면 이 변경은 chore가 아니다. 그 차이를 evidence에 적고 사람에게 알린다.
- 정책 질문: 검사기 시나리오 약 207개와 크레이트 시나리오 약 43개가 canon의 스펙에서 없어진다. 게이트가 평가하는 시나리오 집합이 줄어든다. 사람이 감사표를 보고 승인한다. 주제 `atip-check`를 이후에 `retired`로 옮길지는 오케스트레이터와 사람이 정한다.

## Out of scope

- 1.4 선언과 자기 설명 형식(`canon-0-36-claim-1-4`).
- 검사기의 동작 변경. 태그가 가리키는 검사기를 그대로 쓴다.
- crates.io 게시.
- 크레이트 API를 canon에 맞춰 바꾸는 일. 태그의 API로 canon을 맞춘다.
