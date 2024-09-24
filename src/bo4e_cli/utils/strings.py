"""
Contains utility functions for string manipulation.
"""

import re


def camel_to_snake(name: str) -> str:
    """
    Convert a camel case string to snake case. Credit to https://stackoverflow.com/a/1176023/21303427
    """
    name = re.sub("([^_])([A-Z][a-z]+)", r"\1_\2", name)
    return re.sub("([a-z0-9])([A-Z])", r"\1_\2", name).lower()


def snake_to_pascal(name: str) -> str:
    """
    Convert a snake case string to pascal case.
    """
    return "".join(word.capitalize() for word in name.split("_"))
