"""
Contains functions to post process Python code using black, isort and autoflake.
"""

import os
import subprocess
import tempfile
from pathlib import Path

from datamodel_code_generator import PythonVersion
from datamodel_code_generator.format import CodeFormatter
from rich.progress import track

from bo4e_cli.io.console import CONSOLE


def remove_unused_imports(code: str) -> str:
    """
    Removes unused imports from the given code using autoflake.
    """
    # Create a temporary file
    with tempfile.NamedTemporaryFile(suffix=".py", delete=False) as tmp_file:
        tmp_file_name = tmp_file.name
        tmp_file.write(code.encode("utf-8"))

    # Run autoflake to remove unused imports
    subprocess.run(["autoflake", "--remove-all-unused-imports", "--in-place", tmp_file_name], check=True)

    # Read the cleaned code from the temporary file
    with open(tmp_file_name, "r", encoding="utf-8") as tmp_file:
        cleaned_code = tmp_file.read()

    # Clean up the temporary file
    os.remove(tmp_file_name)

    return cleaned_code


def get_formatter() -> CodeFormatter:
    """
    Returns a formatter to apply black and isort
    """
    return CodeFormatter(
        PythonVersion.PY_311,
        None,
        None,
        skip_string_normalization=False,
        known_third_party=None,
        custom_formatters=None,
        custom_formatters_kwargs=None,
    )


def format_code(code: str) -> str:
    """
    Formats the given code using black and isort.
    """
    formatter = get_formatter()
    return formatter.format_code(code)


def post_process_files(files: dict[Path, str]) -> None:
    """
    Post process the given files using black, isort and autoflake.
    """
    formatter = get_formatter()
    for file_path, file_content in track(
        files.items(), description="Postprocessing generated files...", total=len(files), console=CONSOLE
    ):
        files[file_path] = file_content = remove_unused_imports(file_content)
        files[file_path] = formatter.format_code(file_content)
