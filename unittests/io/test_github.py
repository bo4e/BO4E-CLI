from more_itertools import one

from bo4e_cli.io.github import download_schemas
from bo4e_cli.io.version_file import read_version_file
from unittests.conftest import TEST_DIR


class TestGitHubIO:
    async def test_download_schemas(self, mock_github):
        version = read_version_file(TEST_DIR)
        schemas = await download_schemas(version, None)
        assert len(schemas) > 100
        angebot = one(filter(lambda schema: schema.module == ("bo", "Angebot"), schemas))
        assert angebot.get_schema_parsed().properties["_version"].default == str(version).lstrip("v")
        assert "angebotsnehmer" in angebot.get_schema_parsed().properties
