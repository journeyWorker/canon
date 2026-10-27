# 0.31 HTML 주석 없는 brief

<!-- canon:section intent role=context -->
## Intent

brief에는 사람이 읽지 않는 HTML 주석이 네 종류 들어 있다. 섹션 표지, 안내 문구, 면제, chore 표지다. 섹션 표지는 제목을 brief의 언어로 쓰고도 섹션을 키로 찾으려고 만들었지만, 역할은 키가 이미 정하고 지금 brief는 거의 영어 제목이라 표지가 제목을 되풀이한다. 면제는 리뷰어가 봐야 할 정보인데 주석 안에 숨는다. 에이전트가 편집하다 표지를 떨어뜨리면 섹션이 사라진 것으로 읽힌다.

Source: GitHub 이슈 #121

0.31 순서: CI → 버그 수정 → 승인과 adopt → brief 포맷 → CLI id → 검색. 여섯 brief는 함께 승인받는다.

<!-- canon:section outcome role=normative -->
## Desired outcome

- ==**brief 본문에는 제목, 산문, 맨 위의 frontmatter, 끝의 `Approved:` 줄만 남는다.**== HTML 주석은 없다.
- 섹션은 영어 `##` 제목으로 찾는다. 키와 역할은 제목에서 정해진다. `language:`는 산문 규칙, 안내 문구, 템플릿 문구의 언어만 정한다.
- 제목을 잘못 쓰면 `brief check`가 가장 비슷한 제목을 알려 준다. 섹션이 없다고만 하지 않는다.
- 면제는 frontmatter에 YAML로 적혀 GitHub 화면에서 보인다. 승인 digest가 frontmatter를 덮으므로 승인 뒤에 면제를 더하면 다시 승인받아야 한다.
- 새 brief의 섹션은 비어 있고, 빈 섹션이 곧 아직 쓰지 않은 섹션이다. 안내 문구는 `canon brief show`와 스킬이 보여 준다.
- `canon migrate` 한 번으로 옛 brief가 새 형식이 되고, 승인된 brief는 승인된 채로 남는다.

<!-- canon:section flow role=normative -->
## User flow

1. F1 에이전트가 영어 `##` 제목으로 brief를 쓰고 `canon brief check`를 실행한다 → canon이 제목으로 섹션을 찾는다 → 키와 역할이 제목에서 정해지고, 제목을 잘못 쓴 섹션은 가장 비슷한 제목과 함께 보고된다.
2. F2 에이전트가 규칙 하나를 한 섹션에서 면제하려 한다 → frontmatter의 `waive` 목록에 항목을 적는다 → `brief check`가 그 면제를 읽고, 출력과 GitHub 화면 모두에서 보인다.
3. F3 에이전트가 `canon change new`를 실행한다 → canon이 빈 섹션만 있는 brief를 쓴다 → `brief check`가 빈 섹션을 보고하고, `canon brief show`와 스킬이 섹션별 안내 문구를 보여 준다.
4. F4 사용자가 옛 형식 brief가 있는 레포에서 canon을 실행한다 → canon이 주석 표지나 한국어 제목을 찾는다 → 읽는 명령은 경고와 함께 `canon migrate`를 다음 단계로 내고, 쓰는 명령은 같은 안내로 거절된다.
5. F5 사용자가 `canon migrate`를 실행한다 → canon이 다시 쓸 brief와 다시 묶을 승인을 계산한다 → ==**아무것도 쓰지 않고 계획과 계획 digest를 보여 준다.**==
6. F6 사용자가 그 digest로 `canon migrate --commit`을 실행한다 → canon이 brief를 새 형식으로 다시 쓴다 → 승인된 brief는 여전히 승인된 상태이고, 본문의 산문은 바뀌지 않는다.

<!-- canon:section detail role=normative -->
## Step detail

### F1 영어 제목으로 찾기

- 섹션 제목은 `##` 한 단계다. feature brief의 제목과 키는 Intent→`intent`, Desired outcome→`outcome`, User flow→`flow`, Step detail→`detail`, Acceptance→`acceptance`, Questions and assumptions→`questions`, Out of scope→`out-of-scope`다. chore brief는 Intent, Category, Invariants, Checks, Out of scope이고 키는 `intent`, `category`, `invariants`, `checks`, `out-of-scope`다.
- 역할은 지금 표가 정하는 값과 같다. Intent, Category, Out of scope는 context다. Questions and assumptions는 question이고 나머지는 normative다. 제목 비교는 대소문자를 구분하고 앞뒤 공백만 무시한다.
- 코드 펜스 안의 `##` 줄은 제목이 아니다. 알려진 제목이 아닌 `##` 줄은 앞 섹션의 본문으로 남는다. 지금 표지 없는 제목이 그렇게 읽히는 것과 같다.
- 섹션이 없고 그 자리에 가까운 제목이 있으면 섹션 없음 blocker의 hint가 그 제목을 이름으로 부른다. 영어 오타나 대소문자(`## Intnet`)와 shipped 한국어 제목(`## 의도`)이 여기에 든다. 한국어 제목에는 `canon migrate`도 함께 안내한다.
- 섹션 순서, 중복, 빈 섹션, 흐름 id, 상세 제목 대응의 규칙은 그대로이고 제목 기준으로 읽는다. 알려진 제목이 하나도 없는 brief는 지금처럼 옛 PRD 이전 brief로 읽는다. ==**섹션을 찾는 규칙은 영어 제목 하나뿐이다.**==
- 표지가 없어졌으므로 섹션의 줄 번호는 제목 줄이다. frontmatter가 줄 번호를 밀지만 보고하는 번호는 파일의 실제 줄이다.
- `language:`가 정하는 것은 어떤 산문 규칙을 쓸지, hint와 안내 문구를 어느 언어로 낼지, 새 brief의 템플릿 문구 언어다. 제목에는 영향이 없다.
- 섹션과 흐름 단계를 읽는 모든 곳이 같은 파서를 쓴다. 대상은 brief check, brief show, status의 brief 줄이다. 대시보드의 brief 페이지와 플랜 가져오기의 Intent 요약도 같다.

### F2 frontmatter의 면제

- frontmatter는 파일 맨 앞의 `---` 줄에서 다음 `---` 줄까지이고 YAML이다. 제목 `# …`은 그 뒤에 온다.
- 항목은 `rule`, `section`, `reason`이고 선택으로 `match`가 있다. 의미는 지금 주석 면제와 같다. 한 규칙을 한 섹션에서, 또는 섹션 키 `brief`로 brief 전체 발견에서 면제한다. `match`가 있으면 그 글을 담은 발견만 면제한다.
- `reason`은 비어 있으면 안 되고, 규칙 id와 섹션 키는 알려진 값이어야 한다. 어긋난 항목은 지금처럼 면제 형식 blocker다. frontmatter가 YAML로 읽히지 않거나 모르는 키가 있어도 blocker이고, 그 brief는 면제 없이 읽는다.
- chore brief는 frontmatter에 `kind: chore`를 둔다. 이 키가 없으면 feature brief다.
- 출력의 면제 목록은 지금과 같다. 규칙의 심각도와 적용 여부를 보여 준다.
- 승인 digest는 `Approved:` 줄을 뺀 파일 전체의 해시다. frontmatter가 그 안에 있으므로 면제 추가, 삭제, 이유 수정은 승인 뒤에 승인 이후 변경으로 읽힌다.

### F3 빈 템플릿과 안내 문구

- `canon change new`는 frontmatter 없이(chore는 `kind: chore`만), 제목과 빈 `##` 섹션만 쓴다. Intent에는 지금처럼 변경 제목 한 줄이 들어간다. 템플릿 끝의 승인 안내 주석도 쓰지 않는다.
- 빈 섹션은 지금처럼 blocker로 보고된다. 안내 문구가 파일에 없으므로 이 blocker의 hint가 해당 섹션의 안내 문구를 담는다.
- `canon brief show`는 비어 있는 섹션마다 안내 문구를 `language:`의 언어로 보여 준다. brief-shape 스킬은 섹션 표와 안내 문구를 같은 문구로 싣는다.
- 안내 문구는 미완성 판정 대상이 아니다. 판정 기준은 두 가지다. 섹션이 비어 있는지, 미완성 표시가 남았는지.

### F4 옛 형식 감지

- 옛 형식은 `canon:section`, `canon:hint`, `canon:waive`, `canon:kind` 주석이 하나라도 있거나 shipped 한국어 제목이 있는 brief다. 승인 안내 주석은 감지 기준이 아니다.
- 옛 형식 brief에 `brief check`는 섹션 규칙을 적용하지 않고 blocker 하나로 `canon migrate`를 안내한다. 그 면제 주석을 읽지 않는 것이 조용히 규칙을 되살리는 일이 되지 않게 하기 위해서다.
- 읽는 명령(`status`, `gate check`, `context`, `brief show`)은 설정 오류 없이 경고하고 `canon migrate`를 다음 단계 첫 줄로 낸다. 쓰는 명령은 같은 hint로 거절된다. 0.30 이름 변경 때 옛 레포를 다룬 방식과 같다.
- 승인 줄이 digest와 맞는 옛 brief는 migrate 전에도 승인된 것으로 읽힌다. 옛 형식 감지가 승인 상태를 바꾸지 않는다.

### F5 미리보기

- 변경 디렉터리 아래의 brief와 보관된 변경의 brief를 모두 읽는다.
- 계획은 brief마다 지울 주석 수, 옮길 면제와 chore 표지, 영어로 바꿀 제목을 보여 준다. 다시 묶을 `Approved:` 줄은 옛 digest와 새 digest를 함께 보인다.
- 다시 묶는 조건은 이렇다. 옛 digest가 옛 본문과 맞아야 한다. 바뀌는 것은 표지, 안내 문구, 승인 안내 주석의 삭제와 면제, chore 표지의 이동, 제목 번역뿐이어야 한다. 계획은 이 조건을 문장으로 밝힌다.
- 옛 digest가 맞지 않는 brief(승인 뒤에 편집된 brief)는 형식만 바꾸고 `Approved:` 줄은 건드리지 않는다. 계획이 그 brief를 따로 이름으로 부른다.
- digest가 없는 `Approved:` 줄과 승인 없는 brief는 형식만 바꾼다.
- chore 변경 리뷰가 옛 brief digest에 핀을 박고 있으면 그 리뷰를 계획이 나열한다.
- 아무것도 쓰지 않고, 커밋하지 않은 변경이 이 경로에 있으면 거절한다. 이미 새 형식이면 할 일이 없다고 말한다.

### F6 적용

- 계획 digest가 다르면 계획 변경으로 거절하고 아무것도 쓰지 않는다.
- 면제 주석은 `waive` 항목으로 옮기고 `reason`과 `match`를 그대로 둔다. 같은 규칙·섹션·이유 항목이 이미 있으면 겹치지 않게 하나만 쓴다.
- 한국어 제목은 같은 키의 영어 제목으로 바꾼다. 본문의 산문, 흐름 id, 강조한 핵심 문장은 그대로다.
- 다시 묶을 brief는 새 본문의 digest를 계산해 기존 `Approved:` 줄의 누가·언제·말은 그대로 두고 digest만 바꾼다.
- chore 변경 리뷰의 핀은 계획이 나열한 것만 새 digest로 바꾸고, 쓴 리뷰는 0.30 이름 변경 때처럼 계획에 한 줄씩 보인다.
- canon 자기 레포의 0.30·0.31 brief는 옛 형식으로 쓰고 승인받은 것이므로 이 변경이 같은 방식으로 옮긴다.
- 두 번째 실행은 할 일이 없다. 중간에 멈춘 실행은 다음 실행이 끝낸다.

<!-- canon:section acceptance role=normative -->
## Acceptance

- F1: 영어 제목만 있는 brief가 표지 없이 `brief check`를 통과하고, `--json`의 섹션이 올바른 키와 역할을 낸다. 한국어 산문 brief도 같다. 실패 경로: ==**`## Intnet`은 섹션이 없다는 말과 함께 `Intent`를 가리키고, 지운 섹션과 순서가 바뀐 섹션은 각각 섹션 없음과 순서 blocker로 보고된다.**== 대소문자만 틀린 제목도 가까운 제목으로 불린다.
- F1 언어: 한국어 brief에서 제목은 영어 그대로 통과하고 산문 규칙은 한국어로 적용된다. `language:`를 바꿔도 제목 판정은 같다.
- F1 읽는 곳: 새 형식 brief에서 brief show, status, 대시보드가 옛 형식과 같은 섹션과 흐름 단계를 낸다. 플랜 가져오기의 Intent 요약도 같다.
- F2: frontmatter 면제가 규칙을 한 섹션에서 걸러 내고, `match` 면제는 그 글을 담은 발견만 거르며, 출력의 면제 목록에 보인다. 실패 경로: 이유가 비었거나 규칙 id, 섹션 키, 키를 모르면 면제 형식 blocker가 되고 아무것도 면제되지 않는다. 읽을 수 없는 YAML도 같다. 승인된 brief에 면제를 더하면 승인 이후 변경으로 읽힌다.
- F2 chore: `kind: chore` frontmatter가 있는 brief는 chore로 읽고, 없는 brief는 feature로 읽는다.
- F3: 새로 쓴 brief(feature와 chore)에 HTML 주석이 없고 섹션이 비어 있으며, `brief check`가 빈 섹션마다 blocker를 낸다. 실패 경로: 빈 섹션만 채우고 미완성 표시를 남기면 그 blocker가 남는다. `brief show`는 `language: ko`에서 한국어, 아니면 영어로 안내 문구를 낸다.
- F4: 0.30 형식 fixture에서 `status`, `gate check`, `context`가 경고하고 `canon migrate`를 첫 다음 단계로 낸다. 실패 경로: 옛 형식 brief에서 쓰는 명령(`evidence add`)은 같은 hint로 거절되고 아무것도 쓰지 않는다. 이 brief의 면제 주석은 읽히지 않는다.
- F5: 미리보기 뒤 작업 트리가 바이트 단위로 같다. 계획은 brief마다 지울 주석, 옮길 면제, 바꿀 제목, 다시 묶을 digest를 보인다. 실패 경로: 커밋하지 않은 변경이 변경 디렉터리에 있으면 거절되고, 승인 뒤에 편집된 brief는 다시 묶지 않는 brief로 따로 나열된다.
- F6: canon 자기 레포의 모든 변경 brief와 한국어 제목 fixture를 옮긴다. 옮긴 뒤 승인된 brief는 모두 승인된 채로 읽히고 `BRIEF_CHANGED_SINCE_APPROVAL`이 나는 brief가 없다. 옮긴 brief의 blocker와 advisory는 면제를 포함해 옮기기 전과 같고, 주석과 제목 번역을 뺀 산문은 바이트 단위로 같다. 실패 경로: 다른 digest로 적용하면 거절되고 아무것도 바뀌지 않으며, 옛 digest가 맞지 않던 brief는 옮긴 뒤에도 `BRIEF_CHANGED_SINCE_APPROVAL`로 남는다. 두 번째 실행은 할 일이 없다고 말한다.
- 영향을 받는 시나리오를 새 형식으로 고친다. 대상은 brief check, change new, 대시보드 brief 페이지, brief 스킬 문서, migrate다. 표지와 면제와 안내 문구를 쓰는 conformance 케이스도 고치고 마이그레이션 케이스를 하나 더한다.

<!-- canon:section questions role=question -->
## Questions and assumptions

- 질문, 승인 무결성: `Approved:` digest를 사람 없이 다시 묶어도 되는가? 승인은 사람의 행위다. 가정: 된다. 단 옛 digest가 옛 본문과 맞고 바뀐 것이 표지, 안내 문구, 승인 안내 주석의 삭제, 면제의 이동, 제목 번역뿐일 때만이다. 계획이 이 조건과 다시 묶을 brief 목록을 먼저 보여 주고 사람이 `--commit`으로 승인한다. 승인 뒤에 편집된 brief는 다시 묶지 않는다.
- 질문: chore 변경 리뷰가 brief digest에 핀을 박고 있어 brief를 다시 묶으면 그 리뷰가 오래된 것이 된다. 서명한 리뷰의 핀을 migrate가 바꿔도 되는가? 가정: 바꾼다. 옛 digest에 핀이 있던 리뷰만 새 digest로 옮기고 계획에 한 줄씩 보인다. 이전 migrate도 리뷰 기록을 계획에 나열하고 다시 썼다. 반대라면 리뷰어가 chore를 다시 리뷰해야 한다.
- 질문: 이슈가 말하지 않은 두 주석을 어떻게 하는가? chore 표지(`canon:kind chore`)와 템플릿 끝의 승인 안내 주석이다. 본문에 주석이 남지 않게 하려면 둘 다 없어져야 한다. 가정: chore 표지는 frontmatter의 `kind: chore`로 옮기고, 승인 안내 주석은 쓰지 않으며 안내는 `brief show`와 스킬이 한다.
- 질문: 옛 형식 brief를 새 canon이 migrate 전에 어떻게 읽는가? 가정: F4처럼 감지해 경고하고 쓰기를 거절한다. 주석을 말없이 무시하면 면제가 풀리고 chore가 feature로 읽힌다.
- 가정: 제목 비교는 정확히 일치(대소문자 구분)이고, 가까운 제목은 대소문자 차이나 두 글자 이내의 편집 거리다. 알려진 제목이 아닌 `##`는 앞 섹션의 본문이다.
- 가정: 한국어가 아닌 다른 언어(ja, zh)로 쓴 제목은 shipped 제목이 아니므로 migrate가 번역하지 않는다. 그 brief는 사람이 영어 제목으로 고친다. 이슈가 말한 `ko` 제목만 바꾼다.
- 가정: frontmatter의 알 수 없는 키는 오타를 숨기지 않도록 blocker다. 이후 키를 추가하는 변경이 이 목록을 넓힌다.
- 가정: 승인 줄 위치(파일 끝)와 `Source:` 줄 형식은 바뀌지 않는다.

<!-- canon:section out-of-scope role=context -->
## Out of scope

- 일반 Markdown directive(`:::name{…}`): CommonMark가 아니고 GitHub가 글자 그대로 보여 준다.
- 섹션 집합과 순서의 변경.
- 다른 언어(ja, zh) 제목의 번역.
- 승인 줄(`Approved:`)의 형식 변경.

<!-- canon:waive prose.length brief "마이그레이션과 읽는 곳 전체를 다루는 변경이라 단계마다 조건을 빼면 사실이 빠진다" -->
<!-- canon:waive prose.length detail "여섯 단계의 규칙과 마이그레이션 조건을 모두 담아야 하는 변경이다" -->
<!-- canon:waive prose.length acceptance "여섯 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다" -->
<!-- canon:waive prose.length questions "승인 무결성과 리뷰 핀이 정책 질문이라 가정과 함께 적어야 한다" -->
<!-- canon:waive prose.code-citation acceptance "BRIEF_CHANGED_SINCE_APPROVAL은 사용자가 받는 오류 코드다" match="BRIEF_CHANGED_SINCE_APPROVAL" -->
