"""
Command group for interacting with the BO4E-python repository.
"""

from typing import Annotated

import typer
from rich.table import Table

from bo4e_cli.commands.parser import set_quiet_mode
from bo4e_cli.io.console import CONSOLE
from bo4e_cli.io.git import get_commit_date, get_commit_sha, get_last_n_tags, get_ref

sub_app_repo = typer.Typer(
    help="Command group for interacting with the [link=https://github.com/bo4e/BO4E-python]BO4E-python[/] repository. "
    "See 'repo --help' for more information."
)


# pylint: disable=too-many-arguments, too-many-branches
@sub_app_repo.command("versions")
def get_last_versions(
    *,
    n: Annotated[
        int,
        typer.Option(
            "-n",
            help="Number of last versions to retrieve. If the number is set to 0, "
            "all versions will be retrieved up until v202401.0.0.",
        ),
    ] = 0,
    ref: Annotated[
        str,
        typer.Option(
            "--ref",
            "-r",
            help="The git reference object to start from. The reference can be a tag, branch or commit. "
            "From this point the last n versions will be retrieved. If the reference is a tag, the tag itself won't "
            "be included in the output. If the reference is neither a tag, branch nor a commit, all versions prior to "
            'the current checkout commit (i.e. "HEAD") will be retrieved. ',
        ),
    ] = "main",
    exclude_candidates: Annotated[
        bool,
        typer.Option(
            "--exclude-candidates",
            "-c",
            help="Exclude release candidates from the output. "
            "If set to False, release candidates will be included in the output. "
            "Excluded elements don't count towards the number of versions to retrieve.",
        ),
    ] = False,
    exclude_technical_bumps: Annotated[
        bool,
        typer.Option(
            "--exclude-technical-bumps",
            "-t",
            help="Exclude technical version bumps from the output. "
            "If set to False, technical bumps will be included in the output. "
            "Excluded elements don't count towards the number of versions to retrieve. "
            "From versions differing only in the technical version, the newest technical release will be returned.",
        ),
    ] = False,
    show_full_commit_sha: Annotated[
        bool,
        typer.Option(
            "--show-full-commit-sha",
            "-s",
            help="If set, the full commit SHA will be shown in the output. "
            "Otherwise, only the first 6 characters of the commit SHA will be shown. "
            "This option has no effect if the output is quiet (i.e. --quiet is set). ",
        ),
    ] = False,
    quiet: Annotated[
        bool,
        typer.Option(
            "--quiet",
            "-q",
            help="If set, the output will contain only the versions, separated by newlines. "
            "Additionally, if an error occurs, it will be printed to stdout and the program exits with exit code 1. "
            "Can't be set together with verbose option.",
        ),
    ] = False,
) -> None:
    """
    Get the last n versions of the BO4E-python repository starting from the given reference.

    This command must be executed from the root of the BO4E-python repository. Technically, it should also work
    on other repositories following the same versioning scheme, but it is primarily intended for BO4E-python.
    Note that the command will not explicitly check if the current directory is the root of the BO4E-python repository.

    The output will contain the version tags in chronological descending order, i.e. the newest version first.
    If executed without any arguments, it will return all versions on the main branch since v202401.0.0.
    """
    set_quiet_mode(quiet)

    try:
        ref_type, ref = get_ref(ref)
        last_versions = list(
            get_last_n_tags(
                n, ref=ref, exclude_candidates=exclude_candidates, exclude_technical_bumps=exclude_technical_bumps
            )
        )
        if quiet:
            print("\n".join(map(str, last_versions)))
            return
        # Make a rich table containing some information about the versions
        if n == 0:
            title = "All versions between v202401.0.0 and "
        else:
            title = f"Last {n} versions before "
        if ref_type == "tag":
            title += f"{ref}"
        elif ref_type == "branch":
            title += f"latest commit on branch {ref}"
        elif ref_type == "commit":
            title += f"commit {ref}"
        table = Table(
            title=title,
            highlight=False,
            row_styles=["bo4e.table.row1", "bo4e.table.row2"],
            title_style="bo4e.table.title",
            header_style="bo4e.table.header",
        )

        table.add_column("Version", justify="left", no_wrap=True)
        table.add_column("Commit SHA", justify="left", no_wrap=True)
        table.add_column("Commit date", justify="right", no_wrap=True)

        for version in map(str, last_versions):
            commit_sha = get_commit_sha(version)
            table.add_row(version, commit_sha if show_full_commit_sha else commit_sha[:6], get_commit_date(commit_sha))

        CONSOLE.print("\n", table)

    except Exception as error:  # pylint: disable=broad-exception-caught
        if quiet:
            print(f"Error while retrieving versions: {error}")
            raise typer.Exit(code=1)
        CONSOLE.print(f"Error while retrieving versions: {error}", style="warning")
        CONSOLE.print_exception()
