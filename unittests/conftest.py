import os
import shutil
import sys
from contextlib import contextmanager
from itertools import pairwise
from pathlib import Path, PurePosixPath
from typing import Iterable, Iterator, cast
from unittest.mock import MagicMock, Mock, patch

import pytest
import respx
from github.ContentFile import ContentFile
from github.GitRelease import GitRelease
from github.GitTree import GitTree
from github.PaginatedList import PaginatedList
from httpx import Request, Response
from more_itertools import take
from typer.testing import CliRunner

from bo4e_cli import app
from bo4e_cli.edit.update_refs import REF_ONLINE_REGEX
from bo4e_cli.io.github import OWNER, REPO, get_source_repo
from bo4e_cli.io.schemas import read_schemas, write_schemas
from bo4e_cli.io.version_file import read_version_file
from bo4e_cli.models.meta import SchemaMeta
from bo4e_cli.models.schema import Decimal, String
from bo4e_cli.models.version import Version

REPO_DIR = Path(__file__).parents[1]
BO4E_PYTHON_DIR = REPO_DIR / "BO4E-python"
TEST_DIR = Path(__file__).parent / "test_data"
TEST_DIR_BO4E_ORIGINAL = TEST_DIR / "bo4e_original"
TEST_DIR_BO4E_REL_REFS = TEST_DIR / "bo4e_rel_refs"
TEST_DATA_VERSION = Version.from_str("v202401.4.0")


class RepoMock:
    def __init__(self, local_directory: Path, mount_path: PurePosixPath, version: str):
        mount_path = self._remove_first_slash_if_needed(mount_path)
        self._local_directory = local_directory
        self._mount_path = mount_path
        self._version = version
        self._dirs = {
            PurePosixPath(*take(take_num, mount_path.parts)) for take_num in range(1, 1 + len(mount_path.parts))
        }
        self._files = set()
        for cur_dir, _, files in os.walk(local_directory):
            dir_path = mount_path / Path(cur_dir).relative_to(local_directory)
            self._dirs.add(dir_path)
            for file in files:
                file_path = dir_path / file
                self._files.add(file_path)

    def _remove_first_slash_if_needed(self, path: PurePosixPath) -> PurePosixPath:
        return PurePosixPath(str(path).lstrip("/"))

    def get_latest_release(self) -> GitRelease:
        return cast(GitRelease, Mock(spec=GitRelease, title=self._version))

    def get_releases(self) -> PaginatedList[GitRelease]:
        mock_list = MagicMock(spec=PaginatedList)
        mock_list.__iter__.return_value = iter(
            [
                Mock(spec=GitRelease, title=version)
                for version in [
                    self._version,
                    "v0.6.1-rc13",
                    "v200000.0.0",
                    "v202401.0.1-rc3",
                    "v202407.3.1+g2hb3826gj",
                ]
            ]
        )
        return cast(PaginatedList[GitRelease], mock_list)

    def get_release(self, version: str) -> GitRelease:
        return cast(GitRelease, Mock(spec=GitRelease, target_commitish=version))

    def get_git_tree(self, target_commitish: str, recursive: bool) -> GitTree:
        return cast(GitTree, Mock(spec=GitTree, tree=[Mock(path=str(path)) for path in self._dirs | self._files]))

    def get_contents(self, path: str, ref: str) -> list[ContentFile]:
        path_ = self._remove_first_slash_if_needed(PurePosixPath(path))
        contents = []
        for el in self._dirs:
            if el.parent == path_:
                contents.append(Mock(spec=ContentFile, path=str(el)))
                contents[-1].name = el.name
        for el in self._files:
            if el.parent == path_:
                contents.append(
                    Mock(
                        spec=ContentFile,
                        path=str(el),
                        download_url=f"https://raw.githubusercontent.com/{OWNER}/{REPO}/{ref}/{el}",
                    )
                )
                contents[-1].name = el.name
        return cast(list[ContentFile], contents)


def download_sideeffect(request: Request, version: str, sub_path: str, model: str) -> Response:
    path = TEST_DIR_BO4E_ORIGINAL / sub_path / f"{model}.json"
    return Response(200, content=path.read_text())


@pytest.fixture(scope="function")
def mock_github(respx_mock: respx.MockRouter) -> Iterable[None]:
    version = read_version_file(TEST_DIR_BO4E_ORIGINAL)

    github = Mock()
    github.return_value.get_repo.return_value = RepoMock(
        TEST_DIR_BO4E_ORIGINAL, PurePosixPath("src/bo4e_schemas"), str(version)
    )
    with patch("bo4e_cli.io.github.Github", new=github):
        route = respx_mock.get(url__regex=REF_ONLINE_REGEX)
        route.side_effect = download_sideeffect
        get_source_repo.cache_clear()
        yield


@contextmanager
def patch_python_path(path: Path) -> Iterator[str]:
    """A context manager to temporarily add a path to the python path.

    Args:
        path: The path to add to the python path.
    Yields:
        The path as a string.
    """
    path_str = str(path)
    sys.path.append(path_str)
    yield path_str
    sys.path.remove(path_str)


# pylint: disable=too-many-locals
def create_diff_files() -> None:
    """
    This is a helper function if you need to recreate the diff files for testing.
    """
    diff_file_out_dir = TEST_DIR / "diffs"

    tmp_path = REPO_DIR / "tmp"
    bo4e_v202401_5_0_path = tmp_path / "bo4e_v202401.5.0"
    bo4e_v202401_6_0_path = tmp_path / "bo4e_v202401.6.0"
    bo4e_v202401_6_1_path = tmp_path / "bo4e_v202401.6.1"

    schemas = read_schemas(TEST_DIR_BO4E_REL_REFS)
    # *********** Modify v202401.4.0 -> v202401.5.0
    schema_angebot = schemas.modules["bo", "Angebot"]
    schemas.remove(schema_angebot)
    schema_ausschreibung = schemas.modules["bo", "Ausschreibung"]
    del schema_ausschreibung.object_schema_parsed.properties["abgabefrist"]
    old_type: String = schema_ausschreibung.object_schema_parsed.properties["ausschreibungsnummer"].any_of[
        0
    ]  # type: ignore[assignment]
    schema_ausschreibung.object_schema_parsed.properties["ausschreibungsnummer"].any_of[0] = Decimal(
        type="number",
        format="decimal",
        description=old_type.description,
        title=old_type.title,
        default=old_type.default,
    )
    schemas.version.functional += 1
    write_schemas(schemas, bo4e_v202401_5_0_path)
    # ********** Modify v202401.5.0 -> v202401.6.0
    additional_model = SchemaMeta(name="AdditionalModel", module=("bo", "AdditionalModel"))
    with open(TEST_DIR / "additional_model.json", encoding="utf-8") as file:
        additional_model.set_schema_text(file.read())
    schemas.add(additional_model)
    schemas.version.functional += 1
    write_schemas(schemas, bo4e_v202401_6_0_path)
    # ********** Modify v202401.6.0 -> v202401.6.1
    schema_buendelvertrag = schemas.modules["bo", "Buendelvertrag"]
    schema_buendelvertrag.object_schema_parsed.properties["beschreibung"].description = "AAAAND IT'S GOOONE!"
    schemas.version.technical += 1
    write_schemas(schemas, bo4e_v202401_6_1_path)
    # ********** Create diff files
    diff_files = []
    for previous_schemas_path, next_schemas_path in pairwise(
        [
            TEST_DIR_BO4E_REL_REFS,
            bo4e_v202401_5_0_path,
            bo4e_v202401_6_0_path,
            bo4e_v202401_6_1_path,
        ]
    ):
        diff_file = (
            diff_file_out_dir
            / f"test_diff_{read_version_file(previous_schemas_path)}_{read_version_file(next_schemas_path)}.json"
        )
        CliRunner().invoke(
            app,
            [
                "diff",
                "schemas",
                str(previous_schemas_path),
                str(next_schemas_path),
                "-o",
                str(diff_file),
            ],
            catch_exceptions=False,
        )
        diff_files.append(diff_file)

    shutil.rmtree(tmp_path)


@contextmanager
def change_cwd(new_cwd: Path) -> Iterator[Path]:
    """
    A context manager to temporarily change the current working directory.

    Args:
        new_cwd: The new current working directory.

    Yields:
        The original current working directory.
    """
    original_cwd = Path.cwd()
    os.chdir(new_cwd)
    try:
        yield original_cwd
    finally:
        os.chdir(original_cwd)
