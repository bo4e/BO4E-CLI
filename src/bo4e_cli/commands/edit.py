"""
This module contains the command for editing JSON-schemas.
"""

from pathlib import Path
from typing import Annotated, Optional

import typer

from bo4e_cli.commands.entry import app
from bo4e_cli.edit.add import transform_all_additional_enum_items, transform_all_additional_fields
from bo4e_cli.edit.non_nullable import transform_all_non_nullable_fields
from bo4e_cli.io.cleanse import clear_dir_if_needed
from bo4e_cli.io.config import get_additional_schemas, load_config
from bo4e_cli.io.console import CONSOLE
from bo4e_cli.io.console.console import add_schemas_to_highlighter
from bo4e_cli.io.schemas import read_schemas, write_schemas
from bo4e_cli.models.schema import SchemaRootObject


@app.command()
def edit(
    *,
    input_dir: Annotated[
        Path,
        typer.Option(
            "--input", "-i", help="The directory to read the JSON-schemas from.", show_default=False, resolve_path=True
        ),
    ],
    output_dir: Annotated[
        Path,
        typer.Option(
            "--output",
            "-o",
            help="The directory to save the edited JSON-schemas to.",
            show_default=False,
            resolve_path=True,
        ),
    ],
    config_file: Annotated[
        Optional[Path],
        typer.Option(
            "--config", "-c", help="The configuration file to use for editing the JSON-schemas.", resolve_path=True
        ),
    ] = None,
    set_default_version: Annotated[
        bool,
        typer.Option(
            help="Automatically set or overrides the default version for '_version' fields with the version from "
            ".version file. This is especially useful if you want to define additional models which should "
            "always have the correct version."
        ),
    ] = True,
    clear_output: Annotated[bool, typer.Option(help="Clear the output directory before saving the schemas.")] = True,
) -> None:
    """
    Edit the JSON-schemas in the input directory and save the edited schemas to the output directory.

    The schemas in the input directory won't be changed. If no configuration file is provided, the schemas will be
    copied to the output directory unchanged.
    """
    if config_file is not None:
        config = load_config(config_file)
    else:
        config = None

    schemas = read_schemas(input_dir)

    if config is not None:
        schemas.update(get_additional_schemas(config, config_file))
        add_schemas_to_highlighter(schemas, match_fields=True)
        CONSOLE.print("Added all additional models")
        transform_all_additional_fields(config.additional_fields, schemas)  # type: ignore[arg-type]
        # the load_config function ensures that the references are resolved.
        CONSOLE.print("Added all additional fields")
        transform_all_non_nullable_fields(config.non_nullable_fields, schemas)
        CONSOLE.print("Transformed all non nullable fields")
        transform_all_additional_enum_items(config.additional_enum_items, schemas)
        CONSOLE.print("Added all additional enum items")
    else:
        add_schemas_to_highlighter(schemas)

    if set_default_version:
        for schema in schemas:
            if isinstance(schema.schema_parsed, SchemaRootObject) and "_version" in schema.schema_parsed.properties:
                schema.schema_parsed.properties["_version"].default = schemas.version.to_str_without_prefix()
        CONSOLE.print(f"Set default versions to {schemas.version}")

    if clear_output:
        clear_dir_if_needed(output_dir)

    write_schemas(schemas, output_dir)
