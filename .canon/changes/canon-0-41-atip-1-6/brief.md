---
waive:
  - rule: "prose.code-citation"
    section: "intent"
    reason: "옵션 이름, 규칙 이름, 검사기 행 이름은 호출자와 검사기가 읽는 철자 자체이고 측정값의 대상이다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "선언 멤버와 태그 이름은 호출자가 읽는 철자 자체이다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "옵션 철자와 선언 멤버 이름은 호출자가 읽는 철자 자체이다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "옵션 이름, 검사기 행 이름, 선언 멤버, 환경 변수는 검증 대상의 철자 자체이다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "계약 멤버와 옵션 이름은 소유자에게 묻는 대상의 철자 자체다"
  - rule: "prose.length"
    section: "brief"
    reason: "선언, 의존 둘, 선택 멤버 판정, 한 이름 한 뜻, CI, 소비자, 변경 기록을 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "intent"
    reason: "계약이 바뀌는 곳마다 canon의 측정값을 함께 적어야 현재 동작이 빠지지 않는다"
  - rule: "prose.length"
    section: "outcome"
    reason: "결과 다섯 줄이 각각 하나의 닫힘 조건(선언, 불변식, 의존, 호출자, 소비자)이다"
  - rule: "prose.length"
    section: "detail"
    reason: "단계마다 옵션별 판정과 거절한 설계를 담아야 하는 변경이다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "일곱 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.length"
    section: "questions"
    reason: "결정과 소유자 질문이 서로 다른 선택이라 함께 적어야 한다"
---

# canon이 atip/1 1.6을 선언하고 크레이트와 검사기를 0.3.0으로 올린다

Source: https://github.com/journeyWorker/atip 태그 v1.6.0(atip-crate-v0.3.0, atip-check-v0.3.0, 커밋 9cbc74a)의 CHANGELOG 1.6.0, spec 4.1.1, 9.7.6, 15.1, 15.7, 15.10, annex-schema.md S1 항목 4와 S2 규칙 18·19, approaches/checker.md의 L1.comma-list, L1.depth, L1.one-meaning, L1.one-meaning-values, L2.input-refusals, `--contract` 미리보기, approaches/migration.md §11, canon-0-37-claim-1-5, canon-0-37-atip-dep

## Intent

canon은 `meta.contractVersion` 1.5와 L2를 선언한다. 크레이트는 태그 `atip-crate-v0.2.0`, 검사기는 `atip-check-v0.2.0`이다. 계약 1.6.0은 1.6을 선언한 문서에만 묶이는 선택 멤버 둘을 더한다. 반복 옵션의 `commaList`와 트리의 `pathShape`이다. 옵션 이름의 한 뜻은 도구 전체에서 그룹별로 좁아지고 경로가 두 단어를 넘지 않는다는 권고는 철회된다. 1.6을 선언하려면 1.6을 아는 크레이트와 검사기가 먼저 있어야 한다.

0.40.0 바이너리의 `canon schema --json`으로 잰 canon의 모양이다.

- 명령 문서 78개, 그룹 27개, 트리 항목 105개. 한 단어 경로가 42개이고 그중 27개가 그룹이며 15개가 명령이다. 두 단어 경로는 63개이고 세 단어 이상은 없다.
- 반복 옵션은 이름 8개다: `--artifact`, `--digest`, `--harness`, `--kind`, `--report`, `--report-case`, `--scenario`, `--surface-ref`. `--harness`는 `init`, `skills install`, `skills check`, `skills doctor`가 쓰고 `--artifact`는 명령 4개, `--report`, `--report-case`, `--surface-ref`는 명령 2개가 쓴다. 나머지는 명령 하나씩이다.
- 옵션 값에서 쉼표를 쪼개는 곳이 없다. `--harness claude,omp`와 `--kind change,task`는 `USAGE`로 값 전체가 거절되고 `received`가 쉼표 문자열 그대로이다. `--input`의 `{"harness":["claude,omp"]}`는 `INPUT_INVALID`(`/harness/0`)이다. 쉼표를 쪼개는 코드는 옵션 값이 아닌 두 곳뿐이다: 지워진 `--providers` 철자를 `--harness` 줄로 고쳐 주는 힌트와 예약 옵션 `--fields`의 경로 목록이다.
- 한 이름이 둘 이상의 `valueName`이나 `valueType`을 가진 옵션이 도구 전체에서 0개이다(공유 옵션 35개 포함). 1.5의 도구 전체 규칙을 이미 지키므로 1.6의 그룹별 규칙도 지킨다.
- `atip-deep`의 SHOULD 권고 하나(0.37부터)는 `L1.one-meaning-values`이다. `--kind`가 `gate promote`(evidence, finding, divergence)와 `purge`(run, trajectory, session, event)에서 겹치는 값이 없다. 두 옵션의 값 이름이 같은 `KIND`라서 1.6의 그룹별 비교에서도 비교 대상이다.
- 검사기 0.3.0으로 0.40.0 바이너리(선언 1.5)를 `--contract 1.6 --deep`으로 미리보기한 결과(9분, 바쁜 기계): 94개 행 중 84개 통과, 실패 2개, 해당 없음 8개이다. 실패는 `L0.contract-version`(MUST)과 `L1.one-meaning-values`(SHOULD)이다. 첫째는 바이너리가 1.5를 선언해서 생겼고 선언을 올리면 풀린다. 둘째는 위의 `--kind`이다. `L1.depth`, `L1.one-meaning`, `L1.paging`은 통과이고 `L1.comma-list`는 `commaList`를 선언한 옵션이 없어 해당 없음이다. 나머지 MUST 해당 없음은 canon이 제공하지 않는 기능(`L2.async`, `L2.operation-key`, `L2.stream`)과 `L2.input-env`이다. 미리보기는 `LEVEL_FAILED`와 종료 값 1을 냈다.

## Desired outcome

- 모든 봉투와 `schema` 답이 `contractVersion` `"1.6"`을 말하고 `schema` 답의 `schemaVersion`도 `"1.6"`이며 트리의 `level`은 `L2`이다.
- ==**`canon schema`의 모든 답은 0.40.0과 비교해 선언 문자열(`contractVersion`, `schemaVersion`, `toolVersion`)만 다르다.**== `commaList`와 `pathShape`이 없고 옵션 이름과 값 이름이 그대로이다.
- `canon-cli`가 `atip-crate-v0.3.0`을 받고 CI, `just atip-install`, `just atip`, `just atip-deep`이 `atip-check-v0.3.0`과 `--contract 1.6`을 쓴다. 픽스처가 1.6을 선언한다.
- 1.5 호출자는 바꿀 것이 없다. 호출자에게 Breaking이 아니다.
- 안내, 웹사이트, conformance, 생성 명세, 시험, CHANGELOG가 1.6을 말한다.
- 선택 멤버를 쓰지 않는 결정과 이유는 CHANGELOG에 있다.

## User flow

1. F1 에이전트는 아무 명령이나 `canon schema --json`을 부른다 → 봉투와 `schema` 답이 1.6을 선언하고 트리의 `level`이 `L2`이다 → ==**canon의 문서를 1.6 규칙으로 읽는다.**==
2. F2 1.5를 선언한 호출자가 canon 0.41을 부른다 → 문서의 모양이 같고 선언 문자열만 바뀐다 → 호출자를 바꾸지 않고 쓴다.
3. F3 에이전트가 반복 옵션에 쉼표 목록(`--harness claude,omp`)을 준다 → canon은 `commaList`를 선언하지 않았으므로 0.40처럼 값 전체를 `USAGE`로 거절한다 → 값마다 옵션을 한 번씩 쓴다.
4. F4 에이전트가 트리에서 경로를 읽는다 → 모든 경로가 두 단어 이하이고 `pathShape`이 없다 → 경로를 명령 이름 그대로 쓴다.
5. F5 검사기 0.3.0이 옵션 이름을 그룹별로 대조한다 → 위반이 없고 권고가 하나이다 → 이름 하나가 한 뜻이라는 것을 문서로 확인한다.
6. F6 릴리스 담당은 CI를 본다 → `atip check`가 검사기 0.3.0, 1.6 L2 `--deep`으로 통과한다 → 선언이 증명되었다.
7. F7 에이전트나 사람은 안내, 웹사이트, CHANGELOG를 읽는다 → 1.6 선언, 선택 멤버를 쓰지 않는 결정, 호출자 노트가 있다 → 호출자를 옮기지 않는다.

## Step detail

### F1 선언과 의존

- 선언 상수가 `"1.6"`이고 모든 봉투, 거절, `schema` 답, 생성 명세의 `contractVersion`이 그 값을 말한다. `schema` 답과 레지스트리 답의 `schemaVersion`이 선언을 따른다. 수준은 L2이다.
- 크레이트의 `CONTRACT_VERSION`은 "1.6"이 된다. canon의 선언은 canon 자신의 상수이고 크레이트의 값이 아니다.
- 크레이트 0.3.0에서 `OptionDoc`에 공개 필드 `comma_list`가 생겨 구조체 리터럴이 깨진다. canon의 리터럴은 시험에만 있다: 완전한 리터럴 4곳과 `..option`으로 이어받는 1곳이다. 이어받는 곳은 영향이 없다. `TreeDoc`은 필드가 늘지 않고 `pathShape`은 `Described::path_shape`가 받으므로 트리 생성은 바뀌지 않는다.
- 크레이트의 나머지 공개 API 추가(`Minor::V1_6`, `Minor::ALL`의 일곱째 값, `describe::Error`와 `project::Problem`의 새 변형)는 canon에 짝이 되는 코드가 없다. `Cargo.lock`이 태그와 함께 움직이고 CI가 `--locked`를 쓰므로 잠금 파일도 올린다.
- 투영이 clap의 `value_delimiter`를 읽어 `commaList`로 옮긴다. canon에는 `value_delimiter`가 0곳이라 `Problem::Delimiter`도, 쉼표 목록을 1.6 미만에서 거절하는 `Described::new`의 오류도 일어나지 않는다. canon이 `value_delimiter(',')`를 더하면 `commaList`가 문서에 저절로 나온다. CHANGELOG에 한 줄 적는다.
- 일하는 순서는 둘이다.
  - 먼저 상수를 1.5로 둔 채 크레이트만 0.3.0으로 올린다. 이 단계에서 바뀌는 시험은 `atip_crate12`의 두 곳뿐이다.
  - 모르는 minor의 시료가 1.6에서 1.7이 되고 알려진 minor 목록이 1.6을 더해 일곱이 된다.
  - 그 단계의 `canon schema --json`, `--errors`, `--warnings`, `--schemas`와 모든 명령 문서의 출력이 부모 커밋의 빌드와 같은 해시임을 남긴다.
  - 그 뒤 상수를 1.6으로 바꾼다. 크레이트 탓의 변화와 선언 탓의 변화가 섞이지 않는다.

### F2 1.5 호출자

- 계약 15.10: 1.5 이하 호출자는 두 멤버를 무시하고 쉼표 목록의 값을 occurrence마다 하나씩 준다. 모든 1.6 도구가 이를 받는다.
- canon은 두 멤버를 쓰지 않는다. 호출자가 보는 문서는 선언 문자열만 다르다. 쉼표 목록 옵션이 없으므로 도메인을 검사하는 호출자도, 검사하지 않는 호출자도 값이 다르게 읽히지 않는다.
- "1.5"를 문자열로 비교하는 호출자는 "1.6"을 읽는다. 0.37이 1.4에서 1.5로 옮길 때도 같은 노트를 달았다. CHANGELOG 항목은 Changed에 둔다.
- 0.40.0 바이너리와 새 빌드의 비교는 일회성 evidence이다. 모든 트리, 그룹, 명령 답과 `--schemas`, `--errors`, `--warnings` 답에서 값이 선언 minor와 같은 `contractVersion`, `toolVersion`, `schemaVersion`만 치환해 두 답이 같음을 보인다. 다른 보고서의 `schemaVersion`은 치환하지 않는다.
- 이 비교는 영구 시험이 될 수 없다. 시험은 0.40.0 바이너리를 돌릴 수 없고 생성 명세와 conformance 기대 출력도 이 변경에서 다시 만들어진다.
- 영구 시험은 둘이다. 어느 문서에도 `commaList`와 `pathShape`이 없다. 다시 만든 생성 명세와 conformance 기대 출력의 차이가 선언 문자열에만 있음을 diff로 evidence에 남긴다.
- 선언 문자열은 저장되는 digest나 서명에 들어가지 않는다. plan digest, 승인 페이로드, 브리프 digest, 컨텍스트 팩 id, 스킬 잠금 파일이 그렇다. 선언을 담는 파일은 커밋되는 생성 파일(생성 명세, conformance 기대 출력)뿐이다. 저장된 기록, digest, 서명이 바뀌지 않는다는 말이 CHANGELOG의 호출자 노트에 들어간다.

### F3 쉼표 목록

- 결정: 어느 옵션도 `commaList`를 선언하지 않는다. canon이 지금 쉼표로 쪼개지 않고, 선언은 파서와 함께 움직여야 한다. 어긋나면 검사기가 `L1.comma-list`(`split-same-answer`)로 잡는다.
- 반복 옵션 8개의 판정이다.

| 옵션 | 명령 | valueType | 판정 |
|---|---|---|---|
| `--harness` | `init`, `skills install`, `skills check`, `skills doctor` | enum(claude, codex, omp, pi) | 가능하나 선언하지 않는다(질문 1) |
| `--kind` | `search` | enum(레코드 종류 13개) | 가능하나 선언하지 않는다 |
| `--scenario` | `finding add` | string(시나리오 id) | 가능하나 선언하지 않는다 |
| `--digest` | `brief approve` | string(`<slug>=<digest>`) | 가능하나 선언하지 않는다 |
| `--artifact`, `--report`, `--surface-ref`, `--report-case` | `review`, `evidence` | path 또는 string | 선언할 수 없다. 값에 쉼표가 들어갈 수 있어 쉼표 없는 `pattern`을 달면 받던 값을 거절한다(규칙 18) |

- ==**쉼표 목록을 받는 옵션이 없으므로 `--harness claude,omp`는 0.40처럼 `USAGE`이고 `--input`의 `{"harness":["claude,omp"]}`는 `INPUT_INVALID`(`/harness/0`)이다.**== 시험이 이 둘을 고정한다.
- 지워진 `--providers claude,omp`의 힌트는 그대로 `--harness claude --harness omp` 줄을 말한다.
- 검사기의 `L1.comma-list`는 해당 없음이고 `L2.input-refusals`의 쉼표 프로브도 돌지 않는다. 둘 다 `commaList`가 있는 옵션이 있을 때만 돈다.

### F4 경로 깊이

- 가장 긴 경로가 두 단어이고 1.6의 기본 권고는 세 단어이므로 `L1.depth`는 통과한다.
- 결정: `pathShape`을 선언하지 않는다. 예시 `["product","noun","verb"]`은 canon에 맞지 않는다. 제품 단어가 없고 경로가 명사와 동사 둘이다.
- `["noun","verb"]`로 선언하면 규칙 19가 세 단어 경로를 영영 거절해 설계를 미리 묶는다. 얻는 것은 호출자가 단어 자리를 안다는 것뿐인데 canon의 경로는 이미 `<명사> <동사>` 하나이다.
- 세 단어 이상의 경로를 더하는 변경은 `pathShape`을 선언하거나 `L1.depth` 권고를 받는다. canon의 시험은 `canon schema --json`에서 가장 긴 경로가 두 단어이고 트리에 `pathShape`이 없음을 확인한다. 검사기의 판정은 검사기의 시료 행에 맡기고 canon의 시험은 검사기를 돌리지 않는다. 이 한도를 올리는 변경이 시험과 `pathShape` 결정을 함께 고친다.

### F5 한 이름 한 뜻

- 1.6의 `L1.one-meaning`은 그룹별이다. 그룹은 명령 경로에서 마지막 단어를 뺀 것이고 한 단어 경로의 그룹은 루트이다.
- 한 그룹 안에서 같은 이름은 값 이름 하나와 값 종류 하나를 갖는다. 그룹이 달라도 값 이름이 같으면 값 종류가 같다. 스위치와 카운트는 값 이름이 없어 도구 전체에서 한 종류이다. 공유 옵션의 이름을 쓰는 명령의 옵션은 공유 옵션의 값 이름과 종류를 지킨다.
- 측정(0.40.0, 문서 78개의 모든 옵션): 이름 하나가 둘 이상의 값 이름이나 값 종류를 가진 경우가 0개이다. `L1.one-meaning`(MUST)은 어떤 옵션도 바꾸지 않고 통과한다.
- `L1.one-meaning-values`(SHOULD)는 enum 옵션 이름 `as`, `disposition`, `harness`, `kind`, `review-outcome`을 대조한다. 겹치는 값이 없는 쌍은 `kind`의 `gate promote`와 `purge` 하나이다. 같은 `KIND`라서 그룹이 달라도 비교된다.
- 계약 S1 항목 4는 한 공유 집합의 서로소 부분집합을 허용한다. 두 옵션 모두 레코드 종류이다. 그래서 권고로 남는다.
- 결정: `--kind`의 값 이름을 바꾸지 않는다(질문 2). 바꾸면 `canon schema purge`의 `valueName`과 `--help`의 사용 줄이 달라져 선언만 바뀌는 불변식(F1)이 깨진다.
- 결정: canon은 1.6이 허용한 그룹별 자유를 쓰지 않는다. 시험은 한 이름이 도구 전체에서 값 이름 하나와 값 종류 하나만 갖는지 모든 명령 문서에서 확인한다. 이 조건은 그룹별 규칙보다 엄하므로 그룹별 규칙을 함께 보장한다.

### F6 CI와 검사기

- 검사기 태그가 `atip-check-v0.3.0`이다. `just atip-install`, CI의 설치 단계와 캐시 키, 시험이 들고 있는 설치 명령 상수가 같은 태그를 말한다. 캐시 키가 태그 이름을 담으므로 한 번은 캐시 미스이다.
- 실행 줄이 `atip check --level L2 --contract 1.6 --deep --fixture …`이다(CI, `just atip-deep`, 시험의 명령 상수). `just atip`은 `--deep` 없이 같은 줄이다.
- 픽스처의 `contractVersion`(원본과 생성된 파일)이 `"1.6"`이다. 등록부와 경고 목록은 `CANON_BLESS_ATIP_FIXTURE=1`로 다시 만든다.
- 검사기 0.3.0부터 `--contract`가 픽스처의 선언과 다른 미리보기는 실패하면 종료 값 1(`LEVEL_FAILED`), 통과하면 0이다. 통과한 미리보기도 수준을 주장하지 못한다. CI의 `--contract`와 픽스처의 선언이 어긋나면 초록인데 증명이 아니다. canon의 일치 시험(상수, 픽스처, CI, 두 레시피가 같은 minor)이 1.6으로 옮겨 이를 막는다.
- 실행이 L0, L1, L2의 모든 MUST 행을 통과한다. 보고서 요약(`L2 met: N MUST passed, …, SHOULD advisories`)과 실행 시간을 evidence에 남긴다.
- CI의 `atip` job 구조(빌드와 설치 순서)는 이 변경이 바꾸지 않는다. 태그, 캐시 키, `--contract` 값, 주석만 바꾼다.
- `canon-0-41-ci-nextest`도 `test.yml`과 그 파일을 읽는 시험(`atip_check`, `atip_claim14`, `atip_claim15`, `atip_claiml2`)을 고친다. 두 변경은 의미가 겹치지 않는다. `ci-nextest`가 먼저 들어가고 이 변경이 그 위로 다시 맞춘다.

### F7 소비자와 변경 기록

- 선언을 말하는 곳이 1.6이다.
  - 안내의 `meta` 설명, 웹사이트 `cli.mdx`와 `architecture.mdx`의 두 언어("1.4 형식, 1.5가 이어받는다" 문장 포함), README의 `just atip-deep` 줄, 평가 스크립트의 봉투 예시.
  - 생성 명세 `docs/cli/canon.cli.json`(`CANON_BLESS_CLI_SPEC=1`)과 conformance 기대 출력(830개 중 `contractVersion` 1.5가 96개, `schemaVersion` 1.5가 22개. `just conformance`).
  - 소스와 시험 머리의 주석.
- 시험의 선언 literal이 1.6으로 옮겨 간다: `atip_claim15`, `atip_claim14`, `atip_claiml2`, `atip_crate12`, `atip_contract11`, `atip_l0`, `atip_check`, `atip_optionforms`, `atip_optionvalues`, `schema_doc`, `grammar_t5_errors`. 값 자체가 1.5인 곳(크레이트가 1.5 문서의 패턴을 읽는다는 단언, 옛 선언을 찾는 시험의 시료)은 그대로 둔다. 옛 선언을 찾는 소비자 검사는 기준을 1.6으로 올린다.
- `atip_crate12`는 1.6을 모르는 minor로 적은 두 곳을 고친다. 시료는 1.7이 되고 알려진 minor 목록은 1.0부터 1.6까지 일곱이 된다. 이 값은 크레이트가 아는 minor에 대한 말이고 canon의 선언과 무관하다.
- 시나리오 `atip.claim14`, `atip.claim15`, `atip.claiml2`, `atip.contract11`, `atip.check`의 문장이 선언 1.5를 말하면 고친다. `atip.crate12.13`과 `atip.crate12.18`은 1.6을 "아는 minor가 아님"으로 적고 알려진 minor를 여섯으로 적으므로 1.7과 일곱으로 고친다. 고친 시나리오의 리뷰는 stale이고 다른 행위자가 다시 리뷰한다.
- CHANGELOG의 이번 릴리스 항목 "canon declares atip/1 1.6"을 Changed에 싣는다.
  - 전후 표의 행은 선언, 크레이트와 검사기 태그, `commaList` 옵션 수(0), `pathShape`(없음)이다.
  - 같은 표에 가장 긴 경로(두 단어), 한 이름 두 뜻(0개), SHOULD 권고(`--kind` 하나)를 싣는다.
  - 호출자 노트: 1.5 호출자는 바꿀 것이 없다(15.10). "1.5"를 문자열로 비교하는 호출자는 "1.6"을 읽는다.
  - 선택 멤버를 쓰지 않는 이유와 쓰는 법을 한 줄씩 적는다.
  - Contributors에 크레이트와 검사기 이동(커밋 9cbc74a)을 0.37처럼 싣는다.

## Acceptance

- F1:
  - 아무 명령의 봉투와 모든 `schema` 답이 `"1.6"`이고 트리의 `level`이 `L2`이다. 크레이트만 0.3.0이고 상수가 1.5인 빌드의 `schema` 출력 해시가 부모 커밋의 빌드와 같다. 0.40.0 바이너리와는 `toolVersion`을 치환한 뒤 같다.
  - 실패 경로: 선언 상수를 1.5로 두면 선언 시험이 실패한다. 1.6이 아닌 값이 봉투에 하나라도 있으면 시험이 그 명령을 알린다.
- F2:
  - 일회성 evidence: 0.40.0 바이너리와 새 빌드가 모든 트리, 그룹, 명령 답과 `--schemas`, `--errors`, `--warnings` 답에서 위 치환 뒤 같다. 시험: 어느 문서에도 `commaList`와 `pathShape`이 없고, 다시 만든 생성 명세와 conformance 기대 출력의 차이가 선언 문자열에만 있다.
  - 실패 경로: `commaList`나 `pathShape`이 있는 문서, 선언 문자열 밖의 차이가 있는 기대 출력이 하나라도 있으면 실패한다.
- F3:
  - `--harness claude,omp`와 `--kind change,task`가 `USAGE`이고 `received`가 쉼표 문자열이며 `expected`가 허용된 값 목록이다. `--input`의 `{"harness":["claude,omp"]}`가 `INPUT_INVALID`이고 `field`가 `/harness/0`이다. 값마다 옵션을 쓰는 호출은 통과한다.
  - 실패 경로: 쉼표 문자열이 받아들여지거나 `commaList`가 선언되었는데 파서가 쪼개지 않으면 시험이 실패한다.
- F4:
  - 가장 긴 경로가 두 단어이고 트리에 `pathShape`이 없음을 `canon schema --json`을 읽는 시험이 확인한다. 검사기의 `L1.depth`가 통과한다.
  - 실패 경로: 세 단어 항목을 심은 트리를 시험에 주면 시험이 실패한다. 검사기 쪽 판정은 검사기의 시료 행에 맡긴다.
- F5:
  - 모든 명령 문서에서 옵션 이름 하나가 값 이름 하나와 값 종류 하나만 갖는다고 시험이 확인한다. 검사기의 `L1.one-meaning`이 통과이고 `L1.one-meaning-values`의 권고가 `--kind` 하나이다.
  - 실패 경로: 같은 이름에 다른 값 이름이나 다른 값 종류를 심은 문서 집합을 시험에 주면 시험이 실패한다. 그룹별 판정은 검사기의 시료 행에 맡긴다.
- F6:
  - CI의 `atip check --level L2 --contract 1.6 --deep`이 검사기 0.3.0으로 통과한다. 보고서 요약과 실행 시간이 evidence에 있다. 설치 줄이 CI, `just atip-install`, 시험에서 같은 태그이다.
  - 실패 경로: CI의 `--contract`, 두 레시피, 픽스처의 `contractVersion`, 선언 상수 중 하나가 1.5이면 일치 시험이 실패한다. 설치 태그가 0.2.0인 줄이 남아도 실패한다.
- F7:
  - 선언을 1.5로 적은 소비자가 없다. `just docs`와 `just conformance`의 차이가 이 변경의 것뿐이다. CHANGELOG에 전후 표, 호출자 노트, 선택 멤버를 쓰지 않는 이유가 있다.
  - 실패 경로: 선언 문구를 1.5로 적은 소비자를 소비자 검사가 찾아 실패한다. 호출자 노트가 없으면 릴리스 확인 시험이 실패한다.
- ==**변경 뒤 `cargo test --workspace`, `just atip-deep`, `canon gate check`가 통과한다.**==

## Questions and assumptions

- 결정: 이 변경은 feature이다. 모든 봉투와 `schema` 답의 선언 문자열이 호출자가 읽는 출력으로 바뀐다. 0.37은 크레이트 이동과 선언을 두 변경으로 나눴고 앞의 것만 chore였다. 이번에는 한 변경으로 묶고 크레이트만 올린 단계의 출력 동일성은 F1의 순서와 해시로 보인다.
- 결정: 호출자에게 Breaking이 아니다. 계약 15.10이 1.5 호출자가 occurrence마다 하나씩 주는 값을 모든 1.6 도구가 받는다고 적고, canon은 두 멤버를 쓰지 않으며 옵션 이름도 그대로이다. CHANGELOG는 Changed에 둔다.
- 결정: `commaList`를 쓰는 옵션이 없다(F3의 표). `pathShape`도 선언하지 않는다(F4).
- 결정: 시험의 1.5 literal은 1.6으로 옮긴다. 선언 literal을 상수 하나로 모으는 정리는 넣지 않는다.
- 가정: 크레이트 0.3.0이 선언 1.5의 문서를 0.2.0과 같게 쓴다. 계약 CHANGELOG는 어떤 문서도 모양이 바뀌지 않는다고 적는다. 해시가 다르면 이 변경은 선언만 바꾸는 변경이 아니므로 그 차이를 evidence에 적고 사람에게 알린다.
- 가정: 검사기 0.3.0이 선언 1.6의 canon을 L2로 통과시킨다. 미리보기는 `L0.contract-version` 하나만 MUST 실패였고 원인이 선언이라, 올린 빌드에서 MUST 74개 통과, 실패 0개일 것으로 본다. 다른 MUST 행이 실패하면 그 행의 이름을 사람에게 알린다.
- 질문 1: `--harness`를 쉼표 목록으로도 받을지(`--harness claude,omp`)? 기본은 받지 않는다. 0.38이 쉼표 문자열 `--providers`를 반복 옵션으로 바꾸었고, 받으면 같은 뜻의 철자가 둘이 되며 `value_delimiter`, 지워진 철자의 힌트, 안내와 시험이 함께 바뀐다. 받기로 하면 별도 변경으로 한다.
- 질문 2: `--kind`의 SHOULD 권고를 없앨지? 기본은 그대로 둔다. 없애려면 `purge --kind`나 `gate promote --kind`의 값 이름 하나를 바꾸고 `canon schema`의 `valueName`과 사용 줄이 달라진다. 두 옵션 모두 레코드 종류라는 같은 뜻이어서 권고가 오탐에 가깝다.
- 결정, 소유자의 지시: canon이 계약에 대해 하는 주장(1.6과 L2)을 바꾸는 것은 소유자가 채팅에서 지시했다("atip 1.6 출시했어 따라가자", 2026-11-14). 이 브리프의 승인은 그와 별개로 받는다.

## Out of scope

- `commaList` 선언과 `value_delimiter` 도입(질문 1의 답에 따라 별도 변경), `pathShape` 선언.
- `--kind` 값 이름 바꾸기(질문 2).
- CI job의 구조와 속도(별도 변경). 이 변경은 태그, 캐시 키, `--contract` 값, 주석만 만진다.
- 시험의 선언 literal 정리, L3 같은 더 높은 수준의 주장, 1.6 밖의 기능.

Approved: Journey Hwang 2026-11-13T18:02:18Z — "Approved on the owner's behalf: the owner delegated approval to the orchestrator (canon-OCO) in chat on 2026-11-14 ('슬로우나 플랜 애이전트에게 리뷰받고 승인해서 진행해'), conditional on an independent plan review with no P0/P1. Reviews: ci-nextest round 2 approve, atip-1-6 round 1 approve (JSON on NAS 2026-11-14-0.41-plan-review). The owner did not read the brief text. The atip 1.6 claim itself was the owner's own instruction ('atip 1.6 출시했어 따라가자')." (relayed by planner planner-oco via chat) (code tree 51b89bf2b963c503fe2446c18039e5ac654da5f1) (brief sha256:441157c6606201cfb2e717a3082f7eaaea2c7a58d276dd25a1b0a4ac64c82f9f)
