# 0.30 기존 기능 스펙 보강

<!-- canon:section intent role=context -->
## Intent

canon은 자기 기능을 시나리오 659개로 정의하지만 빈 곳이 있다. CLI 명령 76개 중 17개는 시나리오가 하나도 없고, 7개는 실패 경로가 없다. subject 둘은 시나리오가 0개로 `specced`에 멈춰 있고, 예제 게임 시나리오 10개는 subject가 없다. 0.30의 코드 정리는 동작을 지켜야 하는데, 스펙이 없는 동작은 지켰는지 확인할 수 없다. 정리보다 먼저 지금 동작을 스펙으로 고정한다.

0.30 순서: 계획 이전 → 이 변경 → 코드 정리 → clippy CI → 레저 이름 변경 → 레코드 연결. 여섯 brief는 함께 승인받는다.

<!-- canon:section outcome role=normative -->
## Desired outcome

- 모든 CLI 명령에 성공 경로와 실패 경로 시나리오가 각각 하나 이상 있다.
- 시나리오는 지금 동작을 그대로 적는다. ==**이 변경으로 바뀌는 동작은 없다.**==
- 모든 시나리오가 subject에 속하고, 시나리오가 0개인 subject가 없다.
- 새 시나리오마다 그 id를 이름에 단 테스트가 있고, evidence와 다른 세션의 리뷰가 붙는다.

<!-- canon:section flow role=normative -->
## User flow

1. F1 에이전트가 디스패치 입력 명령을 실행한다 → canon이 입력을 검사하고 기록하거나 보여 준다 → 성공하면 결과와 종료 0을 받는다. ==**잘못된 입력이면 오류 코드와 종료 2를 받고 작업 트리는 그대로다.**==
2. F2 사용자가 저장소 관리 명령을 실행한다 → canon이 미리보기나 검사를 한다 → 계획이나 판정을 받고, 잘못된 인자나 바뀐 계획이면 거절된다.
3. F3 사용자가 게이트·정책 명령을 실행한다 → canon이 훅을 설치하거나 자체 검사를 하거나 정책을 보여 준다 → 결과를 받고, 설치할 수 없거나 정책이 없으면 정해진 오류를 받는다.
4. F4 사용자가 조회 명령을 실행한다 → canon이 저장된 상태를 읽는다 → 결과를 받고, 없는 id나 잘못된 인자면 오류 코드를 받으며 아무것도 쓰이지 않는다.
5. F5 사용자가 데모 명령을 실행한다 → canon이 데모 레포를 만들거나 데모 리뷰를 돌린다 → 결과를 받고, 이미 있는 디렉터리 같은 거절 조건에서는 오류를 받는다.
6. F6 예제 게임을 실행하는 사람이 이동, 적, HUD, 레벨, 세션을 다룬다 → 게임이 반응한다 → 각 동작이 subject에 속한 시나리오로 정의되고 실패 경로도 있다.

<!-- canon:section detail role=normative -->
## Step detail

### F1 디스패치 입력 명령

F1–F5 공통 규칙:

- ==**명령마다 성공 경로 하나와 사용자 실수에 따른 실패 경로 하나 이상을 적는다.**== 사용자 실수는 잘못된 인자, 없는 id, 사람 전용 행위 등이다. 쓰는 명령은 거절 뒤 작업 트리가 그대로인지도 적는다.
- 관찰할 수 있는 것만 적는다: 종료 코드, 오류 코드, 출력 필드, 파일 변화. 레저 파일 이름은 적지 않는다. 레저 이름 변경이 바꾸는 값이기 때문이다.
- 시나리오는 이 변경의 flow 단계로 태그하고, 명령이 속한 subject에 넣는다.
- 레포 테스트가 명령 명세의 모든 leaf와, leaf마다 성공·실패 시나리오 id를 적은 표를 대조한다. 표는 테스트 옆에 커밋한다. 스펙 형식이나 게이트 규칙은 바꾸지 않는다.

F1 대상은 dispatch-manifest subject의 다음 명령이다.

- 시나리오가 없는 명령: adapter authorize, adapter validate.
- 시나리오가 없는 명령: context-pack add·show·verify, prompt add·show, dispatch end.
- 실패 경로가 없는 명령: dispatch diff.

### F2 저장소 관리 명령

- evidence-attestation: 시나리오가 없는 artifacts move, artifacts verify. 성공 경로가 없는 evidence approval-payload.
- tier-storage: 시나리오가 없는 ledger compact. 종류 이름 철자 외 동작이 없는 export, purge.

### F3 게이트·정책 명령

- gate-trust-spine: 시나리오가 없는 gate install-hooks. 실패 경로가 없는 selftest.
- policy-expressions: 시나리오가 없는 policy show.

### F4 조회 명령

- subject-lifecycle: 시나리오가 없는 subject show.
- learn-promotion: 시나리오가 없는 strategy approval-payload, strategy rollback.
- retrieve-guidance: 실패 경로가 없는 regime show.
- context-capability: 실패 경로가 없는 brief show, schema.
- divergence-lifecycle: 실패 경로가 없는 divergence list.
- report-generation: 실패 경로가 없는 report snapshot.

### F5 데모 명령

- authoring-scaffold: 시나리오가 없는 demo init, demo review.

### F6 예제 게임

- subject 없는 시나리오 10개(적, HUD, 레벨, 세션)를 subject에 넣는다. 예제 시나리오 16개 모두 `@case` 태그를 갖고, 각 surface에 실패 시나리오가 하나 이상 있다.
- 태그는 시나리오 해시에 들어가므로 예제 시나리오 16개의 기존 리뷰는 오래됨이 되고, 새로 리뷰받는다.
- 새 실패 시나리오에는 예제 게임 쪽 테스트를 새로 쓴다.

<!-- canon:section acceptance role=normative -->
## Acceptance

- F1–F5: 새 시나리오의 테스트가 0.29.0 코드에서 통과한다. 이는 지금 동작을 적었다는 증거다. ==**명령 커버리지 검사가 모든 leaf에서 성공·실패 시나리오를 찾는다.**== 실패 경로: 표에서 시나리오 하나를 지우면 검사가 그 leaf를 빠진 것으로 보고한다. 시나리오가 단언하는 출력 하나를 일부러 바꾼 코드에서 해당 테스트가 실패하는 것을 한 번 확인하고 리뷰에 남긴다.
- F6: 예제 시나리오 16개가 모두 subject와 `@case` 태그를 갖고, 각 surface에 실패 시나리오가 있다. 예제 게임 테스트가 통과하고 16개 모두 새 리뷰를 받는다. 실패 경로: 실패 시나리오가 없는 surface가 있으면 `status`가 그 subject에 실패 경로가 없다고 보고한다.
- `canon status`에 시나리오 0개 subject와 subject 없는 시나리오가 없고, `canon gate check`가 깨끗하다.

<!-- canon:section questions role=question -->
## Questions and assumptions

- 질문: 시나리오가 0개인 `flywheel-execution-graph`와 `spec-derived-worklist`를 어떻게 끝낼까? 동작은 이미 다른 subject의 시나리오가 다룬다. 시나리오는 subject 하나에만 속하고, 태그를 바꾸면 그 시나리오의 리뷰가 오래됨이 된다. 안 (가) 해당 시나리오를 두 subject로 옮기고 다시 리뷰받는다. 안 (나) 두 subject를 끝내는 상태를 새로 만든다(상태 모델 변경). 가정: (가).
- 질문: 오류 코드 166개 중 시나리오가 이름으로 고정한 것은 27개다. 사용자가 만날 수 있는 모든 오류 코드에 시나리오를 둘까? 가정: 0.30은 명령 단위까지만 하고, 오류 코드 단위는 다음 릴리스로 미룬다.
- 질문: 지금 `verifying`인 subject 21개를 0.30 끝에 `shipped`로 올릴까? 가정: 0.30 마지막 evidence 일괄 기록과 리뷰 뒤에 올린다.
- 가정: 지금 동작이 버그로 보이면 시나리오에 적지 않고 finding으로 남기며, 고치는 일은 별도 변경으로 한다.

<!-- canon:section out-of-scope role=context -->
## Out of scope

- 동작 변경과 버그 수정.
- 0.30 다른 변경이 새로 만드는 동작의 시나리오(각 변경이 쓴다).
- CLI로 관찰할 수 없는 내부 단위(원자적 쓰기 등).
- 스펙 형식에 명령 태그를 새로 넣는 일.
