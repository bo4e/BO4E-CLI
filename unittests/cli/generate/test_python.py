from pathlib import Path

from typer.testing import CliRunner

from bo4e_cli import app
from unittests.conftest import TEST_DIR_BO4E_REL_REFS


class TestGeneratePython:
    """
    A class with pytest unit tests.
    """

    def test_generate_python_sql_model(self, tmp_path: Path) -> None:
        output_dir = Path(__file__).parents[1] / "output"
        result = CliRunner().invoke(
            app,
            ["generate", "-i", str(TEST_DIR_BO4E_REL_REFS), "-o", str(output_dir), "-t", "python-sql-model"],
            catch_exceptions=False,
        )
        assert result.exit_code == 0

    def test_generate_python_pydantic_v2(self, tmp_path: Path) -> None:
        output_dir = Path(__file__).parents[1] / "output"
        result = CliRunner().invoke(
            app,
            ["generate", "-i", str(TEST_DIR_BO4E_REL_REFS), "-o", str(output_dir), "-t", "python-pydantic-v2"],
            catch_exceptions=False,
        )
        assert result.exit_code == 0
