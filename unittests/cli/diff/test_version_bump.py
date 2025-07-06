from pathlib import Path

from typer.testing import CliRunner

from bo4e_cli import app
from bo4e_cli.models.matrix import CompatibilityMatrix, CompatibilitySymbol
from unittests.conftest import TEST_DIR


class TestDiffVersionBump:
    """
    A class with pytest unit tests.
    """

    def test_diff_version_bump(self, tmp_path: Path) -> None:
        diff_files: list[Path] = list(sorted((TEST_DIR / "diffs").glob("*.json")))

        for diff_file, expected_result in zip(diff_files, [True, True, False]):
            result_quiet = CliRunner().invoke(app, ["diff", "version-bump", str(diff_file), "--quiet"])
            result_noisy = CliRunner().invoke(app, ["--verbose", "diff", "version-bump", str(diff_file)])
            if expected_result:
                assert result_quiet.exit_code == 0, f"{result_quiet.output}"
                assert "version bump is valid" in result_noisy.output, f"Unexpected output: {result_noisy.output}"
            else:
                assert result_quiet.exit_code != 0, f"{result_quiet.output}"
                assert (
                    "Functional release bump is needed" in result_noisy.output
                ), f"Unexpected output: {result_noisy.output}"
                assert "Invalid version bump" in result_noisy.output, f"Unexpected output: {result_noisy.output}"
