---
waive:
  - rule: "prose.length"
    section: "detail"
    reason: "0.36에서 잰 옵션별 검사 표가 결정의 근거라서 줄이면 사실이 빠진다"
  - rule: "prose.length"
    section: "brief"
    reason: "세 규칙, 경계, 레짐 키, 변경 기록을 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
---

# 같은 이름의 옵션은 명령마다 같은 값 규칙을 쓴다: id, 한 줄 이름, 한 줄 문장

Source: canon 0.36.0 바이너리의 `canon schema` 옵션 정의를 잰 값, https://github.com/journeyWorker/atip 태그 v1.5.0 spec 9.7과 annex-domains.md V2, approaches/checker.md의 L1.one-meaning-values, canon-0-37-claim-1-5

## Intent

같은 이름의 옵션이 명령마다 다른 값 규칙을 갖는다. 한 명령이 받는 이유를 다른 명령은 거절하고, 검사가 없는 명령은 줄 바꿈이 든 값을 기록에 넣는다. 에이전트는 옵션 하나의 규칙을 명령마다 다시 읽어야 한다. 이 변경은 같은 이름의 옵션에 값 규칙 하나를 준다.

## Desired outcome

- 값 규칙은 id, 한 줄 이름, 한 줄 문장 셋이다. 각 옵션은 그 중 하나를 모든 명령에서 쓴다.
- ==**같은 이름의 옵션은 어느 명령에서나 같은 값 규칙을 쓰고, `canon schema`가 그 옵션에 도메인 하나만 낸다.**==
- 앞뒤 공백은 잘라서 받는다. 값 안의 줄 바꿈과 제어 문자만 거절한다. 값의 가운데는 고쳐 쓰지 않는다.
- `--regime`은 `dispatch begin`과 `retrieve`가 같은 도메인으로 선언한다.
- `--round`는 어디서나 1부터 시작하고 `--limit`의 최소값은 어디서나 1이다.
- `finding close`는 `--disposition open`을 거절한다.
- 새로 거절되는 값과 옮기는 방법이 CHANGELOG의 업그레이드 노트에 있다.

## User flow

1. F1 에이전트는 id 옵션 `--project`, `--role`, `--domain`에 값을 준다 → kebab 형태만 받는다 → ==**id를 소문자, 숫자, 하이픈으로 쓴다.**==
2. F2 에이전트는 이름 옵션 `--actor`, `--approver`, `--relayed-via`에 값을 준다 → 앞뒤 공백은 잘려 기록되고 제어 문자와 줄 바꿈은 거절된다 → 이름을 한 줄로 쓴다.
3. F3 에이전트는 문장 옵션 `--reason`, `--summary`, `--override-reason`에 값을 준다 → 앞뒤 공백은 잘려 기록되고 빈 값과 줄 바꿈은 거절된다 → 문장을 한 줄로 쓴다.
4. F4 에이전트는 `canon schema`로 `dispatch begin`과 `retrieve`의 `--regime`을 읽는다 → 두 명령이 같은 키 도메인을 선언한다 → 키를 한 가지 모양으로 쓴다.
5. F5 에이전트는 `--round`, `--limit`, `--disposition`에 경계 값을 준다 → 0과 `open`이 거절된다 → 경계 안의 값을 쓴다.
6. F6 에이전트는 `canon schema`로 옵션 정의를 읽는다 → 같은 이름의 옵션이 모든 명령에서 같은 도메인이다 → 옵션 정의를 한 번 읽어 둔다.
7. F7 사람은 CHANGELOG를 읽는다 → 새로 거절되는 값과 옮기는 방법이 있다 → 호출자를 고친다.

## Step detail

### F1 id

- id는 소문자와 숫자 덩어리를 하이픈 하나로 이은 것이다. `--project`는 0.36에서 하이픈으로 끝나거나 하이픈이 겹친 값도 받았다. 새 규칙은 그런 값을 거절한다.
- `--domain`은 `subject new`만 kebab으로 검사했다. `query`와 `retrieve`도 같은 규칙으로 검사한다. `--role`은 이미 kebab이다.
- `divergence accept --role`은 id 규칙을 지난 뒤 reviewer만 받는다. 이 거절은 권한이어서 그대로 둔다.
- 설정 파일의 spec root id는 이 변경이 바꾸지 않는다. 구현자가 canon과 알려진 저장소의 root id를 재고, kebab이 아닌 것이 있으면 멈추고 사람에게 알린다.

### F2 이름

- 이름은 앞뒤 공백(탭과 줄 바꿈 포함)을 잘라 받는다. 자른 뒤 비면 거절한다. 자른 값 안에 줄 바꿈, 탭, 터미널 이스케이프 같은 제어 문자나 U+2028, U+2029가 있으면 거절한다. 가운데 스페이스는 그대로 둔다.
- 서명에 드는 이름은 자른 값으로 서명 바이트를 만든다. `evidence approval-payload`와 `evidence add`가 같은 이름을 쓴다.
- 릴레이 승인은 승인 줄에 `<role> <actor> via <tool>`로 적힌다. `--relayed-via`와 릴레이의 `--actor`는 한 낱말이어야 하고 이 검사는 도메인과 별개로 남는다. 이 검사는 `--relayed-via`를 받는 네 명령에서 같다.
- 기본값과 필수 여부는 명령마다 그대로다. `--actor`의 기본값은 `canon`, `canon-scaffold`, 없음이다.
- 선언이 없는 명령 중에도 핸들러가 빈 값이나 줄 바꿈을 따로 거절하는 곳이 있다. 구현자가 명령마다 0.36의 실제 동작을 재서 CHANGELOG 표의 "0.36이 받던 값"을 채운다. `review scenario`와 `review change`는 도움말이 `--actor`의 줄 바꿈 거절을 적지만 0.36은 줄 바꿈이 든 값을 받는다. 도움말은 새 규칙에 맞춘다.

0.36에서 `canon schema`가 선언한 이름 옵션의 도메인:

| 옵션 | 선언된 도메인 | 명령 |
|---|---|---|
| `--actor` | 한 낱말 | `brief approve`, `policy approve` |
| `--actor` | 줄 바꿈 거절 | `evidence add`, `evidence audit` |
| `--actor` | 빈 값과 줄 바꿈 거절 | `finding add` |
| `--actor` | 선언 없음 | 그 밖의 14개 명령 |
| `--approver` | 선언 없음 | `brief approve`, `policy approve`, `strategy approval-payload`, `divergence accept`, `migrate` |
| `--approver` | 앞뒤 공백과 CR, LF 거절 | `evidence add` |
| `--approver` | 앞뒤 공백과 모든 줄 바꿈 거절 | `evidence approval-payload` |
| `--relayed-via` | 한 낱말 | `brief approve`, `policy approve` |
| `--relayed-via` | 선언 없음 | `divergence accept`, `migrate` |

### F3 문장

- 문장은 앞뒤 공백(탭과 줄 바꿈 포함)을 잘라 받는다. 자른 뒤 비면 거절한다. 자른 값 안에 줄 바꿈 문자가 있으면 거절한다. 줄 바꿈 문자는 U+000A, U+000B, U+000C, U+000D, U+0085, U+2028, U+2029이다. 탭과 다른 제어 문자는 막지 않는다.
- `strategy rollback`의 서명은 이유의 해시를 담는다. 해시는 자른 이유로 만든다.
- 거절 코드는 그 명령이 그 옵션에 이미 쓰던 코드이고, 검사가 없던 명령은 자기 주제의 인자 오류 코드를 쓴다. 새 코드 이름은 없다.

0.36에서 `canon schema`가 선언한 문장 옵션의 도메인:

| 옵션 | 선언된 도메인 | 명령 |
|---|---|---|
| `--reason` | 선언 없음 | `divergence add`, `divergence defer`, `strategy rollback` |
| `--reason` | NUL과 빈 값 거절 | `divergence accept` |
| `--reason` | 줄 바꿈과 빈 값 거절 | `finding close` |
| `--reason` | CR, LF와 빈 값 거절 | `subject move` |
| `--summary` | 줄 바꿈 거절 | `evidence add`, `evidence audit` |
| `--summary` | 빈 값과 줄 바꿈 거절 | `finding add` |
| `--summary` | 선언 없음 | `subject new` |
| `--override-reason` | 빈 값과 줄 바꿈 거절 | `evidence add`, `evidence audit` |
| `--override-reason` | 빈 값과 CR, LF 거절 | `subject move` |

### F4 레짐 키

- 레짐 키는 `/`로 나뉜 네 마디이다. 앞의 세 마디는 비어 있지 않고 공백이 없다. 마지막은 소문자 16진수 6자에서 64자이다. `retrieve`가 이 규칙을 도메인으로 선언한다.
- `dispatch begin`은 같은 키를 값 파서로 이미 거절하지만 `canon schema`에는 도메인이 없다. 같은 규칙을 같은 철자로 선언한다. 이 명령의 거절은 지금과 같게 남는다. `--role`이 키의 첫 마디와 같아야 하는 검사도 그대로다.

### F5 경계

- `--round`: `divergence add`, `divergence defer`, `divergence resolve`는 기본값 1을 두고 최소를 0에서 1로 올린다. `finding add`와 `finding close`는 이미 필수이고 최소 1이다.
- `--limit`: `retrieve`가 0을 받았다. 최소 1로 올린다. 기본값은 명령마다 그대로다.
- `finding close`는 fixed, rejected, deferred만 받고 `--disposition open`을 거절한다. `finding add`의 `--disposition open`은 그대로다.
- ==**`--round 0`, `--limit 0`, `finding close --disposition open`은 거절되고 아무것도 기록하지 않는다.**==

### F6 정의 하나

- 옵션 이름마다 그 이름을 쓰는 모든 명령의 도메인이 같다. 다른 것은 기본값, 필수 여부, 열거 값의 부분집합뿐이다. 모든 명령 문서를 훑는 시험이 이를 고정한다.
- `--kind`의 값 집합이 명령마다 다른 점은 이 변경 밖이다. 계약의 L1.one-meaning-values가 SHOULD로 권고한다. 시험의 허용 목록에 이름으로 적는다.
- 도메인은 1.5 어휘로 쓴다. 이름은 제어 문자와 줄 바꿈을 막는 클래스 `[^\p{Cc}\p{Zl}\p{Zp}]`로 쓴다. 문장은 줄 바꿈이 없는 문자 클래스로 쓴다.

### F7 변경 기록

- CHANGELOG의 Breaking 항목에 옵션, 명령, 0.36이 받던 값, 0.37의 처리를 표로 싣는다. 새로 거절되는 값마다 옮기는 방법을 적는다.
- 잘려서 기록되는 값을 한 항목으로 적는다.

## Acceptance

- F1:
  - `--project`, `--role`, `--domain`이 kebab만 받는다.
  - 실패 경로: `--project a-`와 `--project a--b`는 종료 값 2로 거절되고 아무것도 쓰지 않는다. `query --domain Dev`도 같다.
- F2:
  - `--actor " alice "`는 `alice`로 기록된다. 서명에 드는 승인자도 잘린 값이다.
  - 실패 경로: 값 가운데에 줄 바꿈이나 탭이 든 `--actor`, `--approver`와 공백이 든 `--relayed-via`는 거절되고 기록이 없다. 빈 값과 공백뿐인 값도 같다.
- F3:
  - `--reason`, `--summary`, `--override-reason`이 앞뒤 공백을 잘라 받는다. 이전에 검사가 없던 명령도 같다.
  - 실패 경로: 값 가운데에 줄 바꿈이 든 값과 공백뿐인 값은 명령마다 자기 코드로 거절된다. `strategy rollback --reason`도 같다.
- F4:
  - 올바른 키는 두 명령이 같게 받는다.
  - 실패 경로: 마디가 셋인 키와 공백이 든 키를 `dispatch begin`이 `retrieve`와 같은 이유로 거절한다.
- F5:
  - 경계 안의 값 `--round 1`, `--limit 1`, `--disposition fixed`는 받는다.
  - 실패 경로: `--round 0`, `--limit 0`, `finding close --disposition open`은 종료 값 2이고 기록이 없다.
- F6:
  - 모든 명령 문서를 훑은 시험이 같은 이름의 옵션에 도메인이 하나뿐임을 확인한다.
  - 실패 경로: 한 명령이 같은 이름에 다른 도메인을 선언하면 시험이 실패한다.
- F7:
  - CHANGELOG 표가 새로 거절되는 값마다 한 행을 갖는다.
  - 실패 경로: 표에 빠진 거절이 있으면 릴리스 확인 시험이 실패한다.
- ==**`just atip-deep`이 1.5로 통과하고 생성 문서 drift 시험이 통과한다.**==

## Questions and assumptions

- 가정: id는 `--project`, `--role`, `--domain`, 이름은 `--actor`, `--approver`, `--relayed-via`, 문장은 `--reason`, `--summary`, `--override-reason`이다. 틀리면 사람이 고친다.
- 질문: `finding close`가 `open`을 거절하면 닫힌 finding을 다시 여는 명령이 없다. 기본값: 다시 여는 명령을 더하지 않고 새 finding을 기록한다. 되살림을 말하던 시나리오는 이 동작으로 고친다.
- 질문: 릴레이의 한 낱말 검사를 네 명령에 같게 적용한다. 지금은 승인 두 명령만 검사한다. 기본값: 같게 적용한다.
- 가정: 문장 규칙은 줄 바꿈만 막는다. 탭과 터미널 이스케이프도 막으려면 사람이 알려 준다.
- 가정: `--session`, `--reviewer-*`, `--evaluator*`, `--note`, `--tracking`, `--detail`은 정의가 하나라서 이 변경 밖이다. 앞뒤 공백을 거절하는 것들은 "잘라서 받는다"와 다르다. 맞추려면 사람이 알려 준다.
- 가정: 저장된 기록은 바꾸지 않는다. spec root id의 모델 문법도 그대로다. 옵션만 kebab으로 조인다.
- 정책 질문: 이전에 받던 값이 거절되므로 호출자에게 깨지는 변경이다. 사람이 이 거절 목록을 승인한다.

## Out of scope

- 기본값과 필수 여부의 통일.
- `--kind`의 값 집합.
- `--approval-at`과 `--providers`(canon-0-37-option-forms).
- 저장된 기록의 이전.
- 닫힌 finding을 다시 여는 새 명령.
