from unittest.mock import Mock

import pytest
import typer

from bo4e_cli.commands.autocompletion import version_autocompletion


class TestAutocompletion:
    """
    A class with pytest unit tests.
    """

    @pytest.mark.parametrize(
        "version_tag,expected_matches",
        [
            pytest.param("", 4, id="no input"),
            pytest.param("v20", 4, id="prefix v20"),
            pytest.param("v202401", 2, id="prefix v202401"),
        ],
    )
    def test_latest(self, mock_github: None, version_tag: str, expected_matches: int) -> None:
        context_mock = Mock(typer.Context)
        context_mock.params = {"version_tag": version_tag}
        matched_versions = list(version_autocompletion(context_mock))
        assert len(matched_versions) == expected_matches
