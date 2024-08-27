from pathlib import Path
from typing import Iterable
from unittest.mock import Mock

from bo4e_cli.io.version_file import read_version_file

TEST_DIR = Path(__file__).parents[1] / "test_data/bo4e_original"


def _get_tree_mock(target_commitish, recursive):
    TreeMock = Mock()
    TreeMock.tree = [
        Mock(path="meta/some/dummy.json"),
        Mock(path="src/bo4e_schemas/bo/Angebot.json"),
        Mock(path="src/bo4e_schemas/bo/Zaehler.json"),
        Mock(path="src/bo4e_schemas/enum/Sparte.json"),
        Mock(path="src/bo4e_schemas"),
        Mock(path="src/bo4e_schemas/bo"),
        Mock(path="src/bo4e_schemas/com"),
        Mock(path="src/bo4e_schemas/enum"),
    ]


def mock_pygithub():
    GitHubMock = Mock()

    version = read_version_file(TEST_DIR)
    version_list = [version, "v200000.0.0", "v202401.0.1-rc3", "v202407.3.1+dev2hb3826gj"]
    GitHubMock.return_value.get_repo.return_value.get_latest_release.return_value.title = version
    GitHubMock.return_value.get_repo.return_value.get_releases.return_value = {
        Mock(title=version_el) for version_el in version_list
    }
    GitHubMock.return_value.get_repo.return_value.get_release = lambda _version: Mock(
        target_commitish=f"hash of {_version}"
    )
    GitHubMock.return_value.get_repo.return_value.get_git_tree = _get_tree_mock
