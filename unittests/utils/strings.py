import pytest

from bo4e_cli.utils.strings import (
    camel_to_snake,
    construct_id_field_name,
    escaped,
    pydantic_field_name,
    snake_to_pascal,
)


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

    @pytest.mark.parametrize(
        "snake_case_str, pascal_case_str",
        [
            pytest.param("lastgang_kompakt", "LastgangKompakt", id="lastgang_kompakt"),
            pytest.param("abc_test", "AbcTest", id="abc_test"),
            pytest.param("abc", "Abc", id="abc"),
            pytest.param("_with_leading_underscore", "WithLeadingUnderscore", id="_with_leading_underscore"),
            pytest.param("with_last_uppercase_l", "WithLastUppercaseL", id="with_last_uppercase_l"),
        ],
    )
    def test_snake_to_pascal(self, snake_case_str: str, pascal_case_str: str) -> None:
        assert pascal_case_str == snake_to_pascal(snake_case_str)

    @pytest.mark.parametrize(
        "field_name, expected",
        [
            pytest.param("lastgangKompakt", "lastgang_kompakt", id="lastgangKompakt"),
            pytest.param("abcTest", "abc_test", id="abcTest"),
            pytest.param("abc", "abc", id="abc"),
            pytest.param("_withLeadingUnderscore", "with_leading_underscore", id="_withLeadingUnderscore"),
            pytest.param("withLastUppercaseL", "with_last_uppercase_l", id="withLastUppercaseL"),
        ],
    )
    def test_pydantic_field_name(self, field_name: str, expected: str) -> None:
        assert (expected, field_name) == pydantic_field_name(field_name)

    def test_construct_id_field_name(self) -> None:
        assert "angebotsgeber_id" == construct_id_field_name("angebotsgeber")

    @pytest.mark.parametrize(
        "string, expected",
        [
            pytest.param("Hello, World!", '"Hello, World!"', id="Hello, World!"),
            pytest.param("Hello, 'World'!", "\"Hello, 'World'!\"", id="Hello, 'World'!"),
            pytest.param("Hello, \n'World'!", "\"Hello, \\n'World'!\"", id="Hello, \n'World'!"),
            pytest.param('Hello, "World"!', '"Hello, \\"World\\"!"', id='Hello, "World"!'),
            pytest.param("Hello, \\n'World'!", "\"Hello, \\\\n'World'!\"", id="Hello, \\n'World'!"),
        ],
    )
    def test_escape_string_for_python_output(self, string: str, expected: str) -> None:
        assert expected == escaped(string)
