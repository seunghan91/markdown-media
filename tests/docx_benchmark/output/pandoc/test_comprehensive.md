# Document Title (H1)

This is body text under the main heading. It tests basic paragraph
extraction.

## Section One (H2)

Body text in section one.

### Subsection 1.1 (H3)

Body text in subsection 1.1.

#### Sub-subsection 1.1.1 (H4)

Deep nested content.

## Formatting Tests (H2)

**Bold text**, *italic text*, ***bold italic***, ~~strikethrough~~, and
normal text.

Small text (8pt) Normal text (11pt) Large text (18pt)

## List Tests (H2)

- Bullet item 1

- Bullet item 2

- Bullet item 3

1.  Numbered item 1

2.  Numbered item 2

3.  Numbered item 3

- Top level bullet

<!-- -->

- Nested bullet level 2

<!-- -->

- Nested bullet level 3

<!-- -->

- Back to top level

## Table Tests (H2)

  -----------------------------------------------------------------------
  Name                    Age                     City
  ----------------------- ----------------------- -----------------------
  Alice                   30                      Seoul

  Bob                     25                      Busan

  Charlie                 35                      Daejeon
  -----------------------------------------------------------------------

Table with merged cells:

+---------------------------------------------------+-----------------------+
| Merged A+B                                        | C                     |
+=========================+=========================+=======================+
| Vertical Merge          | D                       | E                     |
|                         +-------------------------+-----------------------+
|                         | F                       | G                     |
+-------------------------+-------------------------+-----------------------+

## Hyperlink Tests (H2)

Visit [[MDM GitHub
Repository]{.underline}](https://github.com/seunghan91/markdown-media)
for more info.

Also check [[Dcode Labs]{.underline}](https://dcode-labs.com).

## Footnote Tests (H2)

This sentence has a footnote reference[^1]

## 한국어 콘텐츠 테스트 (H2)

이것은 한국어 텍스트입니다. MDM 파서의 한국어 처리 능력을 테스트합니다.

**굵은 한글**, *기울임 한글*, ***굵은 기울임 한글***

가) 첫 번째 항목

나) 두 번째 항목

다) 세 번째 항목

## Mixed Language (H2)

The **ActionCable** 채널의 성능이 *WebSocket* 연결보다 느립니다.

## Blockquote Test (H2)

> This is a quote from an important source.

Content before rule.

\-\--

Content after rule.

[^1]:
