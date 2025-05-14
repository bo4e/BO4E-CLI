from pathlib import Path

from typer.testing import CliRunner

from bo4e_cli import app
from bo4e_cli.io.schemas import read_schemas, write_schemas
from bo4e_cli.models.changes import Changes, ChangeType
from bo4e_cli.models.schema import Decimal, String
from unittests.conftest import TEST_DIR_BO4E_REL_REFS


class TestDiffSchemas:
    """
    A class with pytest unit tests.
    """

    def test_edit(self, tmp_path: Path) -> None:
        modified_bo4e_path = tmp_path / "modified_bo4e"
        schemas = read_schemas(TEST_DIR_BO4E_REL_REFS)
        schema_angebot = schemas.modules["bo", "Angebot"]
        schemas.remove(schema_angebot)
        schema_ausschreibung = schemas.modules["bo", "Ausschreibung"]
        del schema_ausschreibung.object_schema_parsed.properties["abgabefrist"]
        old_type: String = schema_ausschreibung.object_schema_parsed.properties["ausschreibungsnummer"].any_of[0]
        schema_ausschreibung.object_schema_parsed.properties["ausschreibungsnummer"].any_of[0] = Decimal(
            type="number",
            format="decimal",
            description=old_type.description,
            title=old_type.title,
            default=old_type.default,
        )
        schemas.version.functional += 1
        write_schemas(schemas, modified_bo4e_path)
        # **********
        diff_file = tmp_path / "diff_file.json"
        result = CliRunner().invoke(
            app,
            [
                "diff",
                "schemas",
                str(TEST_DIR_BO4E_REL_REFS),
                str(modified_bo4e_path),
                "-o",
                str(diff_file),
            ],
        )
        assert result.exit_code == 0

        with open(diff_file, "r", encoding="utf-8") as file:
            changes = Changes.model_validate_json(file.read())

        assert changes.old_version < changes.new_version
        assert len(changes.changes) == 3
        assert set(change.type for change in changes.changes) == {
            ChangeType.CLASS_REMOVED,
            ChangeType.FIELD_TYPE_CHANGED,
            ChangeType.FIELD_REMOVED,
        }
