"""
Contains logic to replace online references in the JSON-schemas with relative paths.
"""

import re

from rich.progress import track

from bo4e_cli.io.console import CONSOLE
from bo4e_cli.io.github import OWNER, REPO
from bo4e_cli.models.meta import SchemaMeta, Schemas
from bo4e_cli.models.schema import Reference
from bo4e_cli.utils.fields import iter_schema_type

REF_ONLINE_REGEX = re.compile(
    rf"^https://raw\.githubusercontent\.com/(?:{OWNER.upper()}|{OWNER.lower()}|{OWNER.capitalize()}|Hochfrequenz)/"
    rf"{REPO}/(?P<version>[^/]+)/"
    r"src/bo4e_schemas/(?P<sub_path>(?:\w+/)*)(?P<model>\w+)\.json#?$"
)
# e.g. https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202401.1.0-rc1/src/bo4e_schemas/bo/Angebot.json
REF_DEFS_REGEX = re.compile(r"^#/\$(?:defs|definitions)/(?P<model>\w+)$")


def update_reference(
    field: Reference,
    schema: SchemaMeta,
    schemas: Schemas,
) -> None:
    """
    Update a reference to a schema file by replacing a URL reference or reference to definitions with a relative path
    to the schema file. If using references to definitions, the schema file must be in the namespace.
    Example of online reference:
    https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202401.1.0-rc1/src/bo4e_schemas/bo/Angebot.json
    Example of reference to definitions:
    #/$defs/Angebot
    """
    schema_cls_namespace = schemas.names
    match = REF_ONLINE_REGEX.search(field.ref)
    if match is not None:
        CONSOLE.print(f"Matched online reference: {field.ref}", show_only_on_verbose=True)
        if match.group("version") != str(schemas.version):
            raise ValueError(
                "Version mismatch: References across different versions of BO4E are not allowed. "
                f"{match.group('version')} does not match {schemas.version} for reference {field.ref}"
            )
        if match.group("sub_path") is not None:
            reference_module_path = [*match.group("sub_path").split("/")[:-1], match.group("model")]
        else:
            reference_module_path = [match.group("model")]
    else:
        match = REF_DEFS_REGEX.search(field.ref)
        if match is not None:
            CONSOLE.print(f"Matched reference to definitions: {field.ref}", show_only_on_verbose=True)
            if match.group("model") not in schema_cls_namespace:
                raise ValueError(
                    f"Could not find schema for reference {field.ref} in namespace "
                    f"{set(schema_el.relative_path for schema_el in schemas)}"
                )
            reference_module_path = list(schema_cls_namespace[match.group("model")].module)
        else:
            CONSOLE.print(
                f"Reference unchanged. Could not parse reference: {field.ref}",
                show_only_on_verbose=True,
                style="warning",
            )
            return

    relative_ref = "#"
    for ind, (part, own_part) in enumerate(zip(reference_module_path, schema.module)):
        if part != own_part:
            relative_ref = "../" * (len(schema.module) - ind - 1) + "/".join(reference_module_path[ind:]) + ".json#"
            break

    CONSOLE.print(f"Updated reference {field.ref} to: {relative_ref}", show_only_on_verbose=True)
    field.ref = relative_ref


def update_references(schema: SchemaMeta, schemas: Schemas) -> None:
    """
    Update all references in a schema object. Iterates through the whole structure and calls `update_reference`
    on every Reference object.
    """

    for reference in iter_schema_type(schema.schema_parsed, Reference):
        update_reference(reference, schema, schemas)


def update_references_all_schemas(schemas: Schemas) -> None:
    """
    Update all references in all schemas.
    """
    for schema in track(schemas, description="Updating references...", total=len(schemas), console=CONSOLE):
        update_references(schema, schemas)
