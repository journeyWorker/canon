---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "검사 16개의 규칙, 실행 방법, 결과 상태, fixture, 시험 방식을 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "여섯 단계가 각자 입력, 환경, 판정 규칙을 가져야 해서 줄이면 사실이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "여섯 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.length"
    section: "questions"
    reason: "계약 저자에게 확인할 가정이 서로 다른 결정이라 함께 적어야 한다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "검사 id와 코드 이름은 사용자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "검사 id, 옵션, fixture 키, 코드 이름은 사용자가 쓰는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "검사 id, 옵션, 코드 이름은 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "코드 이름과 fixture 키는 결정 대상의 철자 자체다"
---

# atip check: atip/1 L0 적합성 검사기

Source: https://github.com/journeyWorker/atip (atip/1 1.0) approaches/checker.md §1-§7, spec 17

## Intent

atip/1 계약은 규칙만 있고, 빌드된 바이너리를 밖에서 대어 보는 도구가 없다. atip 레포에는 예시 문서와 그 문서를 검증하는 `scripts/validate.sh`뿐이다. 도구가 계약을 지키는지는 도구마다 자기 테스트로 확인하고, 같은 규칙을 도구마다 따로 해석한다. canon 0.33이 L0를 주장하려면 통과 기준이 되는 하나의 구현이 필요하다.

## Desired outcome

- `atip check`는 도구의 빌드된 바이너리를 실행해 argv, stdout, stderr, 종료 코드, 파일 시스템 변화로 L0 규칙을 검사한다. 도구의 소스를 읽지 않고 도구가 clap이나 Effect로 만들어졌는지 모른다.
- 검사마다 규칙 번호와 결과를 보고한다. 결과는 통과, 실패, 제공되지 않음, 해당 없음, 함의됨 중 하나다. L0 MUST 검사가 모두 통과하면 종료 코드 0이다.
- ==**검사기의 각 검사는 계획한 위반을 심은 가짜 도구에서 실제로 실패한다.**== 실패하지 않는 검사는 아무것도 증명하지 못하기 때문이다.
- canon의 CI가 canon 바이너리에 대해 L0를 돌리고, canon이 통과한다.

## User flow

1. F1 도구 작성자는 `atip check --bin <바이너리> --level L0 --fixture <파일>`을 실행한다 → L0 검사 16개의 결과를 읽는다 → MUST 검사가 모두 통과하면 종료 코드 0이고, 하나라도 실패하면 1이다.
2. F2 도구 작성자는 fixture 파일을 준다 → 검사기가 fixture의 준비 명령을 한 번 돌리고, 검사마다 그 결과의 새 사본에서 바이너리를 실행한다 → 검사끼리 상태를 오염시키지 않는다.
3. F3 도구 작성자는 거절(exit 2)의 부작용을 확인한다 → 거절이 작업 디렉터리나 HOME을 바꾸면 실패로 보고된다 → 바뀐 경로 목록을 받는다.
4. F4 도구 작성자가 `--level L1`이나 `L2`를 준다 → 검사기가 해당 레벨은 아직 없다고 거절한다 → 어느 릴리스에 생기는지 안다.
5. F5 검사기 개발자는 검사기의 시험을 돌린다 → 정상 가짜 도구는 L0 전부 통과하고, 위반을 심은 가짜 도구는 그 검사만 실패한다 → 검사기를 믿을 수 있다.
6. F6 canon 개발자는 CI에서 canon에 대해 `atip check`를 돌린다 → canon 바이너리가 L0 검사 16개를 통과한다 → ==**0.33이 L0를 주장할 수 있다.**==

## Step detail

### F1 검사 실행과 보고

- 명령은 `atip check`이다. 옵션은 `--bin`(필수), `--level`(필수, 지금은 `L0`만), `--fixture`(필수), `--json`이다. 검사기 자신도 L0를 따른다. `--json`이면 봉투 한 개를 내고, 텍스트가 기본이다. 종료 코드는 0, 1, 2뿐이다.
- 검사는 계약 검사기 문서의 L0 표 그대로 16개다. MUST는 `version`, `unknown-command`, `unknown-command-text`, `bare`, `unknown-option`, `json-mode-preparse`, `after-dashdash`, `ok-exit`, `no-null`, `one-document`, `no-env-switch`, `no-env-switch-text` 12개다. SHOULD는 `version-format`, `text-error-form`, `compact`, `contract` 4개다. 각 id는 `L0.<이름>`이다.
- 봉투를 내는 모든 호출의 stdout은 계약 스키마(봉투, 오류, 레지스트리 행)로 검증한다. 스키마는 원본 그대로 `crates/atip`의 한 벌을 쓴다. JSON Schema 2020-12 검증기를 쓰고 `format`은 주석으로만 본다.
- 실패 오류 코드는 fixture의 `registry`에서 찾는다. 등록되지 않은 코드, 종료 값이나 `retryable`이 행과 다른 오류는 `ok-exit`에서 실패한다.
- 결과 상태는 다섯 가지다: 통과, 실패, 제공되지 않음(fixture에 필요한 항목이 없음), 해당 없음, 함의됨. SHOULD 검사의 실패는 권고로만 보고하고 레벨을 떨어뜨리지 않는다. ==**MUST 검사가 실패하거나 제공되지 않으면 레벨은 통과하지 못한다.**==
- 보고 데이터는 `level`, `passed`, 검사 목록(`id`, `strength`, `result`, 규칙 번호, 실패 이유, 관찰한 값)이다. 텍스트 모드는 검사 한 줄씩과 요약 한 줄이다. 텍스트는 계약이 아니다.
- 실패한 레벨은 종료 코드 1, 오류 코드 `LEVEL_NOT_MET`이고 data에 검사 목록을 싣는다. 통과하면 성공 봉투다.
- 거절(exit 2)은 사용 오류, 실행할 수 없는 바이너리, 읽을 수 없거나 스키마에 어긋나는 fixture, 지원하지 않는 레벨이다. 각자 코드(`USAGE`, `BINARY_UNUSABLE`, `FIXTURE_INVALID`, `LEVEL_UNSUPPORTED`)를 가지고 아무것도 쓰지 않는다.

### F2 fixture와 실행 환경

- fixture는 JSON 파일이고 L0에서 읽는 키는 `setup`, `env`, `timeoutSeconds`, `probe`, `registry`다. `commands`, `search`, `domainRejects`는 있어도 무시한다.
- `setup`은 명령의 argv 배열 목록이다. 한 번 돌려 템플릿 디렉터리를 만든다. 시퀀스마다 템플릿의 새 사본을 작업 디렉터리로 쓰고 HOME도 사본 안을 가리킨다.
- `probe`는 상태 없이 실행되는 명령의 id이고, 공백으로 나눈 단어가 argv가 된다. `registry`는 레지스트리 행 `{code, exit, retryable, meaning, fix}`의 배열이다. 둘 중 하나라도 없으면 그것이 필요한 MUST 검사는 제공되지 않음이다. 그래서 fixture를 빼서 통과할 수 없다.
- 환경은 `PATH`, `HOME`, `LANG=C.UTF-8`, fixture의 `env`만이고 stdin은 닫는다. 텔레메트리 끄기 변수는 설정하지 않는다. 시간 제한은 기본 30초이고, 넘기면 이유를 적은 실패다.
- 터미널이 필요한 검사(`no-env-switch`의 pty 실행)는 pty 아래에서 돌린다.
- 검사기는 대상 도구의 레포에 아무것도 설치하거나 쓰지 않는다.

### F3 거절의 부작용

- 호출 앞뒤로 작업 디렉터리와 HOME의 파일 목록(경로, 크기, SHA-256)을 비교한다.
- L0에서 exit 2로 끝난 모든 호출이 아무것도 바꾸지 않아야 한다. 바꾸면 MUST 실패이고 보고가 바뀐 경로를 적는다(계약 8.2). 그 밖의 호출(미리보기, `--help`, 성공한 읽기)의 변화는 L0에서 판정하지 않는다.
- `effectNote`는 보지 않는다. L0에는 허용 목록 키가 없다.

### F4 L1과 L2 거절

- `--level L1`과 `--level L2`는 exit 2, `LEVEL_UNSUPPORTED`이고 hint가 0.34, 0.35에 추가된다고 말한다. 바이너리를 실행하지 않는다. `L0`, `L1`, `L2`가 아닌 값은 `USAGE`다.

### F5 검사기 시험

- 가짜 도구는 JSON 설정으로 동작이 정해지는 작은 프로그램이다. 정상 설정은 L0 16개를 모두 통과한다.
- 검사 id마다 그 규칙 하나만 어기는 설정이 있다. 시험은 그 검사가 실패하고 다른 MUST 검사는 통과하거나 해당 없음임을 확인한다. 예: `hint: null`은 `no-null`, 모르는 옵션의 exit 1은 `ok-exit`, 오류를 stderr에도 찍으면 `unknown-command`, `CI=1`이면 텍스트가 JSON으로 바뀌면 `no-env-switch-text`.
- atip 레포의 예시 문서를 검사기의 시험 입력으로 쓴다. 봉투인 `report-*.json` 예시는 봉투 검증을 통과하고, 봉투와 오류 규칙을 어기는 `invalid/` 예시는 거부된다. L1과 L2 규칙만 어기는 예시는 이유를 적어 건너뛴다. 사본은 계약 저장소의 커밋 `365244c`에 고정하고 어느 파일이 무엇을 기대하는지 표로 둔다.
- 검사기는 자기 자신에게도 돌아간다. `probe`가 `check`이고 `registry`가 검사기의 레지스트리인 fixture로 `atip check --bin atip --level L0`이 통과한다.

### F6 canon에 대한 실행

- canon용 fixture를 레포에 둔다. `setup`은 `init`이고 `probe`는 `status`다. `registry`는 `canon schema --errors --json`에서 생성하며 손으로 쓰지 않는다. 생성된 파일이 커밋된 파일과 다르면 테스트가 실패한다.
- CI 잡 하나가 canon 바이너리와 `atip` 바이너리를 만들어 `atip check --bin <canon> --level L0 --fixture <파일>`을 돌린다. justfile에 같은 명령의 레시피가 있다.
- canon은 MUST 12개와 SHOULD 4개를 모두 통과한다. 이 단계는 `canon-0-33-atip-l0`가 출시된 뒤에만 초록이 된다.

## Acceptance

- F1:
  - 정상 가짜 도구에 `atip check --level L0 --json`은 종료 코드 0이고 `data.checks`에 L0 id 16개가 있고 MUST 12개의 결과가 통과다.
  - 위반을 심은 도구는 종료 코드 1, `LEVEL_NOT_MET`이고, 실패한 검사의 `id`와 규칙 번호와 이유가 data에 있다.
  - 검사기의 봉투가 계약 봉투 스키마를 통과한다. 없는 바이너리 경로는 종료 코드 2, `BINARY_UNUSABLE`이다.
- F2:
  - `setup`이 만든 파일이 검사마다 새로 복원된다. 한 검사가 사본에 쓴 파일이 다음 검사에 보이지 않는다.
  - fixture에서 `probe`나 `registry`를 빼면 해당 MUST 검사가 제공되지 않음으로 보고되고 종료 코드는 1이다.
  - 한 호출이 시간 제한을 넘기면 그 검사가 이유가 적힌 실패다. 읽을 수 없거나 모양이 틀린 fixture는 종료 코드 2, `FIXTURE_INVALID`이다.
- F3:
  - exit 2로 끝나면서 파일을 하나 쓰는 가짜 도구는 MUST 실패이고 보고에 그 경로가 있다.
  - 아무것도 쓰지 않는 거절은 통과한다. 성공한 호출이 파일을 쓰는 것은 L0에서 실패가 아니다.
- F4:
  - `--level L1`은 종료 코드 2, `LEVEL_UNSUPPORTED`이고 hint에 0.34가 있다. `L2`는 0.35다. `--level L9`는 `USAGE`다.
  - 거절은 바이너리를 실행하지 않고 아무것도 쓰지 않는다.
- F5:
  - ==**검사 16개 모두에 대해 위반 설정 시험이 있다.**== 그 검사가 실패하고, 다른 MUST 검사는 통과하거나 해당 없음이다.
  - 계약 저장소의 유효한 봉투 예시가 모두 통과하고 거부해야 할 예시가 모두 거부된다.
  - 실패 경로: 검사 하나를 항상 통과하도록 바꾸면 그 검사의 위반 시험이 실패한다. 검사기에 `atip check`를 돌린 결과가 통과다.
- F6:
  - 커밋된 canon fixture의 `registry`가 `canon schema --errors --json`에서 생성한 것과 같다. CI 잡이 canon에서 L0 통과로 끝난다.
  - 실패 경로: canon의 오류 하나를 `hint: null`로 바꾸면 CI 잡이 `no-null` 실패로 빨개진다.

## Questions and assumptions

- 결정(계약 저자 답): 검사기는 Rust로 쓰고 JSON Schema 2020-12 검증기를 쓴다. `format`은 주석으로만 본다. 계약 스키마는 원본 그대로 넣는다. 워크스페이스에 `crates/atip-check`를 두고 바이너리 이름은 `atip`이다. 나중에 `crates/atip`처럼 atip 레포로 옮긴다.
- 결정(계약 저자 답): 이 릴리스는 L0 검사 16개를 모두 구현한다. 검사기 문서의 L0 표가 16개이고 저자가 16이 맞다고 확인했다. `--level L1`과 `L2`는 exit 2 `LEVEL_UNSUPPORTED`이고 hint가 0.34, 0.35를 말한다. 가짜 도구의 위반 시험이 모든 검사 id에서 실패를 만든다.
- 가정: `LEVEL_UNSUPPORTED`는 저자의 예시 이름이다. `BINARY_UNUSABLE`과 `FIXTURE_INVALID`는 exit 2이고 `LEVEL_NOT_MET`은 exit 1이라는 이름은 계획자의 기본값이다. 검사기의 레지스트리는 예약 코드 `USAGE`, `UNKNOWN_COMMAND`, `COMMAND_REQUIRED`, `INTERNAL`을 가진다.
- 결정(계약 저자 답): L0 fixture 키는 `setup`, `env`, `timeoutSeconds`, `probe`, `registry`다. `registry`는 레지스트리 행 스키마의 배열이고 canon은 `canon schema --errors --json`에서 생성한다. `probe`는 공백으로 나누는 명령 id이고 `L0.unknown-option`이 `meta.command`를 이 id와 비교한다. L0에 허용 목록 키는 없다. `commands`, `search`, `domainRejects`는 L1과 L2 키라 없어도 되고 있으면 무시한다. 검사기 자신도 L0를 따른다.
- 가정: canon fixture의 `registry`는 `canon schema --errors --json`의 각 행에서 레지스트리 행 스키마가 허용하는 다섯 키만 골라 생성한다. 그 응답이 다섯 키 밖의 멤버를 가지면 생성기가 버린다.
- 가정: pty 검사는 macOS와 Linux에서만 돈다. canon의 CI 대상과 같다.
- 결정(계약 저자 답): 플랫폼별 바이너리 배포(npm)와 L1의 ECMAScript 정규식 구현(`regress`)은 이 변경이 아니다.
- 가정: canon에 대한 CI 잡은 `canon-0-33-atip-l0`가 출시된 뒤에 초록이 된다. 순서는 crate 추출, L0, 이 변경이다. 이 변경의 F1부터 F5는 L0 변경 없이 먼저 만들 수 있다.
- 정책 질문은 없다. CI 잡 추가는 gate가 요구하는 것을 바꾸지 않는다.

## Out of scope

- L1 검사(0.34)와 L2 검사(0.35), 그리고 그 검사가 쓰는 fixture 키(`commands`, `search`, `domainRejects`).
- 정규식 이식성 검사와 ECMAScript 정규식 구현.
- 플랫폼 바이너리 배포(npm)와 crates.io 게시, atip 레포로의 이동.
- 보고만 하고 실패하지 않는 모드.
- lute 같은 다른 도구에 대한 실행.
- 계약 문서, 스키마, 예시의 수정. 사본을 읽기만 한다.
- canon의 출력을 계약에 맞추는 일(`canon-0-33-atip-l0`).

Approved: Journey Hwang 2026-10-31T10:46:20Z — "승인" (relayed by orchestrator orchestrator-main via ask) (brief sha256:e83d8889b3af0a38d408f3d13272bf72727d129d86dbf3560b34017027bc42c8)
