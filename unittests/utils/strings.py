import pytest

from bo4e_cli.utils.strings import camel_to_snake


class TestStringManipulations:
    @pytest.mark.parametrize(
        "camel_case_str, snake_case_str",
        [
            pytest.param("LastgangKompakt", "lastgang_kompakt", id="LastgangKompakt"),
            pytest.param("ABCTest", "abc_test", id="ABCTest"),
            pytest.param("ABC", "abc", id="ABC"),
            pytest.param("_WithLeadingUnderscore", "_with_leading_underscore", id="_WithLeadingUnderscore"),
            pytest.param("WithLastUppercaseL", "with_last_uppercase_l", id="WithLastUppercaseL"),
        ],
    )
    def test_camel_to_snake(self, camel_case_str: str, snake_case_str: str) -> None:
        assert snake_case_str == camel_to_snake(camel_case_str)
