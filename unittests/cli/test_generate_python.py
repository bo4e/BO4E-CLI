from pathlib import Path

from typer.testing import CliRunner

from bo4e_cli import app
from bo4e_cli.models.meta import SchemaMeta
from bo4e_cli.models.schema import String
from unittests.conftest import TEST_DATA_VERSION, TEST_DIR, TEST_DIR_BO4E_REL_REFS


class TestGeneratePython:
    """
    A class with pytest unit tests.
    """

    def test_generate_python_sql_model(self, tmp_path: Path) -> None:
        OUTPUT_DIR = Path(__file__).parents[1] / "output"
        result = CliRunner().invoke(
            app,
            ["generate", "-i", str(TEST_DIR_BO4E_REL_REFS), "-o", str(OUTPUT_DIR), "-t", "python-sql-model"],
            catch_exceptions=False,
        )
        assert result.exit_code == 0
