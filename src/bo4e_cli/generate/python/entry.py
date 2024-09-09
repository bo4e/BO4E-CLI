"""
This module is the entry point for the CLI bo4e-generator.
"""

import shutil
from pathlib import Path
from typing import Optional

from bo4e_cli.generate.python.parser import (
    OutputType,
    bo4e_init_file_content,
    bo4e_version_file_content,
    get_formatter,
    parse_bo4e_schemas,
)
from bo4e_cli.generate.python.schema import get_namespace, get_version
from bo4e_cli.generate.python.sqlparser import remove_unused_imports


def resolve_paths(input_directory: Path, output_directory: Path) -> tuple[Path, Path]:
    """
    Resolve the input and output paths. The data-model-parser have problems with handling relative paths.
    """
    if not input_directory.is_absolute():
        input_directory = input_directory.resolve()
    if not output_directory.is_absolute():
        output_directory = output_directory.resolve()
    return input_directory, output_directory


def generate_bo4e_schemas(
    input_directory: Path,
    output_directory: Path,
    output_type: OutputType,
    clear_output: bool = False,
    target_version: Optional[str] = None,
) -> None:
    """
    Generate all BO4E schemas from the given input directory and save them in the given output directory.
    """
    input_directory, output_directory = resolve_paths(input_directory, output_directory)
    namespace = get_namespace(input_directory)
    file_contents = parse_bo4e_schemas(input_directory, namespace, output_type)
    version = get_version(target_version, namespace)
    file_contents[Path("__version__.py")] = bo4e_version_file_content(version)
    file_contents[Path("__init__.py")] = bo4e_init_file_content(namespace, version)
    if clear_output and output_directory.exists():
        shutil.rmtree(output_directory)

    formatter = get_formatter()
    for relative_file_path, file_content in file_contents.items():
        file_path = output_directory / relative_file_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        if (
            relative_file_path.name not in ["__init__.py", "__version__.py"]
            and OutputType[output_type] == OutputType.SQL_MODEL
        ):
            file_content = remove_unused_imports(file_content)
        file_content = formatter.format_code(file_content)
        file_path.write_text(file_content, encoding="utf-8")
        print(f"Created {file_path}")
    print("Done.")
