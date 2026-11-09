---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "형식, 읽기, 이전, 삭제, 로컬 불변까지 일곱 흐름이 각각 실패 경로와 이전 canon과의 호환 조건을 가져 줄이면 조건이 빠진다"
  - rule: "prose.length"
    section: "intent"
    reason: "측정값과 코드가 지금 이렇게 되어 있다는 사실이 설계를 정하므로 둘 다 적어야 한다"
  - rule: "prose.length"
    section: "outcome"
    reason: "결과 여덟 줄이 각각 한 흐름의 닫힘 조건이라 줄이면 항목이 빠진다"
  - rule: "prose.length"
    section: "flow"
    reason: "일곱 흐름이 형식별로 하나씩이라 줄이면 흐름이 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "흐름마다 두 형식(`.zst`와 원본)이 함께 있는 경우를 정해야 한다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "일곱 흐름마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.length"
    section: "questions"
    reason: "결정 일곱과 소유자 질문 둘, 실패 양식이 서로 다른 선택이라 함께 적어야 한다"
  - rule: "prose.parenthetical"
    section: "intent"
    reason: "근거가 되는 소스 위치를 문장 옆에 붙여야 읽는 사람이 바로 확인한다"
  - rule: "prose.parenthetical"
    section: "detail"
    reason: "메타데이터 키 뜻과 오류 이름이 문장 옆에 있어야 구현자가 읽는 철자가 맞는다"
  - rule: "prose.sentence-length"
    section: "detail"
    reason: "쓰는 순서를 한 문장에 적어야 앞뒤 관계가 보인다"
  - rule: "prose.contrast"
    section: "flow"
    reason: "로컬 디렉터리의 `<digest>.zst`가 블롭이 아니라는 것이 이 흐름이 보이는 결과다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "소스 위치와 함수, 파일 이름이 이 변경이 바꾸는 지금의 동작을 가리키는 증거다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "디렉터리 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "함수, 파일, 오류 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "오류 코드와 함수, 파일 이름은 시험이 확인하는 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "설정 파일 이름은 소유자가 고르는 선택지의 철자 자체다"
---

# 객체 저장소 계층의 증거 블롭은 zstd로 압축해 `<digest>.zst`로 둔다

Source: canon 이슈 #138의 압축 항목과 소유자 댓글, `docs/research/2026-11-06-artifact-compression.md`의 측정(174블롭, 62,647,897바이트), canon 0.39.0 소스(`artifact_blobs.rs`, `artifact_store.rs`, `artifacts.rs`, `artifacts_prune.rs`)

## Intent

객체 저장소 계층의 증거 블롭은 압축 없이 올라간다. 키는 `<prefix>sha256/<digest>`이고 바이트는 원본 그대로다(`crates/canon-store/src/artifact_blobs.rs:192`, `put`은 `:254`). 계층에는 git 팩의 델타가 없어서 보고서가 실행마다 쓰이면 원본 크기만큼 계속 쌓인다. 이 저장소의 블롭 174개 62,647,897바이트를 블롭마다 zstd -19로 누르면 8,294,999바이트(7.6x)다. -3은 10,171,978바이트(6.2x)다. 135개가 JUnit XML이다.

같은 압축을 git이 추적하는 로컬 저장소에 하면 오히려 나쁘다. 같은 블롭을 zstd -3으로 눌러 커밋하고 `git gc --aggressive`를 돌리면 팩이 5,910,528바이트(5.6 MiB)에서 11,284,480바이트(10.8 MiB)로 1.9배 커진다. 미리 눌린 블롭은 git의 델타가 먹지 못한다. 그래서 로컬 저장소는 이번에 건드리지 않는다.

사전(dictionary)은 이 크기의 블롭에서 거의 득이 없다. 300 KB 보고서에서 32 KiB 사전은 2.5%, 110 KiB 사전은 10.7%이고, 이전 버전을 기준으로 삼는 `--patch-from`은 블롭이 서로의 기준이 되어 지우기가 엉킨다. 둘 다 이번에 쓰지 않는다.

코드가 지금 이렇게 되어 있다는 것이 설계를 정한다.

- 계층에서 바이트를 내려받는 읽기는 `verify_full` 하나다(`artifact_blobs.rs:232-246`). 객체 전체를 메모리에 올려 sha256을 계산한다. 범위 읽기는 없다. 읽기 옵션은 HEAD뿐이다(`:207`). `canon artifacts verify`와 `gate check`는 HEAD 하나와 메타데이터로 판단한다(`:205-227`).
- 쓰기는 `put` 한 곳이다. 해시를 확인하고, 같은 크기의 객체가 있으면 올리지 않고(`:261`), 아니면 `canon-sha256`과 `canon-size` 메타데이터를 붙여 올린 뒤 HEAD로 읽어 확인한다(`:264-283`). 호출자는 바이트를 통째로 읽어 넘긴다(`crates/canon-gate/src/artifact_store.rs:632-640`). 묶는 파일의 한도는 25 MiB다(`:116`).
- HEAD 검사는 객체 크기가 `canon-size`와 같은지 본다(`:221-225`). 압축하면 객체 크기가 평문 크기가 아니므로 이 비교는 그대로 쓸 수 없다.
- 목록은 파일 이름이 64자 16진수 그대로인 객체만 센다(`:300`). 삭제는 그 이름 하나를 지운다(`:313-318`). `canon artifacts prune`이 이 둘을 쓴다(`crates/canon-cli/src/artifacts_prune.rs:376`, `:646`).
- zstd는 이미 빌드에 있다. `parquet`이 `zstd 0.13.3`을 기본 기능 없이 끌어온다(`Cargo.lock:4364`, parquet의 `Cargo.toml`에 `default-features = false`). canon-store는 `parquet`을 쓴다(`crates/canon-store/Cargo.toml`). 새 크레이트가 잠금 파일에 들어오지 않는다.

## Desired outcome

- ==**객체 저장소 계층은 새 블롭을 `<prefix>sha256/<digest>.zst`로 쓴다. 다이제스트는 압축 전 바이트의 sha256이다.**== 기록이 가진 다이제스트와 "이 바이트"라는 증명은 바뀌지 않는다.
- 읽는 쪽은 두 형식을 모두 받는다. 이미 계층에 있는 `<digest>` 원본 객체는 그대로 유효하다. 어느 쪽도 없을 때만 없는 것이다.
- `canon artifacts verify --full`은 `.zst` 객체를 풀어 나온 바이트의 sha256을 다이제스트와 견준다. 풀 때 메모리는 객체 크기에 비례하지 않는다.
- 깨진 `.zst`(잘림, zstd가 아님, 풀면 다른 해시)는 `Altered`로 읽힌다. 계층에 닿지 못한 것(`Unavailable`)과 섞이지 않는다.
- `canon artifacts prune`은 한 다이제스트의 두 형식을 모두 목록에 올리고 모두 지운다.
- `canon artifacts move`가 올리는 로컬 블롭은 `.zst`가 된다.
- 로컬 git 저장소 `.canon/artifacts/sha256/`는 어떤 경로로도 압축되지 않는다. 그 디렉터리의 `<digest>.zst`는 블롭이 아니다.
- 안내문과 CHANGELOG가 형식, 이전 canon과의 호환, 되돌릴 때 쓰는 `zstd -d`를 말한다.

## User flow

1. F1 오케스트레이터는 객체 저장소 계층이 설정된 저장소에서 `canon evidence add --artifact`(또는 `artifacts store`)를 돌린다 → 블롭이 `<prefix>sha256/<digest>.zst`로 올라가고 읽어 확인된다 → 로컬 사본은 남지 않고 `gate check`가 통과한다.
2. F2 오케스트레이터는 `gate check`나 `canon artifacts verify`를 돌린다 → 한 번의 HEAD로 `.zst` 객체나 옛 원본 객체를 찾는다 → ==**어느 형식이든 한 쪽이 있으면 같은 답이 나온다.**==
3. F3 오케스트레이터는 `canon artifacts verify --full`을 돌린다 → `.zst`를 풀며 해시한다 → 평문의 다이제스트와 같을 때만 통과한다.
4. F4 오케스트레이터는 `canon artifacts move --tier <계층>`을 돌린다 → 로컬 블롭이 `.zst`로 올라가고 읽어 확인된 뒤 로컬 사본이 지워진다 → 이미 어느 형식이든 계층에 있으면 다시 올리지 않는다.
5. F5 오케스트레이터는 객체 저장소 계층이 설정된 저장소에서 `canon artifacts prune`을 돌린다 → 두 형식의 객체가 한 다이제스트로 묶여 미리보기에 오르고 `--commit`이 둘 다 지운다 → 남을 블롭은 `verify --full`을 통과한다.
6. F6 오케스트레이터는 로컬 저장소에서 같은 명령을 돌린다 → 로컬 블롭은 압축되지 않고 어제와 같다 → 로컬 디렉터리에 놓인 `<digest>.zst`는 블롭이 아니라 건드리지 않는 파일로 말한다.
7. F7 사람은 CHANGELOG와 스킬 안내문을 읽는다 → 새 형식, 이전 canon의 거동, 압축을 풀어 확인하는 법이 있다 → 계층을 읽는 canon을 모두 올린 뒤에 쓰라는 말이 있다.

## Step detail

### F1 형식과 쓰기

- ==**새 객체의 키는 `<prefix>sha256/<digest>.zst`이고, 다이제스트는 압축 전 바이트의 sha256이다.**== 내용은 zstd 프레임 하나다. 레벨 19, 사전 없음, 긴 창 옵션 없음이다. 프레임 머리에 내용 크기를 담고 내용 체크섬을 켠다. 그래서 내려받은 객체를 사람이 `zstd -d`나 `zstd -t`로 canon 없이 풀고 확인한다.
- 메타데이터 `canon-sha256`(평문 다이제스트)과 `canon-size`(평문 크기)는 그대로 쓴다. 새로 `canon-stored-size`(객체의 바이트 수)를 더한다. HEAD의 크기 비교는 `.zst`에서 객체 크기를 `canon-stored-size`와 견주고, 원본 객체는 지금처럼 `canon-size`와 견준다. 잘린 업로드를 HEAD로 잡는 힘이 형식마다 남는다.
- 메타데이터를 담지 못하는 저장소(디버그 빌드의 로컬 파일시스템 대체물, `artifact_blobs.rs:271-273`)는 지금처럼 존재만으로 판단한다. 이때 크기 비교와 잘림 검출은 `verify --full`의 몫이다.
- 어떤 바이트든 같은 규칙이다. 이미지나 영상처럼 줄지 않는 바이트도 `.zst`로 쓴다. 프레임 오버헤드는 수십 바이트이고 형식이 하나라 읽는 쪽과 시험이 하나다. 레벨 19의 CPU 시간은 이 변경이 재지 못했다. 구현하며 이 저장소의 가장 큰 블롭(약 790 KB)과 한도 크기(25 MiB) 무작위 바이트 하나를 압축한 시간을 PR에 적는다.
- 쓰는 순서는 지금과 같되 압축이 끝에 붙는다. 평문 해시를 확인하고(다르면 `Changed`, 아무것도 올리지 않음), 두 형식을 HEAD로 묻고, 있으면 올리지 않고(`AlreadyStored`), 없으면 압축해 메타데이터와 함께 PUT한 뒤 HEAD로 읽어 확인한다. 압축은 올릴 때만 한다.
- "이미 있다"의 기준은 둘 중 하나다. `.zst`가 있고 메타데이터가 말하는 평문 크기가 올릴 바이트와 같거나, 원본 객체가 지금 규칙(크기 같음)으로 있다. 옛 원본 객체가 있는 다이제스트를 `.zst`로 다시 쓰지 않는다.
- 호출자가 바이트를 통째로 읽어 넘기는 것은 그대로다(한도 25 MiB). 최대 메모리는 평문과 압축본의 합이다.
- 쓰기 중 압축이 실패하면 아무것도 올리지 않는다. 올리기가 실패하면 지금의 `ARTIFACTS_STORE_FAILED`와 같다.
- 오류 문구와 `store.display(digest)`는 canon이 쓰는 `.zst` 키를 말한다. 운영자가 그 이름으로 계층을 직접 연다.

### F2 읽기와 HEAD

- 한 다이제스트의 HEAD는 `.zst`를 먼저 묻고 `Missing`일 때만 원본 키를 묻는다. 옛 객체만 있는 다이제스트는 HEAD가 두 번이다. 계층에 닿지 못한 오류는 그 자리에서 `Unavailable`이고 다른 형식으로 넘어가지 않는다.
- 두 형식이 함께 있으면(이전 canon이 같은 다이제스트를 원본으로 다시 올렸을 때) HEAD는 `.zst`의 답을 쓴다.
- 다이제스트 문자열 검사(`is_digest`)는 `.zst`를 붙이기 전의 평문 다이제스트에만 한다. 키를 만드는 함수 하나가 접미사를 붙인다.

### F3 verify --full

- `.zst` 객체의 본문을 청크 단위로 받아 풀면서 sha256에 흘린다. 객체 전체를 메모리에 올리지 않는다. 원본 객체는 지금 하던 대로 해시한다.
- 풀린 바이트 수가 메타데이터의 `canon-size`를 넘으면 거기서 멈추고 `Altered`로 읽는다("풀면 N바이트를 넘는다"). 압축 폭탄이 계층에 있어도 시간과 메모리가 이 한도에서 끝난다. 메타데이터가 없는 저장소는 한도가 없지만 메모리는 일정하다.
- 풀기 실패(zstd 프레임이 아님, 체크섬 불일치, 잘림), 풀린 바이트의 해시가 다름, 한도 초과는 모두 `Altered`다. 받는 도중 네트워크 오류는 `Unavailable`이다.
- 두 형식이 함께 있으면 `--full`은 둘 다 판정하고 하나라도 안 맞으면 `Altered`다.

### F4 이전

- `canon artifacts move`는 `put`을 그대로 부르므로 로컬 블롭이 `.zst`로 올라간다. 올린 뒤 로컬 사본을 지우는 순서와 중단 후 다시 돌리면 이어지는 거동은 그대로다.
- 이미 계층에 있는 다이제스트(어느 형식이든)는 `AlreadyStored`이고 로컬 사본을 지우기 전에 `observe(…, full)`로 확인한다. 그 확인이 `.zst`를 푼다.
- 계층에 이미 있는 원본 객체를 `.zst`로 바꾸는 명령은 없다. 그 객체는 계속 유효하다.

### F5 prune

- 목록은 객체 이름에서 `.zst`를 떼어 다이제스트를 만들고 한 다이제스트를 한 항목으로 센다. 크기는 있는 형식들의 객체 바이트 합이다. 이 합은 로컬 사본(평문)의 바이트와 같지 않다. 미리보기의 계층 바이트는 계층이 실제로 차지하는 바이트다.
- 삭제는 `.zst`와 원본 이름을 모두 지운다. 한 쪽이 이미 없어도 오류가 아니다. 한 형식을 지우고 다른 쪽에서 끊기면 다시 돌릴 때 남은 이름이 목록에 오르므로 이어서 지운다.
- 계획 다이제스트가 읽은 상태에 형식별 객체가 들어간다. 미리보기와 `--commit` 사이에 형식이 바뀌어도 `PLAN_STALE`이다.
- 계층 접두사를 말하는 곳이 다이제스트를 빈 문자열로 넘겨 키를 만든다(`artifacts_prune.rs:446`의 `store.display("")`, `artifact_store.rs:602`의 `key("")`). 접미사가 붙으면 `sha256/.zst`가 찍힌다. 접두사를 만드는 별도 함수를 둔다.

### F6 로컬 저장소

- 로컬 저장소의 `stored_blob`, `local_blobs`, 쓰기 경로는 바뀌지 않는다. `local_blobs`는 이미 64자 16진수가 아닌 이름을 블롭이 아닌 것으로 건너뛴다(`artifacts.rs:336-340`). `<digest>.zst`가 로컬 디렉터리에 있어도 같은 길이다.

### F7 의존과 문서

- `canon-store/Cargo.toml`에 `zstd = { version = "0.13", default-features = false }`을 더한다. parquet과 같은 기능 집합이라 `Cargo.lock`은 canon-store의 의존 한 줄만 늘고 새 크레이트는 없다. 같은 C 소스를 이미 모든 대상에서 링크한다. `canon-gate`는 직접 의존하지 않는다.
- CHANGELOG의 Added에 새 형식을, Changed에 "계층을 읽는 모든 canon을 올린 뒤에 쓰라"는 호환 항목을 적는다. 이전 canon(0.39 이하)은 `.zst` 객체를 찾지 못한다. 그 canon의 `gate check`는 그 블롭을 없는 것으로 읽어 위반을 내고 `prune`은 그 객체를 목록에 올리지 않아 지우지 않는다. 통과로 읽는 길은 없다.
- 스킬 안내문 `canon-storage.md`(167행)와 `canon-gate.md`(624행)의 키 서술, `artifacts move`의 도움말을 새 키로 고친다.

## Acceptance

- F1:
  - 계층이 있는 저장소에서 `evidence add --artifact`를 돌리면 대체 저장소에 `artifacts/sha256/<digest>.zst`가 있고 `<digest>` 원본 이름은 없다. `.zst`를 풀면 묶은 파일과 바이트가 같고 로컬 `.canon/artifacts`는 없다. `gate check`가 통과한다.
  - 실패 경로: 올리는 사이 파일이 바뀌어 해시가 다르면 `Changed`로 거절되고 계층에 아무 객체도 없다.
- F2:
  - ==**같은 다이제스트가 `.zst`로만, 원본으로만 있는 두 저장소에서 `gate check`와 `artifacts verify`의 답이 같다.**==
  - 실패 경로: 두 형식 모두 없으면 `missing`이고 `gate check`는 `stale-evidence`를 낸다. 메타데이터가 있는 저장소에서 `.zst`의 `canon-stored-size`와 객체 크기가 다르면 `Altered`다(`canon-store`의 단위 시험, 메타데이터를 담는 저장소 대체물).
- F3:
  - `verify --full`이 `.zst` 객체를 풀어 평문 다이제스트와 같은 것을 확인하고 통과한다. 풀면서 메모리에 올리는 양이 객체 크기에 비례하지 않는다는 것은 청크 단위로 먹이는 단위 시험이 확인한다.
  - 실패 경로: `.zst`의 한 바이트를 바꾼 것, 뒤를 자른 것, zstd가 아닌 파일로 바꾼 것은 각각 `Altered`이고 종료 값 1이다. 풀면 `canon-size`를 넘는 객체는 한도에서 멈춰 `Altered`다. 계층에 닿지 못하면 종료 값 2이고 통과가 아니다.
- F4:
  - 로컬 블롭 둘을 `move`로 옮기면 두 `.zst`가 계층에 있고 로컬 사본이 사라지며 `verify --full`이 통과한다. 이미 원본 객체가 있는 다이제스트는 다시 올리지 않고(`.zst`가 생기지 않는다) 로컬 사본이 지워진다.
  - 실패 경로: 계층에 이미 있는 `.zst`가 풀어서 해시가 맞지 않으면 `ARTIFACTS_BLOB_CORRUPT`로 거절되고 그 로컬 사본이 남는다.
- F5:
  - 같은 다이제스트가 `.zst`와 원본으로 함께 있고 어떤 기록도 묶지 않은 저장소에서 `prune --commit`이 둘 다 지운다. 미리보기의 계층 객체 수는 다이제스트 수이고 바이트는 두 객체의 합이다.
  - 실패 경로: 두 번째 이름을 지우다 계층이 거부하면 거절하고 지운 만큼을 말한다. 다시 돌리면 남은 이름만 지운다. 남기는 블롭은 어느 경우도 건드리지 않는다.
- F6:
  - 로컬 저장소에서 `evidence add`, `artifacts store`, `prune`이 `.zst`를 만들지 않고 이전과 같은 파일을 남긴다.
  - 실패 경로: 로컬 디렉터리에 놓인 `<digest>.zst`는 `stored_blob`이 `Missing`으로 읽고, `verify`와 `move`는 블롭 아닌 것으로 건너뛰며, `prune`은 지우지 않는다.
- F7:
  - CHANGELOG에 새 형식과 호환 항목이 있고, `canon-storage.md`와 `canon-gate.md`가 `.zst` 키를 말한다.
  - 실패 경로: 항목이 없으면 릴리스 확인 시험이 실패한다.
- 작업공간 전체 테스트, `just atip-deep`, `canon gate check`가 통과한다.

## Questions and assumptions

- 결정: 압축은 객체 저장소 계층만이다. 로컬 git 저장소는 평문 그대로다. 버린 설계는 로컬에 `.zst`를 두는 것이다. 이 저장소에서 측정한 대로 팩이 1.9배 커지고 작업 트리만 줄어든다.
- 결정: 새 객체는 모두 `.zst`다. 줄지 않는 바이트도 같다. 버린 설계는 줄지 않으면 원본으로 쓰는 것이다. 새 객체의 형식이 둘이 되고 시험과 읽기 경로가 갈린다. 얻는 것은 프레임 수십 바이트다.
- 결정: 레벨 19, 사전 없음, `--patch-from` 없음이다. 버린 설계: 사전은 300 KB 보고서에서 2~11%라 줄이는 값이 작고 블롭 사이 의존을 만든다. `--patch-from`은 한 블롭을 읽는 데 기준 블롭의 사슬이 필요하고 지울 때 기준을 남겨야 한다. 사전은 작은 블롭이 많아지면 새 변경으로 다룬다. 레벨 19의 CPU 시간은 재지 못했다. 구현하며 잰 값이 감당되지 않으면 레벨만 낮추고 형식은 바꾸지 않는다(프레임은 레벨을 담지 않는다).
- 결정: 옛 원본 객체는 있는 그대로 유효하다. 읽기는 두 형식을 받고 쓰기는 새 형식뿐이다. 다시 압축하는 명령은 이번에 없다. `prune`이 묶이지 않은 블롭(이 저장소는 94%)을 먼저 지우므로 남는 원본 객체는 적다. 잰 값: 묶인 블롭 9개 3,712,209바이트.
- 결정: 읽기는 `.zst`를 먼저, 원본은 그다음이다. 버린 설계는 접두사 목록으로 어떤 형식이 있는지 먼저 보는 것이다. HEAD 한 번이면 되는 경우를 목록 한 번으로 바꾸고 접두사가 큰 계층에서 느리다.
- 결정: 풀린 바이트는 `canon-size`에서 멈춘다. 메타데이터가 없는 저장소는 한도 없이 풀지만 메모리는 일정하다. 버린 설계는 25 MiB 같은 고정 한도다. `--max-artifact-mib`로 올린 블롭이 `verify --full`에서 거짓 `Altered`가 된다.
- 결정: `canon-stored-size`를 새 메타데이터로 둔다. 버린 설계는 `.zst`의 HEAD 크기 검사를 없애는 것이다. 잘린 업로드를 `--full` 전까지 놓친다.
- 질문: 이전 canon(0.39 이하)이 같은 계층을 읽는 저장소에서 새 canon이 첫 `.zst`를 쓰는 순간 이전 canon의 `gate check`는 그 블롭을 없는 것으로 읽어 빨간불을 낸다(통과로 읽지는 않는다). 기본안은 `canon.yaml` 설정 없이 항상 `.zst`로 쓰고 CHANGELOG에 "모든 canon을 먼저 올리라"고 적는 것이다. 설정 키(`artifact_store.compress: false` 같은 끄는 스위치)를 두면 올리는 동안 쓰기를 미룰 수 있지만 키가 하나 늘고, 켜지 않은 저장소는 압축의 득을 놓친다. 이전 canon과 같은 계층을 나눠 쓰는 저장소가 있는지 소유자가 안다.
- 질문: 이미 계층에 있는 원본 객체를 `.zst`로 바꾸는 명령(`artifacts recompress`, 미리보기와 계획 다이제스트, 원본을 지우는 일은 되돌릴 수 없음)을 이번에 넣을지. 기본안은 넣지 않는다. 이슈 #138의 "`artifacts store` can recompress"는 이 명령을 뜻할 수 있다. 소유자가 이번에 원한다고 하면 이 변경에 흐름 하나(F8)를 더한다.
- 실패 양식: 한 다이제스트에 두 형식이 함께 있고 한 쪽이 깨진 경우, HEAD는 `.zst` 쪽만 보므로 `--full`이 아닌 `gate check`는 놓칠 수 있다. 이전 canon이 같은 다이제스트를 원본으로 올릴 때만 생긴다.
- 실패 양식: 압축 중 메모리가 평문과 압축본의 합이다. 한도 25 MiB 안에서 최대 약 50 MiB다. `move`는 한도 없이 로컬 블롭을 읽는다. 이 부분은 지금도 그렇다.

## Out of scope

- 로컬 git 저장소의 압축.
- 압축 사전, `--patch-from`, 블롭 사이 중복 제거.
- 이미 있는 원본 객체를 다시 압축하는 명령(질문 2의 기본안).
- 보고서 추출본 저장, 이미지와 영상 변환, 매체별 계층 경로 지정(#138의 나머지 항목).
- 범위 읽기. 지금 이를 쓰는 곳이 없다.
- `canon tier age`가 쓰는 parquet의 압축.
- 압축을 끄는 설정 키(질문 1의 기본안).

Approved: Journey Hwang 2026-11-09T10:05:36Z — "승인 (canon 0.40 브리프 4개와 정책, 권장 기본값)" (relayed by planner planner-oco via ask) (code tree 95efa39f9e6567eca108728d0627f3b4410a1aec) (brief sha256:3db024b355c34165b1936d0397317e71bd4bd7bf95cb8fceea01b0f2e6bb47c0)
