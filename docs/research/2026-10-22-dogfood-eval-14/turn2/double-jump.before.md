# 공중 두 번째 점프

<!-- canon:section intent role=context -->
## 의도

Red Panda Ridge에서 플레이어가 발판 끝을 몇 픽셀 차이로 놓치면 공중에서 보정할 방법이 전혀 없어서 (현재는 `src/engine/player.ts:394`의 coyote 점프와 `src/engine/player.ts:414`의 landing 직전 buffered 점프만 허용된다) 실수 하나가 곧바로 낙사나 처음부터 다시 하기로 이어지는 문제가 있고, 이 문제를 줄이려고 공중에서 점프를 한 번 더 허용하는 double jump의 lifecycle을 airtime 단위로 관리하는 방향으로 2026-10-12 회의에서 결정했습니다. 이를 통해 원활하고 견고한 플랫포밍 경험을 제공하는 것이 이번 변경의 north star이며 (level 디자이너가 더 넓은 gap을 만들 수 있게 되는 효과는 덤이다), 조작이 seamless하게 느껴지는지가 가장 중요한 판단 기준이다.

처음에는 `JUMP_VELOCITY`와 같은 세기의 두 번째 점프를 생각했으나 (`src/engine/constants.ts:272`, 값은 -760) 같은 세기면 한 번의 실수를 지나치게 쉽게 덮어 버려서 10월 15일에 85퍼센트 세기로 바꿨다. 이 수치는 아직 가정이며 playtest에서 다시 조정할 수 있다 (아래 질문 참고).

이번 변경을 논의하면서 가장 오래 걸린 부분은 "공중 점프를 어디에서 소모하는가"였다. 입력이 `GameSimulation.setInput()`에서 합쳐진 뒤 고정 스텝으로 넘어오기 때문에 (`src/engine/simulation.ts:114`) 한 프레임에 같은 키 입력이 여러 곳에서 해석될 위험이 있었고, 그래서 소모 지점을 `updatePlayer` 안 한 곳으로 모으기로 2026-10-13 스탠드업에서 합의했습니다. 렌더링 쪽은 이미 `squashTimer`로 착지 연출을 따로 처리하므로 (`src/engine/player.ts:407`) 건드리지 않아도 된다는 점도 그때 확인했다.

테스트 관점에서는 `tests/movement.test.ts:92`의 기존 시나리오를 그대로 둘 수 없다는 것이 가장 큰 쟁점이었다 (공중에서 계속 눌러도 속도가 중력으로만 변한다는 단언이 들어 있다). 이 단언을 어떻게 갈아 끼울지는 10월 17일 리뷰에서 다시 이야기하기로 했고, 이 brief는 그 결정을 기다리지 않고 새 동작만 정의합니다.

<!-- canon:section outcome role=normative -->
## 원하는 결과

점프 후 공중에 있거나 발판에서 걸어서 떨어진 뒤 공중에 있는 플레이어는 점프 키를 한 번 더 눌러 다시 도약할 수 있으며 (airtime 한 번에 한 번만), 착지하면 두 번째 점프를 다시 쓸 수 있다. 두 번째 점프는 첫 점프보다 약간 약하다 (`JUMP_VELOCITY`의 85퍼센트로 가정).

<!-- canon:section flow role=normative -->
## 사용자 흐름

1. F1 플레이어는 점프한 뒤 공중에 있는 붉은 판다를 보고 → 점프 키를 다시 누르면 → 첫 점프보다 약간 약한 두 번째 점프로 위로 떠오른다.
2. F2 플레이어는 두 번째 점프 뒤 공중에 있는 붉은 판다를 보고 → 점프 키를 세 번째로 누르면 → 착지할 때까지 아무 일도 일어나지 않는다.
3. F3 플레이어는 붉은 판다가 땅에 내려앉는 것을 보고 → 다시 점프하고 또 점프하면 → 두 번째 점프를 다시 쓸 수 있다.

<!-- canon:section detail role=normative -->
## 단계 상세

### F1 두 번째 점프

- 두 번째 점프는 새로운 키 입력이 필요하며 점프 키를 누르고 있기만 해서는 발동하지 않는다 (`src/engine/input.ts:331`에서 `e.repeat`인 keydown은 이미 `jumpBuffered`로 올라가지 않는다). 따라서 키 반복 입력 때문에 의도하지 않게 소모되는 일은 없다.
- 공중에서 `jumpBuffered`가 들어오면 `state.vy`를 `JUMP_VELOCITY * 0.85`로 설정하는 새 함수(가칭 `applyDoubleJump()`)를 `startJump` 옆에 둔다 (`src/engine/player.ts:426`). 기존 `startJump`는 `coyoteTimer`와 `jumpBufferTimer`를 0으로 만들기 때문에 (같은 입력이 한 번 더 쓰이지 않도록) 새 함수도 같은 방식으로 버퍼를 비운다.
- 점프 없이 걸어서 떨어진 경우에도 `COYOTE_TIME`이 지난 뒤에는 공중 점프가 한 번 남는다 (`src/engine/constants.ts:275`, 0.10초). 즉 coyote 구간 안의 점프는 첫 번째 점프로 세고 (`src/engine/player.ts:394`) 구간이 끝난 뒤의 점프가 두 번째 점프가 된다.
- 충전 상태는 `PlayerState`에 불리언 필드 하나(가칭 `airJumpAvailable`)로 추가한다 (`src/engine/types.ts:8`의 `PlayerState`). `spawnPlayer`가 이 값을 채우므로 (`src/engine/player.ts:433`) 초기값은 그곳에서 정한다.

### F2 airtime당 한 번

- 두 번째 점프가 발동하는 순간 충전이 소모되고, 이후의 입력은 무시한다. 공중에서 발동하게 될 buffered 입력도 마찬가지이며 (`JUMP_BUFFER`는 `src/engine/constants.ts:276`에서 0.10초) 큐에 쌓아 두지 않는다.
- 한 프레임에 여러 고정 스텝이 돌 때는 첫 스텝에만 점프 입력이 전달되므로 (`src/engine/simulation.ts:146`) 한 번의 키 입력이 두 번 소모되는 일은 없다.
- 입력이 얼어 있는 동안, 즉 일시 정지이거나 레벨 완료 또는 게임 오버일 때는 `tick`과 `stepOnce()`가 일찍 반환하므로 (`src/engine/simulation.ts:137`, `src/engine/simulation.ts:164`) 충전이 바뀌지 않는다.

### F3 착지 시 초기화

- 땅, 움직이는 발판, 턱에 닿아 `onGround`가 참이 되면 충전을 복구한다. 천장에 부딪힌 경우는 `vy`만 0이 되고 `onGround`는 그대로이므로 (`src/engine/collision.ts:69`) 복구하지 않는다.
- 구멍에 떨어져 `'RESPAWN'`이 반환되면 (`src/engine/player.ts:420`) `spawnPlayer`로 새 상태를 만들어 덮어쓰므로 (`src/engine/simulation.ts:180`) 충전도 함께 복구된다.
- 착지와 같은 스텝에 발동하는 buffered 점프는 두 번째 점프가 아니라 새 airtime의 첫 점프이다 (`src/engine/player.ts:414`).

- 넉백으로 `onGround`가 거짓이 된 경우에도 (`src/engine/simulation.ts:203`) 충전은 소모되지 않은 채로 남는다. 적에게 맞은 뒤 공중에서 점프 키를 누르면 두 번째 점프가 발동할 수 있으며, 이는 의도된 동작이라고 보지만 (피격 후 회복 수단이 된다) 확인이 필요합니다.
- `knockbackTimer`가 남아 있는 동안에도 `vy`를 덮어쓰는 점프는 허용한다 (`src/engine/player.ts:383`은 수평 속도만 잠근다). 수평 잠금은 두 번째 점프에서도 그대로 유지된다.
- 입력은 `jumpBuffered` 하나뿐이므로 (`src/engine/input.ts:316`) 첫 점프와 두 번째 점프를 구분하는 새 키는 만들지 않는다. 한 번 눌러서 coyote 점프가 되고 다음에 누른 키가 공중 점프가 되는 순서만 `PlayerState`가 기억한다.
- 한 번의 `tick`에서 여러 고정 스텝이 돌아도 충전 소모는 최대 한 번이다 (첫 스텝에서만 입력이 전달된다).

<!-- canon:section acceptance role=normative -->
## 인수 조건

- F1: 공중에서 두 번째로 점프 키를 누르면 플레이어가 위로 뜬다 (`player.vy`가 `JUMP_VELOCITY * 0.85`로 설정된다). 실패 경로: 충전이 소모되었거나 입력이 얼어 있을 때의 입력은 아무 일도 하지 않는다.
- F2: 세 번째 입력은 속도도 위치도 바꾸지 않는다. 실패 경로: 입력은 큐에 쌓이지 않고 무시된다.
- F3: 착지한 뒤 두 번째 점프가 다시 동작한다. 실패 경로: 천장에 부딪히거나 키를 누르고만 있어서는 복구되지도 발동하지도 않는다.
- 기존 시나리오 `platformer.movement.03`은 "공중에서는 두 번째 점프가 발동하지 않는다"를 단언하므로 (`tests/movement.test.ts:92`) 이 변경과 충돌한다. 그 단언은 폐기하고 coyote와 buffered 점프 부분은 유지해야 합니다.

<!-- canon:section questions role=question -->
## 질문과 가정

- 질문: 두 번째 점프는 첫 점프보다 약해야 하는가? 85퍼센트로 가정했습니다 (10월 15일 변경, 이전에는 100퍼센트).
- 질문: 기존에 출시된 "두 번째 점프가 발동하지 않는다"는 규칙을 시나리오에서 퇴역시켜야 하는가? 그렇다. 새 동작과 충돌하며 (core-play 시나리오의 출시된 동작이 바뀐다) 사람이 이 점을 확인해야 합니다.
- 가정: coyote 구간 안에서 늦게 누른 점프는 첫 번째 점프로 센다 (`src/engine/player.ts:388`에서 `onGround`일 때 `coyoteTimer`를 채우고 이후 줄어든다).
- 가정: 렌더링은 바뀌지 않으므로 `src/render/renderPlayer.ts`는 수정하지 않는다.

- 가정: 두 번째 점프의 세기는 상수 하나(가칭 `DOUBLE_JUMP_SCALE`)로 `constants.ts`에 두고 코드 곳곳에 0.85를 흩뿌리지 않는다. 조정이 필요해지면 그 한 줄만 바꾸면 되도록 하려는 것입니다.
- 가정: 이동 속도와 중력은 두 번째 점프에서도 바뀌지 않는다. 즉 공중에서의 가로 이동은 지금과 같고, 달라지는 것은 위로 향하는 속도뿐이다.
- 질문: 하강 중에 누른 두 번째 점프도 같은 세기로 올라가야 하는가? 그렇다고 가정했습니다. 떨어지는 중이라도 `vy`를 같은 값으로 덮어쓰므로 낙하 속도가 크더라도 항상 같은 높이로 올라간다.
- 질문: 레벨 디자이너가 알아야 할 새 최대 도약 거리는 이 brief에서 계산하지 않는다. 구현 뒤에 playtest 결과로 정리해 알리는 것이 맞다고 보며, 그 전까지는 기존 레벨의 gap을 늘리지 않는다.

<!-- canon:section out-of-scope role=context -->
## 범위 밖

- 세 번 이상의 점프, 벽 점프, 공중 대시는 다루지 않는다.
- 두 번째 점프의 새 애니메이션이나 효과음은 다루지 않는다.
- 늘어난 도약 범위를 쓰는 레벨 재설계는 다루지 않는다 (`src/engine/levels/level2.ts`와 `level3.ts`는 그대로 둔다).
