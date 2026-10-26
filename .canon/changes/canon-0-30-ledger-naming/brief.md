# 0.30 레저 파일 이름 규칙

Source: docs/design/ledger-layout.md

<!-- canon:section intent role=context -->
## Intent

레코드 이름은 종류마다 규칙이 다르다. 일부 종류만 project를 넣어 다른 project의 evidence와 이름이 겹칠 수 있고, evidence 이름에 id 표시가 없다. 구분자, 숫자 자릿수, 특수문자 처리도 종류마다 다르다. scenario·review·divergence만 `area=` 단계를 쓰지만 읽는 코드는 없다. evidence는 디스크·레코드에서 `evidence_record`, CLI에서 `evidence`로 이름이 둘이다. 보고서와 archive는 이름 모양을 가정하고 이름을 다시 읽는다. 레포가 적은 지금 이름을 한 규칙으로 맞춘다.
<!-- canon:waive prose.code-citation intent "evidence_record는 디스크와 레코드에 남은 종류 이름이라 사용자가 보는 이름 자체다" match="evidence_record" -->

0.30 순서: 계획 이전 → 스펙 보강 → 코드 정리 → clippy CI → 이 변경 → 레코드 연결. 여섯 brief는 함께 승인받는다.

<!-- canon:section outcome role=normative -->
## Desired outcome

- ==**모든 저장소가 한 규칙을 따른다.**== 저장소는 git 레저, archive, staging, 플러그인 오버레이, sqlite·Postgres 키, R2 객체 키다. 규칙은 종류 디렉터리 하나, 종류별 키 구성요소를 `_`로 이은 뒤 내용 해시 8자리로 끝내는 것이다.
- 종류 이름은 어디서나 하나다. evidence는 디스크, 레코드, 스키마, 설정, 보고서, CLI 모두 `evidence`.
- 이름을 다시 읽는 곳은 해시를 레코드 본문에서 얻는다. 같은 데이터에서 보고서와 대시보드의 집계가 이름 변경 전후로 같다.
- 레코드 순서는 본문 필드로 정해진다.
- 0.29 레포는 `canon migrate` 하나로 옮겨지고, 옮기기 전 모든 명령이 무엇을 해야 하는지 안내한다.
- 이름이 들어간 출력은 새 규칙을 따르고 시나리오로 정의된다. 출력은 조회 id 열, 커서, 오류 메시지, tier age와 export/purge id, 쓰기 결과 줄이다.

<!-- canon:section flow role=normative -->
## User flow

1. F1 에이전트가 레코드를 쓰는 명령을 실행한다 → canon이 레코드를 저장한다 → 출력에 새 규칙의 경로가 나오고 레코드가 그 이름으로 저장된다.
2. F2 0.29 레포에서 사용자가 canon 명령을 실행한다 → canon이 옛 이름 파일이나 옛 설정 키를 찾는다 → 읽는 명령은 설정 오류 없이 경고와 함께 `canon migrate`를 다음 단계 첫 줄로 내고, 쓰는 명령은 같은 안내로 거절된다.
3. F3 사용자가 `canon migrate`를 실행한다 → canon이 옮길 파일, 다시 쓸 레코드와 설정, 다시 매길 데이터베이스 키를 계산한다 → ==**아무것도 쓰지 않고 계획과 계획 digest를 보여 준다.**==
4. F4 사용자가 그 digest로 `canon migrate --commit`을 실행한다 → canon이 계획대로 옮기고 다시 쓴다 → 상태, 판정, 조회, 보고서 결과가 옮기기 전과 같다(이름과 종류 이름 제외).
5. F5 에이전트나 플러그인이 키가 하나도 없는 evidence를 쓰려 한다 → 모델 검증이 거절한다 → 어느 저장소도 바뀌지 않는다.

<!-- canon:section detail role=normative -->
## Step detail

### F1 새 이름

- 형식은 `kind=<종류>/<구성요소>_…_<해시8>.json`. 구성요소는 종류별 표 하나가 정한다(설계 노트의 표).
- project가 있는 종류는 project로 시작하고, 값이 없으면 `-`.
- 키가 여러 종류일 수 있으면 값 앞에 `scenario`, `change`, `task`, `run`을 쓴다. evidence의 우선순위는 지금과 같다(task, scenario, run).
- review 이름에는 40자리 커밋 sha를 넣지 않는다.
- 정수는 패딩 없이 쓴다. 정렬은 본문 필드로 하고, 같은 값끼리는 구성요소를 표 순서로 비교한 뒤 본문 해시로 정한다.
- 타입이 정해진 id는 정규 소문자로 쓴다. run id 같은 ULID는 대소문자를 구분하지 않으므로 소문자로 쓴다.
- 자유 문자열 id에서 `[a-z0-9.-]` 밖 문자(대문자, `_` 포함)는 `+`와 소문자 hex 두 자리로 바꾼다. 모든 저장소에서 같은 문자열로 남는다.
- 파일 이름이 200바이트를 넘으면 그 자유 문자열 구성요소를 `h`와 그 값의 SHA-256 앞 16자리로 바꾼다. 원래 값은 본문에 있다.
- 조회, 커서, 오류 메시지는 escape 전의 구성요소 값을 쓴다.
- 해시는 정규화 JSON의 SHA-256 앞 8자리다. ==**어느 저장소든 같은 키에 다른 내용이 있으면 쓰기를 거절한다.**== git과 R2는 바이트를, sqlite와 Postgres는 정규화한 JSON을 비교한다. 레저에 새로 쓸 경로가 archive에 다른 내용으로 있으면 그것도 거절한다.
- 여러 레코드를 한 번에 쓰다가 충돌이 나면 그 묶음의 레코드는 하나도 남지 않는다.

### F2 옛 레포 감지

- 옛 이름 파일은 전용 위반 코드로 보고하고 hint는 `canon migrate`다. 옛 파일은 판정에 쓰지 않는다.
- `canon.yaml`의 `evidence_record` 키는 `canon migrate`로 보내는 용도로만 읽는다.
- 옛 레포 경고와 다음 단계 표시는 0.27 상태 감지와 같은 방식이다.
- 옮기기 전에는 레코드를 쓰는 명령이 같은 hint로 거절된다. 새 이름 파일이 옛 파일 옆에 생기지 않게 하기 위해서다.

<!-- canon:waive prose.code-citation detail "evidence_record는 canon.yaml에 남은 옛 설정 키 이름이라 사용자가 보는 이름 자체다" match="evidence_record" -->
<!-- canon:waive prose.code-citation detail "canon.yaml은 사용자가 편집하는 설정 파일 이름이다" match="canon.yaml" -->
<!-- canon:waive prose.code-citation detail "PLAN_CHANGED는 사용자가 받는 오류 코드다" match="PLAN_CHANGED" -->

### F3 미리보기

- 레저, archive, staging, 오버레이 파일, `canon.yaml` 키, `.gitignore` 예외 줄, sqlite 행, 설정된 Postgres·R2 키를 계산한다.
- staging의 레코드도 함께 옮긴다. 중단된 promote가 남긴 staging(커밋된 레코드가 그 staging 본문을 가리키는 경우)이 있으면 거절한다.
- 옮길 경로에 커밋하지 않은 변경이 있으면 거절한다.

### F4 적용

- 옮기기 전에 모든 evidence 파일의 옛 키, 옛 12자리 해시, 옛 본문을 계획에 기록한다. 0.23 이전 리뷰 핀 변환은 이 기록과 0.29 이름 계산 방식의 고정된 사본으로 판정한다. 그래야 종류 값이 바뀐 뒤에도 옛 핀을 맞출 수 있다.
- 이 기록을 남긴 뒤 이름 변경을 기존 migrate 단계보다 먼저 한다.
- 파일은 git 이동으로 옮기고, evidence 본문의 종류 값을 바꾸고 해시를 다시 계산한다.
- 다시 쓰는 파일과 sqlite 파일은 백업한다. 되돌리기는 백업 복원과 이동 되돌리기이고, 데이터베이스·R2 키는 계획에 기록된 옛 키로 되돌린다.
- 계획이 미리보기와 다르면 `PLAN_CHANGED`로 거절한다. 다시 실행하면 옮길 것이 없다.
- 레코드 안의 경로 문자열(자유 서술 파일 참조 등)은 그대로 둔다.

### F5 키 없는 evidence

- CLI는 이미 task id나 scenario id를 요구한다. 같은 규칙을 모델에 둬서 플러그인 동기화, ingest, 다른 tier 쓰기도 거절한다.

<!-- canon:section acceptance role=normative -->
## Acceptance

- F1: 15개 종류(오버레이 포함) 각각 쓰기 출력과 저장된 이름이 규칙대로다. escape가 필요한 id와 run id가 git과 R2에서 같은 키로 왕복한다. 실패 경로: ==**같은 키에 다른 내용을 쓰면 git, R2, sqlite, Postgres 모두 거절하고 그 묶음은 아무것도 남지 않는다.**==
- F2: 0.29 fixture 레포에서 `status`, `gate check`, `context`가 설정 오류 없이 경고하고 `canon migrate`를 첫 다음 단계로 낸다. 실패 경로: 옮기기 전 `evidence add`는 같은 hint로 거절되고 아무것도 쓰지 않는다. 새 레저에 옛 이름 파일 하나를 섞으면 그 파일만 전용 코드로 보고된다.
- F3: 미리보기 후 작업 트리, sqlite 파일, 설정이 바이트 단위로 같다. 실패 경로: 중단된 promote의 staging이 있으면 거절된다.
- F4: canon 자기 레포를 옮긴 뒤 `gate check`, `status`, 조회(`--json`)가 같은 레코드와 같은 판정을 낸다. 달라도 되는 것은 이름, 종류 이름, `at`이 같은 레코드끼리의 순서뿐이다. 보고서·대시보드 집계가 같고, 새로 쓴 evidence가 `git status`에 나타난다.
- F4 옛 핀: 0.23 이전 핀이 있는 fixture에서 옮긴 뒤에도 그 리뷰들의 현재/오래됨 판정이 같다.
- F4 실패와 되돌리기: 다른 digest로 적용하면 거절되고 아무것도 바뀌지 않는다. 되돌리기를 fixture에서 실행해 옮기기 전 상태와 같아짐을 확인한다.
- F5: 키 없는 evidence를 저장소 계층과 플러그인 경로로 쓰면 거절된다.
- 이름 변경이 바꾸는 기존 시나리오와 저장소 안내 스킬 문서를 새 내용으로 고친다. 종류 이름을 고정한 시나리오도 포함한다. 기대 출력이 바뀌는 conformance 케이스를 모두 새로 만들어 리뷰한다.

<!-- canon:waive prose.length brief "모든 저장소와 마이그레이션 순서를 다루는 변경이라 두 차례 리뷰가 요구한 조건을 빼면 사실이 빠진다" -->

<!-- canon:section questions role=question -->
## Questions and assumptions

- 질문(정책): canon의 계획 규칙은 마이그레이션을 확장, 이전, 축소의 세 변경으로 나누라고 한다. 이 변경은 하나로 한다. 옛 이름은 `canon migrate`만 읽는다(0.27→0.28 전환과 같은 방식). 이 규칙 예외를 승인할까? 가정: 승인한다. canon을 쓰는 레포가 적다.
- 질문: R2나 Postgres에 실제 데이터를 둔 레포가 있는가? 가정: 없다. 그 tier의 키 변경은 코드와 테스트로 확인하고 실제 데이터 이동은 하지 않는다.
- 가정: 0.29에서 받은 조회 커서는 0.30에서 거절되고, hint가 처음부터 다시 조회하라고 안내한다.

<!-- canon:section out-of-scope role=context -->
## Out of scope

- spec 코퍼스 배치: 사람이 작성하는 트리라 그대로 둔다.
- 다른 도구의 레저를 읽는 ingest 어댑터의 입력 배치.
- 레코드 안 경로 문자열 수정.
- id 플래그 이름 변경.
