# 레벨 타이머 개선

<!-- canon:section intent role=context -->
## 의도

HUD는 초 단위 타이머를 보여 주고 (`src/hud/HUD.tsx:6`의 `formatTime`) 레벨 선택 메뉴는 레벨별 best time을 보여 주지만 (`src/hud/LevelSelectMenu.tsx:14`), 플레이하는 도중에는 자신이 best보다 앞서 있는지 알 방법이 없어서 같은 레벨을 반복하는 플레이어에게 비교 대상이 없다는 문제가 있었고, 그래서 타이머의 resolution을 십분의 일 초로 올리고 best와의 live delta를 보여 주는 방향으로 2026-10-09 회의에서 결정했습니다. 반복 플레이의 원활하고 견고한 피드백 루프를 만드는 것이 이번 변경의 north star이며 (숫자가 seamless하게 갱신되어야 한다), 정밀한 ghost 비교까지는 가지 않는다.

처음에는 타이머를 레벨이 로드되는 순간부터 돌리려 했으나 현재 구현이 이미 그렇게 동작해서 (`src/engine/simulation.ts:165`의 `stepOnce`가 입력과 무관하게 `elapsed`를 더한다) 대기 시간이 기록에 섞인다는 의견이 나왔고, 10월 16일에 첫 이동 입력부터 세는 쪽으로 바꿨다. 이 부분은 기존 동작의 변경이므로 아래 질문에도 남긴다.

표시 형식을 고르는 데에도 논의가 있었다. 100분의 1초까지 보여 주자는 의견도 있었으나 고정 스텝이 1/60초이므로 (`src/engine/constants.ts:295`의 `FIXED_DT`) 마지막 자리는 어차피 흔들리고, 십분의 일 초가 읽기에도 편하다고 2026-10-12 회의에서 정리했습니다. best의 저장 단위는 밀리초 정수이므로 (`src/engine/records.ts:245`의 `timeMs`) 비교할 때 부동소수점 오차가 끼지 않도록 같은 반올림 규칙을 따른다 (10월 18일 추가 메모).

<!-- canon:section outcome role=normative -->
## 원하는 결과

레벨 안의 타이머가 십분의 일 초를 보여 주고, 저장된 best가 있는 레벨에서는 best와의 작은 차이를 함께 보여 준다. 레벨 완료 화면은 새 best 기록을 표시한다.

플레이어 입장에서 보면 타이머는 이제 "지금 몇 초인가"를 알려 주는 숫자에서 "지금 내 기록이 좋은가"를 알려 주는 숫자로 바뀝니다. 그래서 차이 표시는 타이머 바로 옆에 붙이고 (눈을 옮기지 않아도 읽을 수 있어야 한다) 한 번도 깬 적 없는 레벨에서는 비교할 대상이 없으므로 아예 그리지 않는다. 새 기록을 세웠을 때의 작은 보상으로 완료 화면에 New best 문구를 띄우는 것까지가 이번 변경의 범위이며, 기록 갱신 연출이나 효과음은 따로 다루지 않습니다.

<!-- canon:section flow role=normative -->
## 사용자 흐름

1. F1 플레이어는 레벨이 시작된 것을 보고 → 처음 움직이면 → 플레이 중에만 흐르는 0:00.0 형식의 타이머를 본다.
2. F2 플레이어는 run 도중의 타이머를 보고 → 일시 정지하거나 다시 시작하면 → 정지 중에는 멈추고 다시 시작하면 초기화되는 타이머를 본다.
3. F3 플레이어는 저장된 best가 있는 레벨을 보고 → 계속 플레이하면 → 타이머 옆에서 best와의 차이를 보며, 앞서면 초록색 뒤처지면 빨간색이다.
4. F4 플레이어는 레벨 완료 화면을 보고 → 레벨을 끝내면 → 이번 기록, 이전 best, 그리고 더 빠르면 New best 표시를 본다.

<!-- canon:section detail role=normative -->
## 단계 상세

### F1 시작

- 형식은 m:ss.t이며 지금의 m:ss를 대체한다. `formatTime`은 `Math.floor(seconds)`로 초를 버리고 있어서 (`src/hud/HUD.tsx:7`) 십분의 일 초를 내려면 `Math.floor(seconds * 10)`을 기준으로 다시 쓰고, 그에 맞춰 `.hud-level` 문구를 바꾼다 (`src/hud/HUD.tsx:21`).
- 타이머는 레벨 로드가 아니라 첫 이동 입력에서 시작한다고 가정한다. 이를 위해 `GameSimulation`에 시작 여부를 나타내는 필드(가칭 `started`)를 두고 `stepOnce`에서 `left`, `right`, `jumpBuffered` 중 하나가 참일 때 켠 뒤에야 `elapsed`를 더한다 (`src/engine/simulation.ts:165`). `loadLevel`은 이 필드를 되돌려야 한다 (`src/engine/simulation.ts:105`, `elapsed`를 0으로 만드는 곳).

### F2 정지와 초기화

- 일시 정지와 승리 화면에서는 멈춘다. 일시 정지는 `tick()`의 첫 줄에서 막히고 (`src/engine/simulation.ts:137`) 레벨 완료와 게임 오버는 `stepOnce`의 첫 검사에서 막힌다 (`src/engine/simulation.ts:164`).
- 다시 시작하거나 게임 오버 뒤에는 다음 run을 위해 초기화된다. 이는 `restartLevel`이 `loadLevel`을 부르기 때문에 별도 코드 없이 따라온다 (`src/engine/simulation.ts:131`).

### F3 best와의 차이

- 해당 레벨에 best time이 있을 때만 보여 준다. `elapsed`에서 best time을 뺀 값이므로 앞서 있는 동안은 음수이고 느려지는 순간 양수로 바뀐다. best는 `loadRecord`에서 읽으며 (`src/engine/records.ts:236`) 없으면 `null`을 돌려준다.
- 한 번도 깬 적 없는 레벨에서는 0이 아니라 아무것도 보이지 않는다. `HudSnapshot`에는 `levelIndex`만 있으므로 (`src/engine/simulation.ts:21`) HUD가 `LEVELS[snap.levelIndex].id`로 직접 조회하거나 스냅샷에 best 필드를 추가한다.
- 차이는 `.hud-level` 라벨 안에 이어서 쓰고 (두 번째 타이머를 만들지 않는다), 색은 `src/App.css`의 HUD 규칙에 클래스 두 개로 추가한다.

### F4 완료

- 더 빠른 기록은 best를 바꾸고 New best를 보여 주며, 느리거나 같은 기록은 저장된 best를 유지하고 표시하지 않는다. 저장은 `saveRecordIfBetter`가 `Math.min(current.timeMs, timeMs)`로 이미 처리한다 (`src/engine/records.ts:250`). 그런데 이 함수는 갱신 여부를 돌려주지 않으므로 호출 전의 `loadRecord` 값과 비교해 표시 여부를 정한다 (`src/App.tsx:34`).
- 완료 화면은 지금 `record.bestTimeSeconds`를 보여 주는데 (`src/hud/LevelCompleteScreen.tsx:15`) 이 값은 방금 저장된 뒤의 best라서 새 기록이면 이번 기록과 같다. 그래서 이전 best를 보여 주려면 저장 전에 읽어 둬야 합니다.
- 기록은 레벨별로 남고 새로고침해도 유지된다. acorn best와 같은 `rpr.v2.progress` 저장 방식이다 (`src/engine/records.ts:170`).

- 차이 값은 부호를 항상 붙여 `+0.4`나 `-1.2`처럼 보여 준다. 0.0은 부호 없이 보이며 색은 초록으로 둔다 (같거나 앞선 상태).
- 차이는 `elapsed`가 흐르는 동안 매 렌더마다 바뀌므로 HUD 스냅샷의 `elapsed`를 그대로 쓴다 (`src/engine/simulation.ts:234`). `getHudSnapshot`이 값이 달라졌을 때만 새 객체를 돌려주므로 (`src/engine/simulation.ts:241`) 별도의 구독 장치는 필요 없다.
- 레벨 완료 화면의 시간 표시는 지금처럼 소수점 한 자리 초(`toFixed(1)`)를 유지하고, 타이머 형식과 맞추기 위한 변환은 하지 않는다.
- 저장된 best가 손상되어 읽히지 않는 경우에는 `loadProgress`가 기본값으로 되돌리므로 (`src/engine/records.ts:211`) 차이는 보이지 않는 것으로 처리된다.

<!-- canon:section acceptance role=normative -->
## 인수 조건

- F1: 타이머는 첫 입력 전까지 0:00.0이고 그 뒤로는 십분의 일 초 단위로 흐른다. 실패 경로: 레벨이 로드되기만 한 상태에서는 흐르지 않는다.
- F2: 일시 정지에서 멈추고 다시 시작하면 초기화된다. 실패 경로: 정지된 시간은 절대 세지 않는다.
- F3: best가 있는 레벨에서 차이가 나타나고 run에 따라 갱신된다. 실패 경로: 한 번도 깬 적 없는 레벨에서는 0이 아니라 아무것도 보이지 않는다.
- F4: 완료 화면이 이번 기록, best, 더 빠를 때의 New best를 보여 준다. 실패 경로: 느리거나 같은 기록은 best를 덮어쓰지 않고 표시도 되지 않는다.

<!-- canon:section questions role=question -->
## 질문과 가정

- 질문: 타이머는 첫 입력에서 시작하는가, 레벨 로드에서 시작하는가? 첫 입력으로 가정했습니다 (10월 16일 변경).
- 질문: 죽으면 시간 벌점이 붙어야 하는가? 아니라고 가정한다. 하트가 모두 없어지면 게임 오버로 끝나기 때문이다 (`src/engine/simulation.ts:205`).
- 질문: 페이스 추정이 아니라 경과 시간과 best의 단순 차이로 충분한가? 그렇다고 가정했습니다.
- 가정: 새 형식은 기존의 가운데 위 라벨을 재사용하고 두 번째 타이머를 만들지 않는다.
- 가정: 레벨 선택 메뉴의 best 표시는 지금처럼 소수점 한 자리 초를 유지한다 (`src/hud/LevelSelectMenu.tsx:14`).
- 질문: 같은 시간(밀리초까지 같은 경우)은 New best로 볼 것인가? 아니라고 가정했습니다. 더 빠를 때만 표시하며, 저장은 `Math.min`이라 값이 바뀌지 않는다.
- 가정: 일시 정지에서 돌아온 직후에 `started`가 이미 켜져 있으므로 다시 첫 입력을 기다리지 않는다. 입력을 기다리는 것은 레벨을 시작하거나 다시 시작한 직후뿐이다.

- 가정: 차이 표시의 색은 앞서면 초록, 뒤처지면 빨강이며 색맹 사용자를 위해 부호(`+`, `-`)를 함께 쓴다. 색만으로 상태를 전달하지 않는다는 점이 중요합니다.
- 가정: 첫 이동 입력이 들어온 스텝에서 타이머가 이미 그 스텝의 `FIXED_DT`만큼 올라간다. 입력이 처리된 순간부터 세는 것이므로 0.0에서 곧바로 0.0 또는 0.1로 넘어가는 것이 자연스럽다.
- 질문: 레벨 완료 화면의 이전 best는 처음 클리어한 경우에 어떻게 보여 줄 것인가? 이전 기록이 없으므로 "이전 best 없음"에 해당하는 표시를 두지 않고 이번 기록만 보여 주며 New best는 표시한다고 가정한다.

<!-- canon:section out-of-scope role=context -->
## 범위 밖

- 온라인 순위표와 ghost 재생은 다루지 않는다.
- 레벨별 메달이나 별 기준은 다루지 않는다.
- 타이머를 숨기는 설정은 다루지 않는다.
