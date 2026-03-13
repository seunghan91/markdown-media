"""
pytest 설정 파일.

루트 __init__.py의 상대 임포트 문제를 해결하기 위한 설정입니다.
"""
import sys
import os
import types

# parser-py 디렉터리를 sys.path에 추가하여 mdm 패키지를 찾을 수 있도록 합니다
_pkg_root = os.path.dirname(os.path.abspath(__file__))
if _pkg_root not in sys.path:
    sys.path.insert(0, _pkg_root)

# 루트 __init__.py가 상대 임포트로 실패하는 것을 방지합니다.
# pytest가 parser-py 디렉터리를 패키지로 인식할 때 __init__.py를 임포트하려 하는데,
# 이 파일은 .ocr_processor 등 상대 임포트를 사용하므로 단독 실행 시 실패합니다.
# 더미 모듈을 미리 등록하여 상대 임포트 실패를 방지합니다.
def _make_dummy_module(name):
    mod = types.ModuleType(name)
    mod.__spec__ = None
    return mod

_pkg_name = 'parser_py'
if _pkg_name not in sys.modules:
    # parser-py 디렉터리를 패키지로 등록
    _pkg = types.ModuleType(_pkg_name)
    _pkg.__path__ = [_pkg_root]
    _pkg.__package__ = _pkg_name
    _pkg.__spec__ = None
    sys.modules[_pkg_name] = _pkg

    # 상대 임포트 대상 서브모듈들을 더미로 등록
    for _submod in ['ocr_processor', 'ocr_bridge', 'pdf_processor', 'hwp_to_svg']:
        _full = f'{_pkg_name}.{_submod}'
        if _full not in sys.modules:
            sys.modules[_full] = _make_dummy_module(_full)

# 루트 __init__.py 수집 제외
collect_ignore = ["__init__.py"]
