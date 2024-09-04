from more_itertools import one

from bo4e_cli.io.github import download_schemas, get_versions, resolve_latest_version
from bo4e_cli.io.version_file import read_version_file
from unittests.conftest import TEST_DATA_VERSION, TEST_DIR_BO4E_ORIGINAL


class TestGitHubIO:
    async def test_download_schemas(self, mock_github: None) -> None:
        version = read_version_file(TEST_DIR_BO4E_ORIGINAL)
        schemas = await download_schemas(version, None)
        assert len(schemas) > 100
        angebot = one(filter(lambda schema: schema.module == ("bo", "Angebot"), schemas))
        assert angebot.get_schema_parsed().properties["_version"].default == str(version).lstrip("v")
        assert "angebotsnehmer" in angebot.get_schema_parsed().properties

    def test_resolve_latest_version(self, mock_github: None) -> None:
        version = read_version_file(TEST_DIR_BO4E_ORIGINAL)
        latest_version = resolve_latest_version(None)
        assert version == latest_version

    def test_get_versions(self, mock_github: None) -> None:
        versions = list(get_versions(None))
        assert len(versions) > 1
        assert TEST_DATA_VERSION in versions
