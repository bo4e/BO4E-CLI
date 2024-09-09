"""
This module contains functionality to retrieve information about the schemas.
"""

import json
import re
from pathlib import Path
from typing import Any, Optional

from pydantic import BaseModel

SchemaType = dict[str, Any]


class SchemaMetadata(BaseModel):
    """
    Metadata about a schema.
    """

    schema_text: str
    schema_parsed: SchemaType
    class_name: str
    input_file: Path
    output_file: Path
    "The output file will be a relative path"
    module_path: tuple[str, ...]
    "e.g. ('bo', 'preisblatt_netznutzung') or ('zusatz_attribut')"

    def save(self, content: str):
        """
        Save the content to the file defined by `output_file`. Creates parent directories if needed.
        """
        self.output_file.parent.mkdir(parents=True, exist_ok=True)
        self.output_file.write_text(content)

    @property
    def module_name(self) -> str:
        """e.g. 'preisblatt_netznutzung' or 'zusatz_attribut'"""
        return self.module_path[-1]

    @property
    def module_path_with_extension(self) -> tuple[str, ...]:
        """e.g. ('bo', 'preisblatt_netznutzung.py') or ('zusatz_attribut.py')"""
        return *self.module_path[:-1], f"{self.module_path[-1]}.py"

    def __str__(self):
        return ".".join(self.module_path)


def camel_to_snake(name: str) -> str:
    """
    Convert a camel case string to snake case. Credit to https://stackoverflow.com/a/1176023/21303427
    """
    name = re.sub("([^_])([A-Z][a-z]+)", r"\1_\2", name)
    return re.sub("([a-z0-9])([A-Z])", r"\1_\2", name).lower()


def get_namespace(input_directory: Path) -> dict[str, SchemaMetadata]:
    """
    Create a namespace for the bo4e classes.
    """

    namespace: dict[str, SchemaMetadata] = {}
    for file_path in input_directory.rglob("*.json"):
        relative_path = file_path.relative_to(input_directory)
        module_path = tuple(camel_to_snake(part) for part in relative_path.with_suffix("").parts)
        schema_text = file_path.read_text(encoding="utf-8")
        schema_parsed = json.loads(schema_text)
        class_name = schema_parsed["title"].replace(" ", "_")

        namespace[class_name] = SchemaMetadata(
            module_path=module_path,
            input_file=file_path,
            output_file=Path(*module_path).with_suffix(".py"),
            schema_text=schema_text,
            schema_parsed=schema_parsed,
            class_name=class_name,
        )
    return namespace


def get_version(target_version: Optional[str], namespace: dict[str, SchemaMetadata]) -> str:
    """
    Get the version of the bo4e schemas.
    """
    if target_version is not None:
        gh_version_regex = re.compile(r"^v(?P<version>(?:\d+\.)*\d+)(?:-(?P<release_candidate>rc\d+))?$")
        if gh_version_regex.match(target_version) is not None:
            target_version = gh_version_regex.sub(r"\g<version>\g<release_candidate>", target_version)
        return target_version
    # The chosen class is arbitrary. All bo's and com's should contain the same version information.
    try:
        return namespace["Angebot"].schema_parsed["properties"]["_version"]["default"]
    except KeyError:
        return "unknown"
