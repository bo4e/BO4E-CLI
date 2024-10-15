"""
This module contains custom types for the CLI.
"""

from enum import StrEnum


class GenerateType(StrEnum):
    """
    A custom type for the generate command.
    """

    PYTHON_PYDANTIC_V1 = "python-pydantic-v1"
    PYTHON_PYDANTIC_V2 = "python-pydantic-v2"
    PYTHON_SQL_MODEL = "python-sql-model"
