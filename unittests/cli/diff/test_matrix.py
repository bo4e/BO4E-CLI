from pathlib import Path

from typer.testing import CliRunner

from bo4e_cli import app
from bo4e_cli.io.schemas import read_schemas, write_schemas
from bo4e_cli.models.changes import Changes, ChangeSymbol, ChangeType
from bo4e_cli.models.matrix import CompatibilityMatrix
from bo4e_cli.models.schema import Decimal, String
from unittests.conftest import TEST_DIR, TEST_DIR_BO4E_REL_REFS


class TestDiffMatrix:
    """
    A class with pytest unit tests.
    """

    def test_diff_matrix(self, tmp_path: Path) -> None:
        diff_files: list[Path] = list(sorted((TEST_DIR / "diffs").glob("*.json")))
        # **********
        matrix_file = tmp_path / "matrix_single.json"
        CliRunner().invoke(
            app,
            [
                "diff",
                "matrix",
                *map(str, diff_files[:1]),
                "-o",
                str(matrix_file),
            ],
            catch_exceptions=False,
        )

        with open(matrix_file, "r", encoding="utf-8") as file:
            matrix_single = CompatibilityMatrix.model_validate_json(file.read())

        for entries in matrix_single.values():
            assert len(entries) == 1

        # **********
        matrix_file = tmp_path / "matrix_all.json"
        CliRunner().invoke(
            app,
            [
                "diff",
                "matrix",
                *map(str, diff_files[1:] + diff_files[:1]),
                # Change the order to ensure that the command correctly determines the correct order
                "-o",
                str(matrix_file),
                "-et",
                "csv",
            ],
            catch_exceptions=False,
        )

        with open(matrix_file, "r", encoding="utf-8") as file:
            matrix_csv_content = file.read()

        for change_symbol in ChangeSymbol:
            assert change_symbol.value in matrix_csv_content

        assert "bo.AdditionalModel,\-,➕,🟢" in matrix_csv_content
        assert "bo.Angebot,➖,\-,\-" in matrix_csv_content
        assert "bo.Ausschreibung,🔴,🟢,🟢" in matrix_csv_content
        assert "bo.Buendelvertrag,🟢,🟢,🟡" in matrix_csv_content
