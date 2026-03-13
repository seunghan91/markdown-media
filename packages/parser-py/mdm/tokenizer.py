"""
MDM 참조 문법을 토큰화합니다.
"""
import re
from typing import Any, Dict, List, Optional


class Tokenizer:
    """MDM 참조 문법 토크나이저.

    ![[name:preset | attr=val]] 형식의 MDM 참조를 찾아 토큰 배열로 변환합니다.
    """

    # MDM 참조 패턴: ![[name:preset | attributes]]
    MDM_REFERENCE_PATTERN = re.compile(r'!\[\[([^\]]+)\]\]')
    # 참조 구성 요소 분리: name, preset, attributes
    RESOURCE_PARTS_PATTERN = re.compile(r'^([^:|]+)(?::([^|]+))?(?:\s*\|\s*(.+))?$')
    # 속성 파싱: key=value, key="value", key='value', key (boolean)
    ATTRIBUTE_PATTERN = re.compile(r'(\w+)(?:=(?:"([^"]*)"|\'([^\']*)\'|([^\s]+)))?')

    def tokenize(self, text: str) -> List[Dict[str, Any]]:
        """텍스트에서 MDM 참조를 찾아 토큰화합니다.

        Args:
            text: 파싱할 텍스트

        Returns:
            토큰 배열. 각 토큰은 type='text' 또는 type='mdm-reference'.
        """
        tokens: List[Dict[str, Any]] = []
        last_index = 0

        for match in self.MDM_REFERENCE_PATTERN.finditer(text):
            # 이전 텍스트 추가
            if match.start() > last_index:
                tokens.append({
                    'type': 'text',
                    'value': text[last_index:match.start()]
                })

            # MDM 참조 파싱
            reference = match.group(1)
            parsed = self.parse_reference(reference)

            tokens.append({
                'type': 'mdm-reference',
                'raw': match.group(0),
                **parsed
            })

            last_index = match.end()

        # 나머지 텍스트 추가
        if last_index < len(text):
            tokens.append({
                'type': 'text',
                'value': text[last_index:]
            })

        return tokens

    def parse_reference(self, reference: str) -> Dict[str, Any]:
        """MDM 참조 문자열을 파싱합니다.

        Args:
            reference: '![[...]]' 안쪽 문자열

        Returns:
            name, preset, attributes 를 포함한 딕셔너리

        Raises:
            ValueError: 유효하지 않은 MDM 참조 형식일 때
        """
        match = self.RESOURCE_PARTS_PATTERN.match(reference)

        if not match:
            raise ValueError(f"Invalid MDM reference: {reference}")

        name_raw, preset_raw, attrs_str = match.group(1), match.group(2), match.group(3)
        attributes = self.parse_attributes(attrs_str) if attrs_str else {}

        return {
            'name': name_raw.strip(),
            'preset': preset_raw.strip() if preset_raw else None,
            'attributes': attributes
        }

    def parse_attributes(self, attrs_str: str) -> Dict[str, Any]:
        """속성 문자열을 파싱합니다.

        key=value, key="value", key='value', key (boolean True) 형식을 지원합니다.
        숫자 문자열은 int 또는 float으로 자동 변환합니다.

        Args:
            attrs_str: 속성 문자열

        Returns:
            파싱된 속성 딕셔너리
        """
        attributes: Dict[str, Any] = {}

        for match in self.ATTRIBUTE_PATTERN.finditer(attrs_str):
            key = match.group(1)
            # 우선순위: 이중따옴표 > 단따옴표 > 따옴표없는값 > True (boolean)
            double_quoted = match.group(2)
            single_quoted = match.group(3)
            unquoted = match.group(4)

            if double_quoted is not None:
                raw_value: Any = double_quoted
            elif single_quoted is not None:
                raw_value = single_quoted
            elif unquoted is not None:
                raw_value = unquoted
            else:
                # 값 없음 → boolean True
                raw_value = True

            # 숫자 변환
            if isinstance(raw_value, str):
                if re.fullmatch(r'\d+', raw_value):
                    raw_value = int(raw_value)
                elif re.fullmatch(r'\d+\.\d+', raw_value):
                    raw_value = float(raw_value)
                elif raw_value == 'true':
                    raw_value = True
                elif raw_value == 'false':
                    raw_value = False

            attributes[key] = raw_value

        return attributes

    def reconstruct(self, tokens: List[Dict[str, Any]]) -> str:
        """토큰 배열을 원본 텍스트로 재구성합니다 (디버깅용).

        Args:
            tokens: 토큰 배열

        Returns:
            재구성된 텍스트
        """
        parts = []
        for token in tokens:
            if token['type'] == 'text':
                parts.append(token['value'])
            elif token['type'] == 'mdm-reference':
                parts.append(token.get('raw', ''))
        return ''.join(parts)
