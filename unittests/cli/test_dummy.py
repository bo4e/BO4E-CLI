from pathlib import Path

from typer.testing import CliRunner

from bo4e_cli import app


class TestDummy:
    """
    A class with pytest unit tests.
    """

    def test_dummy(self) -> None:
        result = CliRunner().invoke(app, ["diff", "bump", str(Path(__file__))], catch_exceptions=False)
        assert result.exit_code == 0
        assert "dummy function" in result.stdout
