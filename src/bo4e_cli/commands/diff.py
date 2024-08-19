from pathlib import Path
from typing import Annotated, Optional

import typer

sub_app_diff = typer.Typer()


@sub_app_diff.command("schemas")
def diff_schemas(
    *,
    input_dir_base: Annotated[Path, typer.Argument()],
    input_dir_comp: Annotated[Path, typer.Argument()],
    output_file: Annotated[Path, typer.Option(help="The JSON-file to save the differences to.", show_default=False)],
):
    """
    Compare the JSON-schemas in the two input directories and save the differences to the output file.
    The output file will contain the differences in JSON-format. It will also contain information about the
    compared versions.
    """
    pass
