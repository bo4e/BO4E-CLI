"""
This module contains the generate command.
"""

from pathlib import Path
from typing import Annotated

import typer

from bo4e_cli.commands.entry import app
from bo4e_cli.generate.python.entry import generate_bo4e_schemas
from bo4e_cli.io.console import CONSOLE
from bo4e_cli.io.schemas import read_schemas
from bo4e_cli.types import GenerateType


@app.command()
def generate(
    *,
    input_dir: Annotated[
        Path, typer.Option("--input", "-i", help="The directory to read the JSON-schemas from.", show_default=False)
    ],
    output_dir: Annotated[
        Path, typer.Option("--output", "-o", help="The directory to save the generated code to.", show_default=False)
    ],
    output_type: Annotated[
        GenerateType, typer.Option("--output-type", "-t", help="The type of code to generate.", show_default=False)
    ],
    clear_output: Annotated[
        bool, typer.Option(help="Clear the output directory before saving the generated code.")
    ] = True,
) -> None:
    """
    Generate the BO4E models from the JSON-schemas in the input directory and save them in the
    output directory.

    Several output types are available, see --output-type.
    """
    schemas = read_schemas(input_dir)
    if output_type in (GenerateType.PYTHON_PYDANTIC_V1, GenerateType.PYTHON_PYDANTIC_V2, GenerateType.PYTHON_SQL_MODEL):
        generate_bo4e_schemas(schemas, output_dir, output_type, clear_output)
    else:
        CONSOLE.print(f"Output type {output_type} is not supported yet.")
        raise typer.Exit(1)
