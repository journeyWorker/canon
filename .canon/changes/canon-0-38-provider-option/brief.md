---
waive:
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "입력 문서의 키와 설정 키 이름은 호출자가 쓰는 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "거절 코드 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "입력 문서의 키와 선언 멤버 이름은 계약이 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "입력 문서의 키 이름은 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "선언 멤버 이름은 문제의 대상이다"
  - rule: "prose.length"
    section: "brief"
    reason: "제거, 대체 철자, 입력 문서, 선언을 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
---

# 제공자는 반복하는 `--provider` 옵션으로 고른다

Source: canon 이슈 #141의 첫째 항목(ATIP 저자의 요청), ATIP 태그 v1.5.0 spec 4.2.2와 10.5와 annex-schema S1 7, canon-0-37-option-forms

## Intent

`init`과 `skills`의 세 명령은 제공자를 `--providers claude,omp`처럼 쉼표로 이은 문자열 하나로 받는다. 선언은 이름 넷과 쉼표 규칙을 정규식 하나에 적는다. ATIP는 고정 이름 목록을 값마다 한 번 쓰는 반복 열거 옵션으로 선언하게 하고 목록 열거 도메인을 더하지 않는다. 지금 이름은 `possibleValues`에 없고 `--input`은 쉼표 문자열을 받는다. 제공자를 하나도 고르지 않으려면 값 `none`을 쓴다.

## Desired outcome

- `init`, `skills install`, `skills check`, `skills doctor`는 반복 옵션 `--provider`를 받는다. 값은 `claude`, `codex`, `omp`, `pi` 중 하나이고 이름은 `possibleValues`에 한 번 적힌다.
- 제공자를 하나도 고르지 않는 일은 스위치 `--no-provider`가 맡는다. `--provider`와 함께 쓰면 거절한다.
- `--input`은 `provider` 키에 이름 배열을, `noProvider` 키에 `true`를 받는다.
- ==**`--providers`는 없어진다. 쓰면 거절하고 고친 명령 줄을 알려 준다.**==
- 같은 제공자 선택은 0.37과 같은 설정, 설치, 종료 값을 낸다.
- CHANGELOG에 옮기는 방법이 있다.

## User flow

1. F1 에이전트는 `init`이나 `skills install`에 `--provider`를 값마다 한 번씩 준다 → 이름만 받는다 → 선택한 제공자의 설정과 설치가 이전과 같다.
2. F2 에이전트는 제공자를 하나도 설치하지 않으려 한다 → `--no-provider`를 준다 → 설정의 제공자 목록이 비고 아무것도 설치되지 않는다.
3. F3 에이전트는 `skills check`와 `skills doctor`에 같은 옵션을 준다 → `skills install`과 같은 선택 규칙을 따른다 → 선택한 제공자만 점검한다.
4. F4 에이전트는 `--input` 문서로 같은 호출을 한다 → 배열과 스위치 키를 받는다 → 플래그와 같은 결과가 나온다.
5. F5 에이전트나 스크립트는 옛 `--providers`를 쓴다 → 거절된다 → ==**힌트가 `--provider`와 `--no-provider`로 고친 명령 줄을 보인다.**==
6. F6 에이전트는 `canon schema`로 네 명령을 읽는다 → 같은 선언이다 → 이름 목록을 호출 전에 안다.
7. F7 사람은 CHANGELOG를 읽는다 → 옛 철자와 새 철자의 표가 있다 → 스크립트를 고친다.

## Step detail

### F1 반복 옵션

- 받는 이름은 소문자 넷이다. 대소문자가 다른 이름, 공백이 든 이름, 모르는 이름은 `expected`와 `suggestions`를 달아 종료 값 2로 거절하고 아무것도 쓰지 않는다.
- 같은 이름을 두 번 적으면 한 번으로 받는다. 기록과 설치는 지금처럼 이름순이다.
- `init`은 이 옵션이 없으면 감지한 제공자를 쓰고, `skills install`은 설정, 설치 잠금, 감지 순으로 읽는다. 이 순서는 그대로다.
- `--provider`가 있는 `init`은 `--check`와 함께 쓰지 못한다. 이 규칙은 지금과 같다.

### F2 제공자 없음

- `--no-provider`는 지금 `--providers none`이 하던 일을 한다. 설정에 빈 목록을 쓰고 아무것도 설치하지 않는다.
- `--provider`와 함께 주면 이름을 대며 거절한다. `init`은 `--check`와도 함께 쓰지 못한다. 거절하면 아무것도 쓰지 않는다.
- `skills check`와 `skills doctor`에서 이 스위치는 선택이 빈 목록임을 뜻한다.

### F3 점검 명령

- `skills check`와 `skills doctor`는 `skills install`과 같은 선택 규칙을 읽는다. 옵션이 없을 때의 순서는 그대로다.

### F4 입력 문서

- 키는 옵션 이름의 camelCase이고 단수형이다. `provider`는 이름 배열, `noProvider`는 불리언이다.
- `provider`의 빈 배열은 지금처럼 거절한다. 없음은 `noProvider`로 쓴다. 이름 자리의 `none`은 모르는 이름이다.
- 옛 `providers` 키는 모르는 키로 거절한다.
- 플래그와 문서가 어긋나면 지금의 `INPUT_CONFLICT`가 선다.

### F5 옛 철자

- ==**옛 `--providers`는 어떤 값이어도 종료 값 2의 `USAGE`로 거절한다.**== 옛 이름의 별칭은 남기지 않는다.
- 값이 쉼표로 이은 올바른 이름이면 힌트가 `--provider claude --provider omp`로 고친 명령 줄이다. 값이 `none`이면 `--no-provider`이다. 값이 틀렸으면 힌트가 새 옵션 이름만 말한다.
- 도움말, init이 쓰는 안내 문구, init 미완료 사유의 해결 문구, 생성 문서, 스킬 안내문이 새 철자를 말한다. 옛 철자를 쓰는 곳이 남지 않는다.

### F6 선언

- 네 명령의 `provider`는 반복 열거 옵션이고 `possibleValues`가 이름 넷이다. 이름을 담은 정규식 도메인은 선언에 없다.
- `noProvider`는 스위치이다. `provider`와 `noProvider`는 서로 배타적이고 `init`은 `check`와도 배타적이다. 이 배타는 선언에 있다.
- 값표는 실제 파서에 돌려 증명한다. `just atip-deep`이 1.5로 통과한다.

### F7 변경 기록

- Breaking 항목에 표가 있다. 옛 철자, 0.37의 처리, 0.38의 처리, 옮기는 방법이다. 옮기는 방법은 값 형태마다 적는다. 쉼표 목록, `none`, 입력 문서의 `providers` 키가 있다.

## Acceptance

- F1:
  - `--provider claude --provider omp`와 `--provider omp --provider claude`가 같은 설정과 설치를 낸다.
  - 실패 경로: `--provider Claude`, `--provider "claude "`, `--provider gemini`는 종료 값 2로 거절되고 `canon.yaml`과 설치 결과가 바뀌지 않는다.
- F2:
  - `skills install --no-provider`가 설정에 빈 목록을 쓰고 설치 잠금에 제공자가 없다.
  - 실패 경로: `--no-provider --provider claude`는 거절되고 아무것도 쓰지 않는다.
- F3:
  - 선택을 준 `skills check`와 `skills doctor`가 그 제공자만 점검한다.
  - 실패 경로: 모르는 이름은 `skills install`처럼 거절된다.
- F4:
  - `{"provider":["claude","omp"]}`와 `{"noProvider":true}`가 플래그와 같은 결과를 낸다.
  - 실패 경로: `{"provider":[]}`, `{"provider":["none"]}`, `{"providers":"claude"}`는 거절되고 기록이 없다.
- F5:
  - `--providers claude,omp`는 종료 값 2로 거절되고 힌트의 명령 줄을 그대로 돌리면 통과한다.
  - 실패 경로: `--providers Claude`는 거절되고 힌트가 옵션 이름만 말한다. ==**옛 철자가 도움말과 생성 문서에 남아 있으면 시험이 실패한다.**==
- F6:
  - 네 명령의 선언이 같고 이름은 `possibleValues`에 한 번이다. 값표 전체가 파서와 맞는다.
  - 실패 경로: 한 명령이 다른 이름 목록이나 정규식 도메인을 선언하면 시험이 실패한다.
- F7:
  - CHANGELOG 표가 값 형태마다 한 행을 갖는다.
  - 실패 경로: 행이 빠지면 릴리스 확인 시험이 실패한다.
- `just atip-deep`과 생성 문서 drift 시험이 통과한다.

## Questions and assumptions

- 결정: 제공자 없음은 스위치 `--no-provider`이다. 열거 값 `none`은 `none` 단독이라는 값 사이 규칙을 선언할 수 없고, `--input`의 빈 배열은 canon이 거절한다. 이미 `--no-policy`와 `--no-agents-md`가 같은 모양이다.
- 결정: 모르는 이름은 열거 단계의 `USAGE`로 거절한다. 이 옵션 때문에만 내보내던 `INIT_ARGUMENT_INVALID`와 `SKILLS_ARGUMENT_INVALID`의 사유는 코드 표에서 걷는다. 설정 파일의 모르는 이름은 지금의 사유를 그대로 쓴다.
- 결정: 같은 이름을 두 번 적은 호출은 0.37처럼 받는다.
- 결정: 옛 `--providers`는 별칭 없이 거절만 한다(annex-schema S1 7).
- 결정: 설정 파일의 제공자 목록 키는 옵션이 아니어서 이 변경 밖이다.
- 결정: 이전에 받던 호출이 거절되므로 호출자에게 깨지는 변경이다. 스크립트, CI, 에이전트 안내문이 `--providers`를 쓰고 있으면 고쳐야 한다. CHANGELOG의 Breaking에 싣는다.

## Out of scope

- 설정 파일의 제공자 목록과 새 제공자 추가.
- 다른 옵션의 이름, 값 이름, 도메인(canon-0-38-schema-shape).
- 목록을 한 값으로 받는 열거 도메인.
- 옛 철자를 오래 살려 두는 별칭.
