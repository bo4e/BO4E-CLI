"""
Contains functions to load the config file for `bo4e edit`.
"""

from pathlib import Path
from typing import Iterable, Union

from pydantic import TypeAdapter

from bo4e_cli.io.console import CONSOLE
from bo4e_cli.io.schemas import read_parsed_schema
from bo4e_cli.models.config import AdditionalField, Config
from bo4e_cli.models.meta import SchemaMeta
from bo4e_cli.models.schema import Reference


def load_config(path: Path) -> Config:
    """
    Load the config file
    """
    CONSOLE.print(f"Loading config from {path}")
    config = Config.model_validate_json(path.read_text())

    deletion_list = []
    for additional_field in config.additional_fields:
        if isinstance(additional_field, Reference):
            reference_path = Path(additional_field.ref)
            if not reference_path.is_absolute():
                reference_path = path.parent / reference_path

            additional_fields: Union[AdditionalField, list[AdditionalField]] = TypeAdapter(
                Union[AdditionalField, list[AdditionalField]]
            ).validate_json(reference_path.read_text(encoding="utf-8"))
            deletion_list.append(additional_field)
            if isinstance(additional_fields, list):
                config.additional_fields.extend(additional_fields)
            else:
                config.additional_fields.append(additional_fields)
    for additional_field in deletion_list:
        config.additional_fields.remove(additional_field)

    return config


def get_additional_schemas(config: Config | None, config_path: Path | None) -> Iterable[SchemaMeta]:
    """
    Get all additional models from the config file.
    """
    if config is None:
        return
    assert config_path is not None, "Config path must be set if config is set"

    for additional_model in config.additional_models:
        if isinstance(additional_model.schema_parsed, Reference):
            reference_path = Path(additional_model.schema_parsed.ref)
            if not reference_path.is_absolute():
                reference_path = config_path.parent / reference_path
            schema_parsed = read_parsed_schema(reference_path)
        else:
            reference_path = None
            schema_parsed = additional_model.schema_parsed

        if schema_parsed.title == "":
            raise ValueError("Config Error: Title is required for additional models to determine the class name")

        schema_meta = SchemaMeta(
            name=schema_parsed.title,
            module=(additional_model.module, schema_parsed.title),
        )
        schema_meta.set_schema_parsed(schema_parsed)
        if reference_path is not None:
            CONSOLE.print(
                f"Loaded additional model {schema_meta.name} from {reference_path}", show_only_on_verbose=True
            )
        else:
            CONSOLE.print(f"Loaded additional model {schema_meta.name}", show_only_on_verbose=True)
        yield schema_meta
