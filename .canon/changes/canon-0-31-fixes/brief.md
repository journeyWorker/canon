# 0.31 0.30에서 찾은 버그 수정

<!-- canon:section intent role=context -->
## Intent

0.30 스펙을 쓰며 동작 여섯 가지가 문서나 다른 명령과 어긋나는 것을 재현했다. `demo init`은 설정이 있는 디렉터리의 정책을 덮어쓰고, `gate install-hooks`는 없는 디렉터리를 만든다. `context-pack`은 틀린 id에 "없음"이라 답하고, `evidence approval-payload`는 없는 시나리오의 서명 대상을 만든다. ADAPTER_REFUSED와 `strategy rollback`은 종료 코드가 어긋난다.

0.31 순서: CI → 승인과 adopt → 버그 수정 → brief 포맷 → CLI id → 검색. 여섯 brief는 함께 승인받고, 승인과 adopt 전에는 고치지 않는다.

<!-- canon:section outcome role=normative -->
## Desired outcome

- ==**아래 여섯 동작에서 없는 대상과 잘못된 입력은 종료 2이고, 종료 1은 검사 결과나 거절된 판정에만 남는다.**==
- 여섯 동작이 문서와 서로 같고, 거절된 호출은 파일을 바꾸지 않는다.
- 바뀐 동작마다 시나리오가 있고, 지금 동작을 적은 것은 고쳐 쓴다. 이 변경의 단계로 태그한다.

<!-- canon:section flow role=normative -->
## User flow

1. F1 사용자가 설정 파일이 있는 디렉터리에서 `canon demo init`을 실행한다 → canon이 먼저 설정 파일을 본다 → ==**거절되고 디렉터리는 그대로다.**==
2. F2 사용자가 없는 디렉터리를 `--repo`로 주고 `canon gate install-hooks`를 실행한다 → canon이 디렉터리를 확인한다 → 거절된다.
3. F3 사용자가 틀린 팩 id로 `context-pack show`나 `verify`를 실행한다 → canon이 형식을 본다 → 사용법 오류를 받는다.
4. F4 서명자가 선언되지 않은 시나리오로 `evidence approval-payload`를 실행한다 → canon이 시나리오를 찾는다 → 거절되고 서명할 바이트가 없다.
5. F5 사용자가 없거나 깨진 응답 파일로 `adapter validate`나 `authorize`를 실행한다 → canon이 검사한다 → 경로마다 정해진 종료 코드를 받는다.
6. F6 승인자가 없는 전략 id로 `strategy rollback`을 실행한다 → canon이 id를 먼저 찾는다 → `approval-payload`와 같은 오류와 종료 2를 받는다.

<!-- canon:section detail role=normative -->
## Step detail

### F1 demo init

- 지금은 `.gitignore`에 canon 줄이 모두 있을 때만 거절한다. 대상 디렉터리에 설정 파일이 있으면 쓰기 전에 INIT_CONFIG_EXISTS, 종료 2로 거절한다. `canon init`의 `.gitignore` 갱신은 그대로다.
- 시나리오: authoring.demo.02의 Given을 "`.gitignore`에 canon 줄이 없는 설정 파일"로 넓힌다. 정책 파일이 그대로임도 단언한다.

### F2 gate install-hooks

- 없는 `--repo`는 `--dry-run`에서도 HOOKS_INSTALL_FAILED, 종료 2로 거절하고 아무것도 만들지 않는다.
- 사람이 읽는 출력은 `--dry-run`에서도 에이전트 훅 파일 둘과 훅 스크립트를 이름으로 말한다. ==**git 훅은 설치하지 않았고, 스크립트를 git의 pre-commit 훅 자리에 복사하거나 훅 실행기에서 부르면 된다고 적는다.**==
- JSON 보고서는 세 필드를 지키고, 스키마 설명이 `installed`는 에이전트 훅 파일이 바뀐 것, 스크립트 필드는 스크립트를 쓴 것이라 말한다.
- `schema gate install-hooks`의 종료 2 설명에서 CONFIG_MISSING을 뺀다.
- 시나리오: gate.check.105에 출력 단언을, 107에 없는 디렉터리 사례를 더한다. 106은 JSON만 봐서 그대로다.

### F3 context-pack 팩 id

- 형식은 `sha256:` 뒤 소문자 16진수 64자다. 그 밖의 값은 대문자·빈 값까지 `show`와 `verify` 모두 USAGE, 종료 2이고 피연산자 이름을 말한다.
- 형식이 맞는 없는 id는 CONTEXT_PACK_NOT_FOUND, 종료 2다. dispatch.manifest.17과 19는 그대로다.
- 시나리오: show와 verify에 형식 오류를 더한다.

### F4 evidence approval-payload

- 시나리오 색인에 없는 `--scenario-id`는 종료 2로 거절한다. 메시지는 id와 스펙 루트를 말하고 힌트는 `canon inventory sync`를 안내한다. 새 오류 코드 EVIDENCE_SCENARIO_NOT_INDEXED를 쓴다.
- 시나리오: 실패 시나리오를 더한다. evidence.add.121·122는 그대로다.

### F5 ADAPTER_REFUSED 종료 코드

| 호출 | 입력 | 종료 |
|---|---|---|
| validate | 응답 파일 없음·읽을 수 없음 | 2 |
| validate | 봉투가 규칙을 어김 | 1 |
| authorize | 응답 파일 없음·읽을 수 없음 | 2 |
| authorize | 봉투가 규칙을 어기거나 정책 파일이 깨짐 | 2 (지금 1) |

- authorize의 종료 1은 ADAPTER_EXECUTION_NOT_AUTHORIZED에만 남는다. 그 종료 표의 2와 `schema --error ADAPTER_REFUSED`가 이 구분을 적는다.
- 시나리오: dispatch.manifest.10, 12, 13은 그대로다. authorize의 깨진 봉투와 `schema --error` 표를 더한다.

### F6 strategy rollback

- 형식이 맞는 id를 서명 파일을 읽기 전에 먼저 찾는다. 없으면 다른 오류보다 앞서 LEARN_STRATEGY_NOT_FOUND, 종료 2다. 서명 검증 실패는 LEARN_ROLLBACK_FAILED, 종료 1이다. `schema`의 오류 코드가 이를 따른다.
- 시나리오: 없는 id를 더한다. learn.promotion.13은 그대로다.

<!-- canon:section acceptance role=normative -->
## Acceptance

- F1: `.gitignore`에 canon 줄이 없는 설정 디렉터리에서 `demo init`이 종료 2로 거절하고 트리가 같다. ==**정책 파일 내용이 그대로다.**== 실패 경로: 설정 파일이 없는 디렉터리는 지금처럼 만들어진다.
- F2: 없는 `--repo`는 `--dry-run`까지 종료 2이고 디렉터리가 안 생긴다. 출력이 git 훅 미설치를 말한다. 실패 경로: 파일 경로는 지금처럼 거절된다.
- F3: `sha256:abc`와 빈 값은 `show`·`verify` 모두 USAGE, 종료 2. 실패 경로: 형식이 맞는 없는 id는 CONTEXT_PACK_NOT_FOUND다.
- F4: 선언되지 않은 시나리오는 종료 2이고 stdout이 비며 트리가 같다. 실패 경로: 선언된 것은 바이트를 낸다.
- F5: 표의 네 행이 종료 코드와 일치하고 `schema`가 같은 표와 코드를 보여 준다. 실패 경로: 예산 없는 정책의 authorize는 종료 1이다.
- F6: 없는 id는 서명 파일이 없거나 틀려도 LEARN_STRATEGY_NOT_FOUND, 종료 2이고 쓰기가 없다. 실패 경로: 있는 전략에 틀린 서명은 종료 1이다.

<!-- canon:section questions role=question -->
## Questions and assumptions

- 질문: authorize의 깨진 입력을 종료 2로 바꿔도 되는가? validate는 봉투 검사가 일이라 1을 지킨다. 가정: 바꾼다. 대안은 1을 두고 `schema --error`만 고치는 것이다.
- 질문: `install-hooks`가 git 훅을 직접 설치해야 하는가? 가정: 아니다. 스크립트를 연결하지 않고 두는 것이 설계라 출력만 고친다.
- 질문: F4에서 색인이 오래된 저장소는? 가정: 같은 안내로 거절한다.
- 가정: F3의 소문자 규정은 지금보다 엄격하다. 어댑터 응답의 검사는 그대로다.
- 가정: `demo init`의 거절 조건은 설정 파일뿐이다. 정책 파일만 있으면 지금처럼 덮어쓴다.

<!-- canon:section out-of-scope role=context -->
## Out of scope

- `gate install-hooks`가 설정 없는 저장소에 설치하는 동작. 설정 전 저장소용으로 일부러 둔 예외다.
- `evidence add`가 색인에 없는 시나리오 id를 받는 동작. 리포트 케이스용이다.
- `canon init`의 `.gitignore` 갱신, 서명 검증 실패의 종료 1, 여섯 밖에 남은 같은 유형의 종료 1(예: `strategy promote`의 없는 id).
