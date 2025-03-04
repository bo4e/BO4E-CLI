from pathlib import Path

from typer.testing import CliRunner

from bo4e_cli import app
from bo4e_cli.models.meta import SchemaMeta
from unittests.conftest import TEST_DATA_VERSION


class TestPull:
    """
    A class with pytest unit tests.
    """

    def test_latest(self, tmp_path: Path, mock_github: None) -> None:
        result = CliRunner().invoke(app, ["pull", "-o", str(tmp_path), "--no-update-refs"], catch_exceptions=False)
        assert result.exit_code == 0

        version_file = tmp_path / ".version"
        angebot_schema = tmp_path / "bo/Angebot.json"
        assert version_file.exists()
        assert version_file.read_text() == TEST_DATA_VERSION
        assert angebot_schema.exists()
        angebot = SchemaMeta(name="Angebot", module=("bo", "Angebot"), src=angebot_schema)
        angebot.set_schema_text(angebot_schema.read_text())
        assert angebot.schema_parsed.title == "Angebot"

    def test_explicit_version(self, tmp_path: Path, mock_github: None) -> None:
        result = CliRunner().invoke(
            app, ["pull", "-o", str(tmp_path), "--no-update-refs", "-t", str(TEST_DATA_VERSION)], catch_exceptions=False
        )
        assert result.exit_code == 0

        version_file = tmp_path / ".version"
        angebot_schema = tmp_path / "bo/Angebot.json"
        assert version_file.exists()
        assert version_file.read_text() == TEST_DATA_VERSION
        assert angebot_schema.exists()
        angebot = SchemaMeta(name="Angebot", module=("bo", "Angebot"), src=angebot_schema)
        angebot.set_schema_text(angebot_schema.read_text())
        assert angebot.schema_parsed.title == "Angebot"

    def test_update_refs(self, tmp_path: Path, mock_github: None) -> None:
        result = CliRunner().invoke(app, ["pull", "-o", str(tmp_path)], catch_exceptions=False)
        assert result.exit_code == 0

        version_file = tmp_path / ".version"
        angebot_schema = tmp_path / "bo/Angebot.json"
        assert version_file.exists()
        assert version_file.read_text() == TEST_DATA_VERSION
        assert angebot_schema.exists()
        angebot = SchemaMeta(name="Angebot", module=("bo", "Angebot"), src=angebot_schema)
        angebot.set_schema_text(angebot_schema.read_text())
        assert angebot.schema_parsed.title == "Angebot"
        assert angebot.schema_parsed.properties["_typ"].any_of[0].ref == "../enum/Typ.json#"
