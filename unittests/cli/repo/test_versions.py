from typer.testing import CliRunner

from bo4e_cli import app
from unittests.conftest import BO4E_PYTHON_DIR, change_cwd


class TestLastVersions:
    """
    A class with pytest unit tests.
    """

    def test_repo_last_versions_quiet_and_extra_flags(self) -> None:

        with change_cwd(BO4E_PYTHON_DIR):
            result = CliRunner().invoke(
                app,
                ["repo", "versions", "-qctn", "3", "-r", "v202501.0.0"],
                catch_exceptions=False,
            )

        version_tags = result.stdout.splitlines()

        assert version_tags == ["v202401.7.1", "v202401.6.0", "v202401.5.0"]

    def test_repo_last_versions_with_table(self) -> None:
        with change_cwd(BO4E_PYTHON_DIR):
            result = CliRunner().invoke(
                app,
                ["repo", "versions", "-sr", "ff94097932d6e2e0aadb515281f53f3619a93486"],
                # This commit sha corresponds to the (wrongly tagged) 2025.0.0 tag.
                # For some reason, dependabot created this tag?
                catch_exceptions=False,
                # terminal_width=20,
                # It seems that the click runner constraints the terminal width + this option does not work.
                # I think this issue is related: https://github.com/pallets/click/issues/1997
            )

        # Check if the output is a table with the expected columns
        assert "Version" in result.stdout
        assert "Commit SHA" in result.stdout
        assert "Commit date" in result.stdout

        # Check if the output contains some expected versions
        assert "v202401.7.1" in result.stdout
        assert "v202401.7.0" in result.stdout
        assert "v202401.6.0" in result.stdout
        assert "v202401.5.0" in result.stdout
        assert "v202401.0.0" in result.stdout
        # assert "v202401.1.0-rc1" in result.stdout
        # Assertion will fail due to truncated output in the table cells.

        # Check if the full commit SHA is shown
        assert "441199993eb7109d6f95181fea" in result.stdout, "Full commit SHA of version v202401.0.0 should be shown"
        # Full commit SHA (441199993eb7109d6f95181fea007b2846629bd3) isn't shown in the table due to
        # truncation. Therefore, we check for a shorter string which is still longer than the truncation from
        # the CLI-flag.

        # Check if the commit date is shown
        # Note, that again, the commit date is truncated in the table output. Normally, there should be also a
        # time shift defined at the end of the commit date.
        assert "2025-04-17 07:47" in result.stdout, "Commit date of version v202401.0.0 should be shown"
