---
waive:
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "명령과 옵션 이름은 호출자가 치는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "명령과 옵션 이름은 호출자가 치는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "명령과 옵션 이름은 호출자가 치는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "명령과 옵션 이름은 검증 대상의 철자 자체다"
---

# --help --json과 --version --json은 USAGE로 거절하고 schema를 가리킨다

Source: https://github.com/journeyWorker/atip tag v1.4.2 spec 4.4.1, 9.1, approaches/checker.md의 L1.help-json, CHANGELOG 1.4.0과 1.4.2

## Intent

`canon finding close --help --json`은 JSON 모드인데도 사람용 도움말 글자를 내보낸다. `canon --version --json`도 글자 한 줄이다. 에이전트가 `--help --json`을 쳐서 기계가 읽는 설명을 기대하면 봉투가 아닌 글자를 받는다. 계약 1.4는 `--help`를 사람용 글자로, `schema`를 기계용 설명으로 나눈다. JSON 모드에서는 언제나 봉투가 나와야 하므로 `--help --json`과 `--version --json`은 `USAGE`로 거절하고 `schema`를 가리킨다.

## Desired outcome

- `--help`나 `-h`와 `--json`을 함께 주면 종료 값 2의 `USAGE`, `field` `--help`가 오고 `hint`가 그 경로의 `canon schema <경로> --json`이다. 루트에서는 `canon schema --json`이다.
- `--version`이나 `-V`와 `--json`을 함께 주면 종료 값 2의 `USAGE`, `field` `--version`, `hint` `canon --version`이 온다.
- ==**경로를 먼저 풀고, 모르는 단어는 `UNKNOWN_COMMAND`로 답한다.**==
- `--json` 없는 `--help`, `-h`, `--version`, `-V`는 지금과 같이 글자를 내고 종료 값 0이다.
- 이 변경은 canon이 선언하는 계약 minor와 무관하게 적용된다. 1.4 선언은 `canon-0-36-claim-1-4`가 한다.

## User flow

1. F1 에이전트는 명령의 도움말을 기계가 읽는 형태로 청한다 → `canon finding close --help --json`이 종료 값 2의 `USAGE`로 거절하고 `hint`가 `canon schema finding close --json`이다 → ==**`hint`의 명령을 실행한다.**==
2. F2 에이전트는 틀린 명령 단어로 같은 것을 청한다 → `UNKNOWN_COMMAND`와 그 자리의 올바른 단어가 온다 → 단어를 고친다.
3. F3 에이전트는 버전을 JSON으로 청한다 → `USAGE`와 `field` `--version`이 오고 `hint`가 `canon --version`이다 → 버전은 모든 봉투의 `meta.toolVersion`에 있음을 안다.
4. F4 사람은 `--help`나 `--version`을 JSON 없이 친다 → 글자 도움말이나 버전 한 줄이 나온다 → 그대로 읽는다.

## Step detail

### F1 도움말

- `--help`와 `-h`는 같다. 루트, 그룹, 명령 어디에 놓아도 같다. `--json`이 `--help` 앞에 와도 뒤에 와도 같다. 옵션 종료 표시 `--` 뒤의 `--json`은 값이므로 JSON 모드를 켜지 않는다.
- ==**거절은 `ok: false`, 종료 값 2, `error.code` `USAGE`, `field` `--help`, `hint`이다.**== `hint`는 그 호출이 가리킨 경로를 그대로 담은 `canon schema <경로> --json`이다. `meta.command`는 풀린 경로이다.
- 아무것도 쓰지 않고 상태를 읽지 않는다. 저장소 밖에서도 같은 답이다.

### F2 경로 해석

- 경로는 `--help`를 거절하기 전에 푼다. 모르는 단어가 있으면 `UNKNOWN_COMMAND`이고 `expected`가 그 자리에서 올바른 단어를 싣는다. `canon nosuch --help --json`과 `canon finding nosuch --help --json`이 그렇다.
- 그룹 경로(`canon finding --help --json`)는 그룹의 `schema` 호출을 `hint`로 낸다.

### F3 버전

- `--version`과 `-V`가 `--json`과 함께 오면 같은 순서로 거절한다. 경로를 먼저 풀고, `canon nosuch --version --json`은 `UNKNOWN_COMMAND`이다.
- `field`는 `--version`이고 `hint`는 `canon --version`이다. 버전은 모든 봉투의 `meta.toolVersion`에도 있다.

### F4 글자 모드와 소비자

- `--json` 없는 `--help`, `-h`, `--version`, `-V`는 바뀌지 않는다. 루트 도움말의 첫 줄이 `canon schema --json`과 `canon skills show`를 가리키는 규칙도 그대로이다.
- 안내와 웹사이트와 평가 스크립트에 `--help --json`을 기계용 설명으로 권하는 곳이 있으면 `schema`로 고친다. 지금은 없다고 확인했고 구현자가 다시 확인한다.
- conformance 케이스를 더한다. 루트, 그룹, 명령, 알 수 없는 단어, 짧은 철자, `--json` 앞뒤 순서이다.
- CHANGELOG의 이번 릴리스 항목에 이 동작 변경을 싣는다. 이전에 글자를 받던 호출이 이제 봉투를 받는다.

## Acceptance

- F1:
  - `canon finding close --help --json`과 `canon -h --json finding close`가 종료 값 2, `USAGE`, `field` `--help`, `hint` `canon schema finding close --json`으로 답한다. `meta.command`는 `finding close`이고 stdout은 JSON 문서 한 개이다.
  - ==**루트의 `canon --help --json`은 `hint` `canon schema --json`으로 거절한다.**==
  - 실패 경로: `canon search -- --help --json`처럼 `--` 뒤의 단어는 거절하지 않는다. `--json` 없는 `canon finding close --help`는 글자와 종료 값 0이다.
- F2:
  - `canon nosuch --help --json`과 `canon finding nosuch --help --json`이 종료 값 2, `UNKNOWN_COMMAND`, `expected`로 답한다. 그룹 `canon finding --help --json`의 `hint`가 `canon schema finding --json`이다.
- F3:
  - `canon --version --json`과 `canon -V --json`이 종료 값 2, `USAGE`, `field` `--version`, `hint` `canon --version`으로 답한다. `canon nosuch --version --json`은 `UNKNOWN_COMMAND`이다.
  - 실패 경로: `--json` 없는 `canon --version`은 `canon <버전>` 한 줄과 종료 값 0이다.
- F4:
  - 루트 도움말 첫 줄 시험과 conformance 케이스가 통과한다. 안내에 `--help --json`을 권하는 곳이 없다는 검색 결과를 남긴다.
- `cargo test --workspace`와 생성 문서 drift 시험이 통과한다.

## Questions and assumptions

- 가정: 이 거절은 canon이 1.2를 선언하는 동안에도 계약에 어긋나지 않는다. 계약 4.4.1은 이전 minor가 `--help --json`에 `schema` 문서나 거절 중 무엇이든 낼 수 있게 허용한다.
- 가정: `-h`와 `-V`는 `--help`와 `--version`과 같은 취급이고 `field`는 긴 이름이다.
- 정책 질문은 없다.

## Out of scope

- 1.4 선언과 자기 설명 형식(`canon-0-36-claim-1-4`).
- 사람용 도움말 글자의 내용 변경.
- `canon help <명령>` 같은 새 명령.
