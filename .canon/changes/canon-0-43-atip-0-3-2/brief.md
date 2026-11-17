---
kind: chore
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "의존 두 곳과 설치 경로 전환을 한 변경으로 묶어 태그별 근거, 측정값, 시험 처분 표가 모두 필요하다"
  - rule: "prose.parenthetical"
    section: "checks"
    reason: "괄호는 확인이 읽는 불변식 번호와 기대 값의 출처다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "크레이트, 태그, 패키지, 명령 이름은 이 변경이 바꾸는 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "태그, 파일, 레시피, 명령의 철자가 이 변경이 지켜야 하는 대상 자체다"
  - rule: "prose.code-citation"
    section: "checks"
    reason: "확인은 파일과 명령의 철자를 그대로 말해야 구현자와 리뷰어가 같은 것을 돌린다"
  - rule: "prose.length"
    section: "intent"
    reason: "버전마다 바뀐 것과 설치 경로를 가른 근거가 결정이라 빼면 구현자가 다시 조사해야 한다"
  - rule: "prose.length"
    section: "invariants"
    reason: "의존, 설치, 명령, 시험, 문서를 한 변경으로 묶어 불변식이 모두 필요하다"
  - rule: "prose.length"
    section: "checks"
    reason: "불변식마다 확인이 하나씩 필요하다"
  - rule: "prose.parenthetical"
    section: "intent"
    reason: "괄호는 태그와 값의 출처다"
  - rule: "prose.parenthetical"
    section: "invariants"
    reason: "괄호는 시험 이름과 파일의 예다"
  - rule: "prose.sentence-length"
    section: "invariants"
    reason: "명령의 철자와 파일 열거가 한 문장이다"
  - rule: "prose.sentence-length"
    section: "checks"
    reason: "확인 명령과 기대 결과의 열거가 한 문장이다"
---

# atip 크레이트를 0.3.2로 올리고, CI와 레시피는 미리 빌드된 검사기 atip-check 0.4.1을 잠금 파일로 고정해 `--jobs`로 돌린다

Source: https://github.com/journeyWorker/atip 의 태그 `atip-crate-v0.3.2`(커밋 `31e434c`), `atip-check-v0.4.1`(커밋 `2191855`), `v1.6.1`과 그 `CHANGELOG.md`(Crates 0.3.1~0.4.1, 1.6.1), npm 패키지 `@journeykit/atip-check@0.4.1`, canon 0.42.0 소스(main `5c4f46fab`)의 `Cargo.toml`, `justfile`, `.github/workflows/test.yml`

## Intent

canon은 atip 크레이트를 태그 `atip-crate-v0.3.0`의 git 의존으로, 검사기를 `atip-check-v0.3.0`에서 `cargo install --git … --locked`로 소스 빌드해 쓴다. atip 레포는 그 뒤로 이렇게 움직였다.

| 태그 | 바뀐 것 | canon에 닿는 곳 |
| --- | --- | --- |
| `atip-crate-v0.3.1` | 1.6 문서에서 값 이름이나 값 종류가 다른 정의를 가진 이름은 트리의 공유 `options`에 넣지 않는다(이름마다 그룹 안에서 한 뜻) | `canon schema --json`의 `options` |
| `atip-crate-v0.3.2` | `schema` 답의 비용 개선(`Described::of_models`, 패턴 컴파일 1회, 정의 비교의 복사 제거). 답은 `Described::new`와 바이트 단위로 같다. 크레이트가 crates.io에 게시된다 | 성능만 |
| `atip-check-v0.3.2`, `0.3.3` | `--deep`가 빨라진다(복사본 한 번의 `clonefile`, 변경분만 해시, 종료 대기). `--jobs N`으로 독립 호출을 동시에 돌린다. 보고서는 N과 무관하다 | CI 시간 |
| `atip-check-v0.4.0`, `0.4.1` | 검사기 자신이 1.6 L2다. 보고서와 코드가 현재 스펙의 절을 인용한다. 최소 Rust가 1.100.0이다. `--jobs`는 1 이상만 받는다 | 설치 경로 |
| `v1.6.1` | 스펙 패치. 규칙 변경 없음, `meta.contractVersion`은 `"1.6"` 그대로. 쉼표 없는 문자열 패턴 두 개를 기록한다 | 없음 |

0.3.1의 수정은 canon의 문서를 바꿀 수 있는 유일한 변화다. canon 0.42.0의 `schema` 답을 읽으면 이름 133개 가운데 정의가 둘 이상이면서 값 이름이나 값 종류가 다른 이름이 0개다(트리의 38개 공유 옵션과 81개 명령 문서를 합쳐 읽음). 그래서 이 변경은 chore이고 출력이 바뀌지 않아야 한다. 확인은 전후 `canon schema` 답의 해시다(Checks C1).

검사기는 0.4.0부터 `rust-version = "1.100.0"`이다. canon의 `rust-toolchain.toml`은 1.99.1이고 올리는 일은 owner가 4개 Rust 프로젝트와 함께 정한다(`canon-0-42-ci-refresh` Out of scope). 그래서 `cargo install atip-check@0.4.1`과 `--git … --tag atip-check-v0.4.1`은 canon의 툴체인에서 cargo가 거부한다 [INFERENCE: cargo의 `rust-version` 검사, 이 환경에서는 돌리지 않았다]. 검사기의 npm 패키지는 미리 빌드한 바이너리(macOS arm64, Linux x64와 arm64, 정적)라 툴체인과 무관하다. 소스 설치(`cargo install`)는 1.100 이상의 툴체인에서만 맞는 대안이고, 캐시된 소스 빌드 단계가 CI에서 없어진다.

크레이트 0.3.2는 `rust-version`을 선언하지 않는다(crates.io 메타데이터 `null`을 읽음). 변경 기록은 "다음 릴리스가 `1.99`를 선언한다"고 한다. canon의 1.99.1은 어느 쪽이든 맞는다.

## Category

dependency

## Invariants

- I1 ==**canon의 바이너리 출력은 한 바이트도 바뀌지 않는다.**== `canon schema --json`, 모든 명령의 `schema <명령> --json`과 `--schemas --json`, `schema --errors --json`, `schema --warnings --json`의 답이 전후로 같다. 선언은 1.6 그대로이고(`meta.contractVersion`, `meta.schemaVersion`, 트리의 `level` L2) 픽스처 `canon.fixture.json`의 `contractVersion`도 `1.6`이다. 같지 않으면 이 변경은 chore가 아니다. 구현은 멈추고 차이를 사람에게 알린다(Questions의 가정).
- I2 `Cargo.toml`의 `atip`는 태그 `atip-crate-v0.3.2`의 git 의존이고 `validate` 기능을 지금과 같이 켠다. `Cargo.lock`이 그 태그의 커밋을 고정하며 `cargo build --locked`와 `cargo fetch --locked`가 통과한다. 저장된 규칙 호출표 `crates/canon-cli/tests/atip/rule_calls.jsonl`의 머리와 모든 행의 해시가 `Cargo.lock`의 `atip` 줄을 담으므로, 잠금이 움직인 뒤 `just rule-calls`로 표를 다시 만들어 커밋한다(`arg_rules`와 `atip_check_order`가 머리가 다르면 실패한다). `canon-0-43-test-cleanup`이 이 표의 `include_str!` 해시를 바꾸면 그 변경이 표를 다시 만든다. `Described::of_models`는 쓰지 않고 `crates/canon-cli/src/spec.rs`의 `Described::new` 호출은 그대로다.
- I3 CI의 잡 `atip`와 레시피 `atip-install`, `atip`, `atip-deep`는 검사기를 소스에서 빌드하지 않고 `@journeykit/atip-check` 0.4.1의 미리 빌드한 바이너리를 쓴다. 버전의 출처는 하나다: 저장소의 새 디렉터리 `tools/atip-check/`의 `package.json`과 `bun.lock`이고, 설치는 그 디렉터리에서 `bun install --frozen-lockfile`이다. `bun.lock`이 시작 패키지와 세 플랫폼 패키지의 `sha512` 무결성 값을 담는다. 이것이 이 변경의 "고정과 체크섬"이며, 워크플로와 `justfile`은 버전 숫자를 따로 적지 않는다.
- I4 이 디렉터리는 루트 `package.json`의 `workspaces`(`packages/*`) 밖이고 루트 `bun.lock`은 바뀌지 않는다. `package.json`은 `{"name":"canon-atip-check","version":"<워크스페이스 버전>","private":true,"dependencies":{"@journeykit/atip-check":"0.4.1"}}`다. `version`은 `Cargo.toml`의 `[workspace.package].version`(지금 `0.42.0`)이고, 릴리스 안전 검사의 규칙 6이 모든 `package.json`에 그것을 요구하므로 릴리스 버전 범프가 이 파일도 올린다. `bun.lock`은 CI가 고정한 bun(`1.3.14`)이 쓴 것이고 머리가 `"lockfileVersion": 1`이다(bun 1.4가 쓴 `2`는 1.3.14의 `--frozen-lockfile`이 읽지 못한다).
- I5 검사기는 한 번 설치되고, CI의 검사 단계와 레시피 `atip`, `atip-deep`는 `tools/atip-check/node_modules/.bin/atip`를 경로로 부른다(PATH를 찾지 않는다. 개발자 머신의 `~/.cargo/bin/atip` 0.3.0이 대신 돌지 않는다). 레시피 `atip-install`은 위 설치 명령 하나이고 CI의 설치 단계도 같은 명령이다. CI 잡과 레시피 `atip`, `atip-deep`는 같은 검사 명령을 돌린다.
- I6 검사 명령은 `--jobs 4`를 더하고 값은 CI와 레시피에서 같다(결정, owner). `--contract`는 두 곳에서 모두 뺀다: 검사기는 `--contract`가 없으면 픽스처의 `contractVersion`을 따르고(0.4.1의 `--help`), 선언과 다른 값이 든 `--contract`는 통과해도 `preview`(수준 주장이 아님)로 끝나기 때문이다. 나머지(`--level L2 --deep --fixture …`)는 그대로다. 보고서는 N과 무관하므로 판정과 행 수가 `--jobs 1`과 같다(C3).
- I7 잡 `atip`는 `~/.cargo/bin/atip`의 검사기 캐시 단계와 `cargo install` 단계를 갖지 않는다. 잡의 나머지(러너 `ubuntu-26.04`, `timeout-minutes: 25`, 의존성 캐시 복원과 저장, `cargo fetch --locked`, `cargo build --locked --offline -p canon-cli`)는 그대로다. 이로써 `canon-0-42-ci-refresh`의 I7(검사기 캐시의 `key:` 철자)은 이 변경이 넘겨받아 없앤다.
- I8 외부 액션은 전체 SHA로 고정한다. 잡 `atip`에 더하는 `oven-sh/setup-bun`은 이미 쓰는 SHA와 `bun-version`(`1.3.14`)과 같고, `actions/setup-node`도 이미 쓰는 SHA(`949feb24…`, v7.1.0)와 `node-version: "26"`으로 항상 더한다(npm 시작 파일의 shebang이 `node`이고, 러너 이미지에 기대지 않는다). `scripts/check-release-workflow-safety.py`는 규칙 추가 없이, 예외 추가 없이 새 단계를 받아들인다.
- I9 이 변경은 시험을 새로 더하지 않는다. 문구와 설치 줄과 명령 철자를 읽는 시험은 고치지 않고 없앤다(owner: "제발 바이트레벨 문구 고정같은건 하지말자. 진짜."). 남기는 것은 소비자에게 보이는 데이터의 일치 하나다: 픽스처 `canon.fixture.json`이 canon이 선언한 minor(`DECLARED_MINOR`)를 말한다.

  | 시험 | 처분 |
  | --- | --- |
  | `atip_check_43`(`atip_check.rs`) | 없앤다. 설치 줄을 빼면 남는 것이 CI와 레시피의 명령 문자열이 같다는 철자 비교뿐이다 |
  | `atip_claim14_14`(`atip_claim14.rs`) | 없앤다. 남는 것은 명령 토큰과 `just ci` 의존의 철자와, 검사기가 CI에서 이미 보는 픽스처 내용이다 |
  | `atip_claim15_11`(`atip_claim15.rs`) | 없앤다. 워크플로 텍스트의 `--contract 1.6`을 1.4로 바꿔 읽는 시험이다 |
  | `atip_claiml2_07`(`atip_claiml2.rs`) | 없앤다. 레시피와 잡의 명령 줄을 문자열로 못 박는 시험이다 |
  | `atip_claim15_10`(`atip_claim15.rs`) | 명령 토큰과 태그 줄의 단언을 모두 없애고, 픽스처의 `contractVersion`이 `DECLARED_MINOR`와 같다는 단언과 트리의 `meta.contractVersion` 단언만 남긴다(이름을 그에 맞게 고친다) |
  | `atip_claim16_11`, `atip_claim16_12`(`atip_claim16.rs`) | 없앤다(결정, owner). 옛 태그 문자열과 설치 줄 철자를 읽는 시험이다 |
  | `atip_claim14_13`(`atip_claim14.rs`) | 없앤다. 잡과 두 레시피에서 `--contract`가 한 번씩 읽힌다는 것과 워크플로와 `justfile` 글자를 바꿔 읽는 것뿐이다 |
  | `atip_optionforms_15`(`atip_optionforms.rs`), `atip_optionvalues_16`(`atip_optionvalues.rs`) | `justfile`에 `--contract 1.6`이 있다는 단언 한 줄만 없앤다. 픽스처가 도메인이 거절하는 값을 적었다는 단언은 데이터이므로 남기고, 이름에서 "the recipes run at 1.5"를 뺀다 |

  이 시험들만 쓰던 도우미(`minor_problems`, `contracts_in`, `tokens_of`, `job_block`, `recipe`, `recipe_lines`, `job_commands`, `stale_tag_lines` 등)와 상수(`INSTALL`, `COMMAND`)는 같이 지운다. 다른 시험이 쓰는 도우미는 남긴다.

- I10 스펙의 시나리오는 시험을 따라간다. 없애는 시나리오와 그 subject는 이렇다: `atip-contract`의 `atip.check.43`, `atip.claim14.13`, `atip.claim14.14`, `atip.claim16.11`, `atip.claim16.12`(결정, owner), `atip.claiml2.07`과 `atip-check`의 `atip.claim15.11`. 남는 `atip.optionforms.15`(`atip-schema`)와 `atip.optionvalues.16`(`atip-contract`)은 레시피 문구를 뺀다. 새 시나리오는 없다.
- I10b `atip.claim15.10`은 `@subject:atip-contract`로 옮기고 문구를 "픽스처와 트리가 canon이 선언한 minor를 말한다"로 고친다. 이유는 `atip-check`가 retired이고 그 시나리오 둘이 그 subject의 전부라, 다시 여는 것보다 현재 계약을 말하는 subject로 옮기는 것이 단순하기 때문이다(`@flow` 태그는 그대로). 다시 열어 새 evidence를 기록하고 출시하는 subject는 `atip-contract`와 `atip-schema`다.
- I11 문서가 새 위치를 말한다: `Cargo.toml`의 주석(24~25줄), `README.md`(64~67줄), `ARCHITECTURE.md`(83~93줄), `.github/workflows/test.yml`의 `atip` 잡 위 주석과 `justfile`의 `atip-install` 위 주석, `crates/canon-cli/src/spec.rs`(114~115줄)와 `atip_check.rs`(10~11줄)와 `atip_claiml2.rs`(3~4줄)의 머리 주석(옛 태그와 `--contract 1.6`을 말한다), `CHANGELOG.md`의 `Unreleased` 항목(Changed, 아래 항목). 스펙 v1.6.1은 패치이고 canon의 선언은 1.6이다. 문서는 "1.6.1"을 canon의 선언으로 쓰지 않는다. CHANGELOG는 이전 항목(0.41의 `atip-check-v0.3.0`)을 고치지 않는다.
- I12 CHANGELOG 항목은 전후 표를 갖는다: 크레이트 `atip-crate-v0.3.0`에서 `atip-crate-v0.3.2`, 검사기 소스 설치 `atip-check-v0.3.0`에서 미리 빌드 `0.4.1`, `--jobs`, 잡 `atip`의 시간, 출력 변화 없음(전후 해시). `1.6.1`이 패치라서 선언이 `1.6`에 남는다는 한 줄을 넣는다.

## Checks

- C1(I1) ==**전후 `schema` 답의 해시가 같다.**== 전은 기준 커밋(main `5c4f46fab`)의 빌드이고, 같은 버전(`0.42.0`)이면 PATH의 `canon 0.42.0`과 교차 확인한다. 해시 만드는 법은 이렇다.
  ```bash
  { canon schema --json
    canon schema --json | jq -r '.data.commands[]|select(.kind!="group")|.command' | while read -r c; do canon schema $c --json; canon schema $c --schemas --json; done
    canon schema --errors --json; canon schema --warnings --json; } | sha256sum
  ```
  0.42.0에서 이 명령은 165줄을 내고 해시는 `ac14c89211aeef2cc87eb8273a5d66f99ce5006d8c70d9663265d8581249c061`이다(두 번 돌려 같음을 확인했다). 후의 해시가 같다. 다르면 구현은 멈추고 차이(`diff`)와 함께 사람에게 알린다. 해시를 evidence에 남긴다. `cargo test -p canon-cli --test atip_check`(픽스처가 `schema --errors`와 `--warnings`와 같음)와 `just conformance`, `docs/cli/canon.cli.json`의 차이가 없다.
- C2(I2) `cargo fetch --locked`와 `cargo build --locked -p canon-cli`가 통과한다. `Cargo.lock`의 `atip` 항목이 `?tag=atip-crate-v0.3.2#<커밋>`이고 `Cargo.lock`의 변화가 그 항목(과 새 의존이 있다면 그것)뿐이다. `just rule-calls` 뒤 `just rule-calls-check`가 깨끗한 트리에서 통과하고 표의 차이는 머리의 `atip` 줄과 행의 해시뿐이다. 찾은 호출(`found`)이 바뀌면 크레이트의 규칙 동작이 바뀐 것이므로 evidence에 적고 사람에게 알린다. [INFERENCE: 태그 사이에 크레이트의 의존성 목록은 바뀌지 않았다, `atip` 레포의 두 태그 사이 `Cargo.lock` 차이가 `atip`의 두 줄뿐이었다]
- C3(I3, I5, I6) 샌드박스에서 `just atip-install`, `just atip-deep`가 통과한다. 레시피가 부르는 `atip --version`이 `~/.cargo/bin/atip` 0.3.0이 있어도 `atip 0.4.1`이다. 보고서가 `L2 met`이고 MUST 실패 0이다(0.41 기준: `74 MUST passed, 0 MUST failed, 0 MUST unprovided, 8 not applicable, 1 SHOULD advisories`, 권고는 `L1.one-meaning-values`의 `--kind`). 검사기 0.4.1이 새 행이나 새 권고를 내면 evidence에 적고, MUST 실패면 멈추고 사람에게 알린다 [INFERENCE: 0.3.0에서 0.4.1까지의 행 변화를 이 계획이 대조하지 않았다]. 같은 실행을 `--jobs 1`로 돌려 판정과 행 수가 같음을 evidence에 남긴다.
- C4(I3, I4) `tools/atip-check/bun.lock`의 머리가 `"lockfileVersion": 1`이고 네 패키지(`@journeykit/atip-check`와 `-darwin-arm64`, `-linux-arm64`, `-linux-x64`)가 `0.4.1`과 `sha512`로 있다. 값이 `npm view @journeykit/atip-check@0.4.1 dist.integrity`와 같다(시작 패키지는 `sha512-krmGHaR9…xcplUQ==`). 값을 바꾼 복사본의 `bun install --frozen-lockfile`은 빈 캐시에서 `Integrity check failed`로 실패한다(잡은 bun 캐시를 복원하지 않는다). 락이 없는 경우는 bun이 막지 않으므로 커밋된 파일의 리뷰가 지킨다. 루트 `git diff -- bun.lock`이 비어 있다.
- C5(I5, I7, I11) PR의 CI에서 잡 `atip`가 통과한다. 잡 로그에서 검사기가 `atip 0.4.1`이고(`atip --version`), `cargo install` 줄과 `Cache the checker` 단계가 없다. `rg "atip-check-v0|cargo install.*atip|--contract 1\.6" .github justfile README.md ARCHITECTURE.md Cargo.toml crates`가 CHANGELOG와 소비자 문서를 훑는 시험의 부정 사례 문자열(`atip_claim14`, `atip_claim15`) 밖에서 비어 있다.
- C6(I8) `just release-check`가 통과한다(규칙 6이 `tools/atip-check/package.json`의 `version`을 본다). `setup-bun`과 `setup-node` 단계의 SHA와 버전이 다른 잡과 같고, 로그의 `atip --version`이 `node`로 돌았다는 증거다.
- C7(I9, I10) `cargo test -p canon-cli --test atip_check --test atip_claim14 --test atip_claim15 --test atip_claim16 --test atip_claiml2 --test atip_optionforms --test atip_optionvalues`가 통과한다. 없앤 시험의 이름이 코드와 스펙 어디에도 없고(`rg "atip_check_43|atip_claim14_1[34]|atip_claim15_11|atip_claim16_1[12]|atip_claiml2_07" crates .canon/specs`가 비어 있다), `canon inventory sync` 뒤 `canon gate check`가 위반 0이다. 남은 시험 어느 것도 `justfile`과 `.github/workflows/test.yml`의 글자를 읽어 태그, 설치 줄, 옵션 철자를 단언하지 않는다. 소비자 문서에서 낮은 minor를 찾는 `consumer_problems` 검사는 남는다. 지운 도우미가 쓰이던 곳이 없다(컴파일 경고 0).
- C8(I11, I12) `README.md`, `ARCHITECTURE.md`, `Cargo.toml`, 워크플로, `justfile`의 태그 철자가 새 값이다. CHANGELOG에 전후 표와 한 줄 설명이 있다. 잡 `atip`의 시간은 PR 실행과 기준선(`canon-0-42-ci-refresh`가 잰 최대 7.7분)을 표로 비교한다.
- C9(전체) `just lint`, `canon gate check`와 이 PR의 마지막 커밋에서 모든 CI 잡이 통과한다. 변경 단위 리뷰는 다른 세션의 reviewer가 한다.

## Questions and assumptions

- 가정: 0.3.1의 수정이 canon의 문서를 바꾸지 않는다(위 Intent의 읽기: 값 이름이나 값 종류가 다른 이름 0개). 틀리면 이 변경은 chore가 아니다. 구현은 C1의 차이를 evidence에 적고 사람에게 알린다(변경 종류와 CHANGELOG가 달라진다).
- 가정: 검사기 0.4.1이 canon 0.42의 L2 1.6 주장을 같은 행으로 판정한다. 0.4.x의 바뀐 것은 검사기 자신의 선언, 코드 이름, clap 파싱, `--jobs` 범위와 보고서의 절 인용이다. 위 C3이 확인한다.
- 결정(owner, 2026-11-17): `--jobs 4`를 CI와 레시피 모두에 쓴다.
- 결정(owner, 2026-11-17): 시나리오 `atip.claim16.11`과 `atip.claim16.12`를 없앤다. 게이트가 보는 시나리오 집합이 둘 줄어든다.
- 결정(owner, 2026-11-17): 크레이트는 crates.io로 옮기지 않고 태그 `atip-crate-v0.3.2`의 git 의존을 유지한다(0.3.2가 crates.io에 게시돼 있음은 읽었다). 옮기는 일은 후속 변경으로 따로 정한다.
- 결정(owner, 2026-11-17): 검사기 설치는 `bun install --frozen-lockfile`의 전용 디렉터리다. 락 없는 `bunx`는 쓰지 않는다.
- 결정(owner, 2026-11-17): `cargo install atip-check`는 문서에 대안으로 적지 않는다. canon의 툴체인(1.99.1)에서는 되지 않는다.
- 질문 6(이 계획이 더한 것): 위 I6의 `--contract` 삭제. 기본: ==**CI와 레시피에서 `--contract`를 뺀다.**== 지금은 선언을 바꿀 때 명령 줄의 `--contract 1.6`도 따로 고쳐야 하고, 그 일치를 지키려고 워크플로 글자를 읽는 시험(위에서 없애는 시험들)이 있었다. 빼면 일치는 픽스처의 한 값이고 시험은 데이터 하나다. 대안은 `--contract 1.6`을 두고 일치 시험도 두지 않는 것이다. 그러면 낮은 값이 남아도 검사는 통과로 끝날 수 있다.
- 결정: 크레이트의 `rust-version`은 canon에 영향이 없다. 계획은 이를 CHANGELOG에 한 줄로만 적는다(0.3.2는 선언하지 않고 다음 릴리스가 `1.99`).
- 병합 순서: 같은 파일(`test.yml`의 `atip` 잡, `justfile`, `CHANGELOG.md`)을 만지는 다른 0.43 변경이 있으면 이 변경이 구현할 때 그 위로 옮긴다. 줄 단위 충돌뿐이고 의미가 겹치지 않는다 [INFERENCE: 다른 0.43 brief를 읽지 않았다].

## Out of scope

- `Described::of_models`의 도입(owner가 나중에 정한다).
- 선언을 1.6에서 올리거나 바꾸는 일. 스펙 v1.6.1은 패치이고 `contractVersion`은 `1.6`이다.
- canon의 `rust-toolchain.toml` 올리기와 `cargo install`로의 검사기 설치.
- canon의 코드, 출력, 레저, 픽스처의 변경. 검사기가 새로 지적하는 것을 고치는 일(C3에서 나오면 별도 변경).
- 크레이트의 crates.io 전환(질문 3).
- 검사기의 동작 변경. 태그가 가리키는 검사기를 그대로 쓴다.

Approved: journey 2026-11-17T09:50:53Z — "오너가 채팅에서 오케스트레이터에게 0.43 승인을 위임함 (2026-11-17 ask: '응'). 독립 계획 리뷰 2라운드 approve(1라운드 P1 2건 수정 확인). 오너는 브리프 본문을 읽지 않음." (relayed by planner planner-oco via chat) (code tree 0c6fb013d928439f3157a3b1ad6c0ef35fe8d5eb) (brief sha256:0f0cd773bbbe211656fe04036f450a75259abf41a5f8b13ceebdb066ae760e02)
