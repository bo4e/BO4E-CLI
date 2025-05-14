from pathlib import Path

from typer.testing import CliRunner

from bo4e_cli import app
from bo4e_cli.models.meta import SchemaMeta
from bo4e_cli.models.schema import String
from unittests.conftest import TEST_DATA_VERSION, TEST_DIR, TEST_DIR_BO4E_REL_REFS


class TestEdit:
    """
    A class with pytest unit tests.
    """

    def test_edit(self, tmp_path: Path) -> None:
        result = CliRunner().invoke(
            app,
            ["edit", "-i", str(TEST_DIR_BO4E_REL_REFS), "-o", str(tmp_path), "-c", str(TEST_DIR / "config_test.json")],
        )
        assert result.exit_code == 0

        version_file = tmp_path / ".version"
        angebot_schema_file = tmp_path / "bo/Angebot.json"
        addidional_schema_file = tmp_path / "bo/AdditionalModel.json"
        typ_schema_file = tmp_path / "enum/Typ.json"

        assert version_file.exists()
        assert version_file.read_text() == TEST_DATA_VERSION
        assert angebot_schema_file.exists()
        assert addidional_schema_file.exists()

        angebot = SchemaMeta(name="Angebot", module=("bo", "Angebot"), _src=angebot_schema_file)
        angebot.set_schema_text(angebot_schema_file.read_text())
        angebot_schema = angebot.schema_parsed
        assert angebot_schema.title == "Angebot"
        assert "foo" in angebot_schema.properties

        additional_model = SchemaMeta(
            name="AdditionalModel", module=("bo", "AdditionalModel"), _src=addidional_schema_file
        )
        additional_model.set_schema_text(addidional_schema_file.read_text())
        additional_schema = additional_model.schema_parsed
        assert additional_schema.title == "AdditionalModel"
        assert additional_schema.properties["_version"].default == TEST_DATA_VERSION.to_str_without_prefix()
        assert isinstance(additional_schema.properties["_version"], String)

        typ_model = SchemaMeta(name="Typ", module=("enum", "Typ"), _src=typ_schema_file)
        typ_model.set_schema_text(typ_schema_file.read_text())
        typ_schema = typ_model.schema_parsed
        assert typ_schema.title == "Typ"
        assert "foo" in typ_schema.enum
        assert "bar" in typ_schema.enum
