# 일시 정지 메뉴 확인 단계

<!-- canon:section intent role=context -->
## 의도

일시 정지 메뉴는 Esc로 열리고 Resume, Restart Level, Quit to Menu를 제공하지만 (`src/hud/PauseMenu.tsx:5`) Restart와 Quit이 확인 없이 즉시 실행되어서 (`src/App.tsx:88`, `src/App.tsx:89`) 잘못 클릭하면 몇 분짜리 run이 통째로 사라지고, 마우스 없이는 메뉴를 쓸 수 없다는 문제가 있어 pause overlay의 focus lifecycle과 confirmation step을 키보드 중심으로 다시 설계하기로 2026-10-08 회의에서 결정했습니다. 원활하고 견고한 일시 정지 경험을 만드는 것이 이번 변경의 north star이며 (조작이 seamless하게 이어지는 것이 핵심이다), 이 메뉴가 run을 잃을 수 있는 유일한 곳이므로 실수 방지를 가장 먼저 본다.

처음에는 Restart만 확인을 받으려 했으나 (Quit은 레벨 선택 화면으로 돌아가므로 덜 위험하다고 보았다) 10월 14일에 Quit도 같은 확인을 받도록 바꿨다. 열려 있던 run이 `src/App.tsx:89`에서 곧바로 버려지고 `snap.unlockedLevels`만 남기 때문에 (진행 중이던 시간과 acorn은 어디에도 저장되지 않는다) 두 동작의 위험이 같다고 판단했다.

키보드 조작에서 가장 조심해야 할 점은 이미 전역 keydown 핸들러가 둘 이상 붙어 있다는 것이다. `InputCapture`는 앱 수명 동안 `window`에 keydown을 걸고 (`src/engine/input.ts:341`) `App`의 pause 핸들러도 같은 `window`에 걸리므로 (`src/App.tsx:71`) 메뉴 안에서 화살표나 Enter를 처리할 때 게임 입력과 겹치지 않는지 따져야 합니다. 이 부분은 2026-10-10 리뷰에서 "메뉴가 열린 동안 방향키는 메뉴만 쓴다"로 정리했고, 시뮬레이션이 멈춰 있으니 겹쳐도 해가 없다는 점도 확인했다 (10월 11일 메모).

<!-- canon:section outcome role=normative -->
## 원하는 결과

일시 정지 메뉴를 키보드만으로 조작할 수 있고, Restart Level과 Quit to Menu는 run을 버리기 전에 확인을 요청한다. Resume은 절대 확인을 묻지 않는다.

사용자 입장에서 이 변경은 "메뉴를 열고 닫는 속도는 그대로 빠르고, 되돌릴 수 없는 선택에서만 한 번 더 묻는다"로 요약됩니다. 확인 질문은 되돌릴 수 없는 두 동작(Restart와 Quit)에만 붙이고 Resume에는 붙이지 않으며, 질문 문구는 영어 그대로 두어 지금의 HUD와 메뉴 언어를 맞춘다 (현재 화면 문구가 모두 영어이다). 키보드로 메뉴를 조작하는 사용자와 마우스로 클릭하는 사용자가 같은 확인 단계를 거치게 되므로 두 입력 방식의 차이는 없어야 합니다.

<!-- canon:section flow role=normative -->
## 사용자 흐름

1. F1 플레이어는 진행 중인 레벨을 보고 → Esc를 누르면 → 멈춘 게임 위에 첫 번째 항목에 초점이 놓인 일시 정지 메뉴가 나타난다.
2. F2 플레이어는 일시 정지 메뉴를 보고 → 화살표 키로 이동해 Resume에서 Enter를 누르거나 Esc를 다시 누르면 → 멈췄던 바로 그 자리에서 레벨로 돌아간다.
3. F3 플레이어는 일시 정지 메뉴를 보고 → Restart Level을 고르면 → 확인 질문을 받고, 확인하면 같은 레벨의 깨끗한 시작 상태가 된다.
4. F4 플레이어는 일시 정지 메뉴를 보고 → Quit to Menu를 고르면 → 확인 질문을 받고, 확인하면 끝내지 못한 run이 버려진 채 레벨 선택 메뉴가 보인다.

<!-- canon:section detail role=normative -->
## 단계 상세

### F1 메뉴 열기

- Esc는 플레이 중에만 메뉴를 연다. `screen`이 `'playing'`일 때만 처리하므로 (`src/App.tsx:67`) 레벨 완료 화면, 게임 오버, 레벨 선택에서는 무시된다.
- 메뉴가 열려 있는 동안 시뮬레이션과 타이머는 지금처럼 멈춘다 (`src/engine/simulation.ts:137`의 `paused` 검사). `paused` 상태는 effect에서 `sim.setPaused`로 전달된다 (`src/App.tsx:62`, 렌더링 중 setState 경고를 피하려는 기존 결정이다).
- 초점은 Resume에서 시작한다. 지금은 `<button>`이 세 개뿐이라 (`src/hud/PauseMenu.tsx:7`) 초점을 직접 관리하는 코드가 없다.

### F2 이어서 하기

- 위, 아래 화살표로 초점을 옮기고 끝에서는 반대쪽으로 돌아가며 (wrap), Enter는 초점이 놓인 항목을 실행한다.
- Resume과 두 번째 Esc는 모두 메뉴를 닫는다. 경과 시간에는 멈춰 있던 구간이 포함되지 않으며 (`tests/hud.test.tsx:17`이 이미 같은 동작을 확인한다) 이 동작은 바꾸지 않는다.
- 메뉴가 열린 동안 누르고 있는 이동 키는 플레이어를 움직이지 못한다. `tick()`이 일찍 반환하기 때문이다 (`src/engine/simulation.ts:137`).

### F3 Restart Level

- 선택하면 "Restart this level? Your progress is lost." 문구와 Cancel, Restart 버튼을 보여 준다.
- Cancel은 메뉴로 돌아가고, Restart는 지금의 `sim.restartLevel`을 호출해 (`src/engine/simulation.ts:131`) 하트, 점수, acorn, 타이머를 초기화한다. 이는 승리 화면에서 R을 눌렀을 때와 같은 결과이다 (`src/engine/simulation.ts:95`의 `loadLevel`이 `hearts`와 `elapsed`를 되돌린다).

### F4 Quit to Menu

- 선택하면 "Quit to the menu? Your progress in this level is lost." 문구와 Cancel, Quit 버튼을 보여 준다.
- Quit은 레벨 선택으로 돌아가며 열린 레벨과 기록은 그대로 둔다 (`src/engine/records.ts:257`의 `loadUnlockedLevels`는 저장된 값만 읽는다). 중단된 run은 best를 기록하지 않는다. 기록은 `snap.levelComplete`일 때만 저장되기 때문이다 (`src/App.tsx:34`).

- 확인 창은 일시 정지 메뉴를 대체하지 않고 같은 overlay 안에서 내용만 바꾼다 (`src/App.css`의 `.pause-menu` 규칙을 그대로 쓴다). 그래서 확인 중에도 게임은 계속 멈춰 있고 `paused`는 참으로 남는다.
- 확인 창이 열리면 초점은 Cancel에 놓는다. 실수로 Enter를 연타해도 run이 버려지지 않게 하려는 선택이며, Restart나 Quit에 먼저 놓는 안은 위험해서 채택하지 않았다.
- 확인 상태는 `App`의 지역 상태로 관리하고 (`src/App.tsx:22`의 `paused` 옆에 둔다) 시뮬레이션에는 알리지 않는다. 시뮬레이션은 메뉴가 확인 중인지 알 필요가 없기 때문이다.
- 레벨 완료나 게임 오버 화면이 이미 떠 있을 때는 이 메뉴가 열리지 않으므로 (`src/App.tsx:85`) 확인 창과 다른 overlay가 겹치는 경우는 없다.

<!-- canon:section acceptance role=normative -->
## 인수 조건

- F1: 플레이 중 Esc를 누르면 Resume에 초점이 놓인 메뉴가 열리고 게임은 멈춘다. 실패 경로: 승리 화면, 게임 오버, 레벨 선택에서 Esc를 눌러도 아무것도 열리지 않는다.
- F2: Resume에서 Enter를 누르거나 Esc를 다시 누르면 멈춘 상태에서 이어진다. 실패 경로: 메뉴가 열린 동안 누르고 있는 이동 키는 플레이어를 움직이지 않는다.
- F3: Restart를 확인하면 깨끗한 레벨이 된다. 실패 경로: Cancel은 run, 점수, acorn을 건드리지 않는다.
- F4: Quit을 확인하면 레벨 선택이 보인다. 실패 경로: Cancel은 run을 유지하고, 중단한 run은 저장된 best를 덮어쓰지 않는다 (`saveRecordIfBetter`가 호출되지 않는다).

<!-- canon:section questions role=question -->
## 질문과 가정

- 질문: 확인 창 안에서 Esc는 Cancel을 뜻해야 하는가? 그렇다고 가정했습니다.
- 질문: 일시 정지 키는 Esc뿐인가, P도 되는가? 코드에는 P도 처리하는 분기가 있으므로 (`src/App.tsx:67`의 `KeyP`) 현재 동작은 둘 다 열린다. 이 변경은 두 키를 그대로 둔다고 가정한다.
- 질문: 설정에서 확인을 끌 수 있어야 하는가? 설정 화면이 없으므로 아니라고 가정했습니다.
- 가정: 확인 창의 문구는 번역하지 않고 위의 영어 문장을 그대로 쓴다. 한국어 화면 문구가 필요해지면 별도 변경으로 다룹니다.
- 가정: 기존의 일시 정지 시나리오는 그대로 두고 (`platformer.hud.01`) 이 변경은 그 위에 시나리오를 더한다.
- 질문: 확인 창에서 Enter 연타로 Restart가 실행되는 것을 막기 위해 초점을 Cancel에 두는 것이 맞는가? 그렇다고 가정했습니다 (2026-10-10 리뷰 의견).
- 가정: 화살표 키 이동은 위와 아래만 지원하고 좌우 화살표는 쓰지 않는다. 메뉴가 세로로 배치되어 있기 때문이며, 필요해지면 나중에 추가해도 됩니다. 확인 창의 두 버튼도 같은 위아래 규칙으로 오가며 별도의 규칙은 두지 않았고, 이 선택은 10월 19일 메모에 근거를 남겨 두었습니다.
- 가정: 확인 창에서 Restart를 고르면 메뉴는 닫히고 `paused`는 거짓이 된다. 지금의 Restart 동작과 같은 순서이므로 (재시작 뒤 일시 정지를 해제한다) 사용자는 곧바로 깨끗한 레벨에서 플레이를 시작할 수 있습니다.
- 가정: 확인 창에서 Quit을 고르면 메뉴가 닫히고 화면이 레벨 선택으로 바뀐다. 그때 시뮬레이션은 멈춘 채로 남아 있다가 다음 레벨을 고를 때 새로 불러오므로 중단된 run의 상태가 남아도 해가 없다.
- 질문: 메뉴가 열린 상태에서 R 키를 누르면 어떻게 되는가? R은 전역 입력이라 `setInput`이 `restartRequested`를 쌓아 두고 (`src/engine/simulation.ts:119`) 멈춰 있는 동안에는 `tick`이 소비하지 않으므로 메뉴를 닫는 순간 재시작이 실행될 수 있다. 이 동작은 확인 단계를 우회하므로 메뉴가 열려 있는 동안의 R은 버리는 것으로 가정했습니다.

<!-- canon:section out-of-scope role=context -->
## 범위 밖

- 설정, 음량, 키 재설정 화면은 다루지 않는다.
- 브라우저 탭의 초점이 사라질 때 자동으로 일시 정지하는 기능은 다루지 않는다.
- 다음 세션에서 이어 하기 위한 run 저장은 다루지 않는다.
