---
waive:
  - rule: "prose.code-citation"
    section: "detail"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "한 변경에서 정해야 하는 사실이 많아 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "brief"
    reason: "한 변경에서 정해야 하는 사실이 많아 줄이면 결정이 빠진다"
  - rule: "prose.parenthetical"
    section: "detail"
    reason: "이름과 예시 값을 괄호로 붙이는 표기라 문장으로 풀면 길이만 는다"
  - rule: "prose.parenthetical"
    section: "questions"
    reason: "이름과 예시 값을 괄호로 붙이는 표기라 문장으로 풀면 길이만 는다"
---

# crates/atip: atip::plan이 계획, 효과, JCS digest, PLAN_STALE 관문, 확정 argv를 정한다

Source: https://github.com/journeyWorker/atip tag v1.2.0, spec 12.2-12.8과 8.3.2, approaches/rust.md §3(safety), issue 134 (둘째 댓글 1번과 저자 1번 답)

## Intent

파괴적 명령의 계획과 확정 관문은 canon의 `safety.rs`에만 있고, 다른 구현은 정렬된 키 JSON으로 따로 짠다. 그래서 같은 계획이 두 도구에서 다른 digest를 낸다. canon의 digest는 계약 12.6과 세 곳이 다르다. 효과만 해시하고 인수와 읽은 파일의 내용은 해시하지 않으며, serde의 필드 순서로 직렬화하고, `sha256:` 접두사 없이 소문자 16진만 쓴다. 계약은 digest가 효과와 그것을 정하는 모든 입력을 덮고 RFC 8785(JCS) 정규 JSON의 SHA-256이기를 권한다. 계약 저자는 효과만 해시하는 것은 규칙에 못 미치고, 고치는 일은 canon이 옮겨 갈 때 반드시 한다고 답했다.

## Desired outcome

- `atip::plan`이 `Plan { effects, inputs }`와 `Effect` 형식, 미리보기 보고서와 그 JSON Schema, digest, 확정 관문, 확정 argv를 낸다.
- digest는 `sha256:` 뒤에 소문자 16진 SHA-256이고, 대상은 호출자가 만든 계획의 JCS 정규 JSON이다. ==**같은 계획은 어느 도구에서나 같은 digest를 낸다.**==
- 확정 관문은 `--commit`과 `--plan-digest`가 짝이 아니거나 digest가 다르면 계약대로 거절한다. 거절은 아무것도 바꾸지 않는다.
- 이 변경은 crate만 바꾼다. canon의 `safety.rs`와 6곳의 호출 지점은 `canon-0-35-plan-jcs`가 옮긴다.

## User flow

1. F1 도구 작성자는 효과 목록과 입력으로 `Plan`을 만든다 → digest가 계약 형식으로 나온다 → 미리보기 보고서에 싣는다.
2. F2 도구는 미리보기 답을 만든다 → `planDigest`, `effects`, `committed: false`와 확정 호출을 가리키는 첫 `next` 항목이 나온다 → 호출자가 그 항목을 그대로 실행한다.
3. F3 호출자가 `--commit`과 `--plan-digest`로 부른다 → 도구가 계획을 다시 계산해 관문에 넘긴다 → ==**같으면 확정하고 다르면 `PLAN_STALE`이다.**==
4. F4 호출자가 둘 중 하나만 준다 → 계약대로 `USAGE`나 짝을 말하는 규칙의 코드로 거절된다 → 나머지를 채워 다시 부른다.
5. F5 도구는 확정 중에 계획이 바뀐 것을 안다 → 효과를 일부 낸 뒤라면 `STOPPED_MIDWAY`의 이유 `PLAN_STALE`을 낸다 → `meta.next`가 한 일을 읽는 명령을 이끈다.

## Step detail

### F1 계획과 digest

- ==**`Plan`은 `effects`(효과 목록)와 `inputs`(JSON 객체)를 가진다.**== `inputs`는 호출을 정규화한 인수 객체와, 계획이 읽은 상태나 파일 내용의 digest들이다. 이 값의 내용은 도구가 정하고 crate는 모양을 강제하지 않는다.
- `Effect`는 `action`, 선택적 `path`, `record`, `detail`이다(spec 12.4). 도구가 다른 멤버를 더할 수 있다.
- digest는 `sha256:` 뒤에 `{"effects":[…],"inputs":{…}}`를 JCS로 쓴 JSON의 SHA-256 소문자 16진 문자열이다. 효과의 순서는 도구가 정한 그대로이고, 순서가 바뀌면 digest가 바뀐다.
- JCS는 `atip::jcs` 모듈의 것이다. 키는 UTF-16 코드 단위 순서로 정렬하고, 수는 안전한 정수 범위의 정수만 받고, 그 밖의 수는 오류이다. 널과 빈 배열을 지우지 않는다. 이 모듈이 `canon-0-35-crate-cursor`에서 먼저 만들어져 있어야 한다.
- digest는 만료되지 않고 상태에 묶인다(spec 12.6). 다른 시스템이 digest를 계산하면 도구가 그대로 넘기고 비교는 거기서 한다. 그 경우는 crate가 digest를 만들지 않고 관문만 쓴다.

### F2 미리보기

- 미리보기 보고서의 멤버는 `planDigest`, `effects`, `committed`이다. 타입과 JSON Schema가 crate에 있고 도구가 보고서 스키마에 그것을 참조할 수 있다.
- 미리보기는 효과를 내지 않는다(spec 12.2). crate는 효과를 내는 코드를 갖지 않는다.
- 첫 `meta.next` 항목은 확정 호출이다. `argv`는 정규화한 인수에서 만든 옵션들에 `--commit`, `--plan-digest <digest>`, `--json`을 더한 것이다. 도구 이름 뒤에 `argv`를 이은 문자열이 `command`이다.
- 인수가 `--input -`나 파일로 왔으면 argv 옵션이나 인라인 `--input '<compact JSON>'` 단어 하나로 다시 쓴다. `--input -`는 싣지 않는다(spec 12.5).
- 값이 `-`로 시작하면 `--name=value`이다. 비밀은 인수로 받지 않는다(spec 12.5, 16.1). crate는 이를 확인하지 않는다.

### F3 관문

- 관문은 확정 여부, 호출이 준 digest, 다시 계산한 계획을 받는다. 확정이 아니면 미리보기이다. 확정이고 digest가 같으면 통과이다.
- digest가 다르면 오류는 `PLAN_STALE`이다. exit 2, 아무것도 바꾸지 않고, `meta.next`가 미리보기 호출이다. 힌트는 미리보기를 다시 실행하라고 말한다.
- 도구 쪽 digest를 계산할 수 없는 원격 시스템이 확정을 거절하는 경우는 도구 자신의 exit 2 코드이다. `PLAN_STALE`이 아니다(spec 12.7). crate는 이 경우를 이름으로 구분하지 않는다.

### F4 둘 중 하나만 준 경우

- 관문은 `--commit`만 있거나 `--plan-digest`만 있는 호출을 거절한다. 코드는 `USAGE`나 도구가 등록한 더 세분된 코드이다. 명령 문서에 `requires` 규칙이 있으면 그 규칙의 `code`이다. `field`는 빠진 쪽이다(spec 10.8.1).
- 도구가 규칙과 코드를 소유한다. crate는 두 옵션의 이름과 `requires` 쌍을 낸다.

### F5 확정 중의 변화

- crate는 `STOPPED_MIDWAY`의 이유로 쓸 `PLAN_STALE` 이름과 변환(이유, 한 일을 읽는 `next` 한 항목)을 돕는다. 도구가 효과마다 계획을 확인하는 일은 도구의 몫이다.
- 일부 효과를 낸 뒤의 변화는 exit 1이다. exit 2로 끝나면 안 된다(spec 8.2).

## Acceptance

- F1:
  - 같은 `Plan`이 `{"effects":…,"inputs":…}`의 JCS와 SHA-256으로 만든 `sha256:` digest를 낸다. digest가 64자 소문자 16진이고 접두사가 `sha256:`이다.
  - ==**키 순서만 다른 두 `inputs`가 같은 digest를 낸다.**== 효과의 순서나 값이 바뀌면 digest가 바뀐다. 한 입력의 한 글자를 바꾸면 digest가 바뀐다.
  - 실패 경로: 안전한 정수 밖의 수나 부동소수는 오류를 낸다.
- F2:
  - 미리보기 보고서의 `data`가 `planDigest`, `effects`, `committed: false`를 가지고 crate의 스키마를 통과한다.
  - 첫 `next` 항목의 `argv`가 `--commit`, `--plan-digest <digest>`, `--json`을 포함하고 `command`가 도구 이름 뒤에 `argv`를 이은 것이다. 그 `argv`를 `split_argv`로 되쪼갠 결과가 같다.
  - 실패 경로: `--input -`로 온 인수는 `--input -` 없이 argv 옵션이나 인라인 단어로 쓰이고, `-`로 시작하는 값은 `--name=value`로 쓰인다.
- F3:
  - 같은 digest로 확정하면 통과이고 다른 digest는 `PLAN_STALE`, exit 2이다. 오류에 `meta.next`용 미리보기 항목이 있다.
  - 실패 경로: 거절은 어떤 효과도 내지 않는다. 한 글자만 다른 digest도 거절된다. 접두사 없는 옛 형식 digest도 거절된다.
- F4:
  - `--commit`만, `--plan-digest`만 준 호출이 거절되고 `field`가 빠진 옵션이다. 도구가 `requires` 규칙의 코드를 넘기면 그 코드로 나온다.
  - 실패 경로: 둘 다 있고 digest가 맞으면 거절하지 않는다. 둘 다 없으면 미리보기이다.
- F5:
  - `PLAN_STALE`을 `STOPPED_MIDWAY`의 이유로 쓰는 변환이 종료 값 1과 한 일을 읽는 `next` 항목을 낸다.
- `cargo test --workspace`가 통과하고 canon과 `atip check`의 출력이 바뀌지 않는다. crate의 시험이 RFC 8785의 부록 시험 벡터를 포함한다.

## Questions and assumptions

- 결정(계약 저자 답): crate는 호출자가 만든 `Plan { effects, inputs }`를 해시한다. `inputs`는 정규화한 인수 객체와 계획이 읽은 상태나 파일 내용의 digest이다. 효과만 해시하는 것은 계약 12.6에 못 미친다.
- 결정(계약 저자 답): 이 digest 이동은 canon에서 선택이 아니다. 예전 형식으로 받은 미리보기는 이동 뒤에 `PLAN_STALE`이다. 이 점은 canon 쪽 변경(`canon-0-35-plan-jcs`)의 업그레이드 노트에 싣는다.
- 가정: `Effect`의 JSON Schema와 미리보기 보고서의 스키마는 canon의 보고서 스키마가 `$ref`하는 방식으로 쓴다. canon의 현재 스키마가 어떻게 되는지는 `canon-0-35-plan-jcs`가 정한다.
- 가정: 관문 함수의 서명은 구현자가 정한다. 도구가 `Plan`을 다시 계산하는 일과 원격 시스템이 digest를 계산하는 경우 모두 쓸 수 있어야 한다.
- 정책 질문은 없다.

## Out of scope

- canon의 `safety.rs`, 6곳의 호출 지점, canon의 digest 형식 변경(`canon-0-35-plan-jcs`).
- 파괴적 명령이 효과를 내는 일, 확정 중 효과마다 계획을 확인하는 일, 잠금.
- `--dry-run`, 비동기 명령, `--operation`(canon에 없다).
- JCS 구현이 부동소수 수 표현을 다루는 일. 안전한 정수만 받는다.
