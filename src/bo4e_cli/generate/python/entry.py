"""
This module is the entry point for the CLI bo4e-generator.
"""

from pathlib import Path
from typing import Literal

from bo4e_cli.generate.python.format import post_process_files
from bo4e_cli.generate.python.parser import bo4e_init_file_content, bo4e_version_file_content, parse_bo4e_schemas
from bo4e_cli.io.cleanse import clear_dir_if_needed
from bo4e_cli.io.file import write_file_contents
from bo4e_cli.models.meta import Schemas
from bo4e_cli.types import GenerateType


def generate_bo4e_schemas(
    schemas: Schemas,
    output_directory: Path,
    generate_type: Literal[
        GenerateType.PYTHON_PYDANTIC_V1,
        GenerateType.PYTHON_PYDANTIC_V2,
        GenerateType.PYTHON_SQL_MODEL,
    ],
    clear_output: bool = True,
) -> None:
    """
    Generate all BO4E schemas from the given input directory and save them in the given output directory.
    """
    file_contents = parse_bo4e_schemas(schemas, generate_type)
    file_contents[Path("__version__.py")] = bo4e_version_file_content(schemas.version.to_str_without_prefix())
    file_contents[Path("__init__.py")] = bo4e_init_file_content(schemas)

    post_process_files(file_contents)
    if clear_output:
        clear_dir_if_needed(output_directory)
    write_file_contents(file_contents, base_path=output_directory)
